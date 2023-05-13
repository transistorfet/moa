
const DEFAULT_RAD_TESTS: &str = "tests/jsmoo/misc/tests/GeneratedTests/z80/v1/";

use std::rc::Rc;
use std::cell::RefCell;
use std::io::prelude::*;
use std::fmt::{Debug, UpperHex};
use std::path::PathBuf;
use std::time::SystemTime;
use std::fs::{self, File};

use clap::{Parser, ArgEnum};
use flate2::read::GzDecoder;
use serde_derive::Deserialize;

use moa_core::{System, Error, MemoryBlock, Bus, BusPort, Frequency, Address, Addressable, Steppable, wrap_transmutable};

use moa_z80::{Z80, Z80Type};
use moa_z80::state::Flags;
use moa_z80::state::Status;

#[derive(Copy, Clone, PartialEq, Eq, ArgEnum)]
enum Selection {
    Include,
    Exclude,
    ExcludeAddr,
    Only,
}

#[derive(Parser)]
struct Args {
    /// Filter the tests by gzip file name
    filter: Option<String>,
    /// Only run the one test with the given number
    #[clap(short, long)]
    only: Option<String>,
    /// Dump the CPU state when a test fails
    #[clap(short, long)]
    debug: bool,
    /// Only print a summary for each test file
    #[clap(short, long)]
    quiet: bool,
    /// Check the Half Carry, F3, and F5 flags for accuracy
    #[clap(short = 'f', long)]
    check_extra_flags: bool,
    /// Check undocumented instructions
    #[clap(short = 'u', long)]
    check_undocumented: bool,
    /// Directory to the test suite to run
    #[clap(long, default_value = DEFAULT_RAD_TESTS)]
    testsuite: String,
    #[clap(long, short, arg_enum, default_value_t = Selection::Include)]
    exceptions: Selection,
}

fn main() {
    let args = Args::parse();
    run_all_tests(&args);
}


#[derive(Debug, Deserialize)]
struct TestState {
    pc: u16,
    sp: u16,
    a: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    f: u8,
    h: u8,
    l: u8,
    //i: u8,
    //r: u8,
    //ei: u8,
    //wz: u8,
    ix: u16,
    iy: u16,
    af_: u16,
    bc_: u16,
    de_: u16,
    hl_: u16,
    //im: u8,
    //p: u8,
    //q: u8,
    //iff1: u8,
    //iff2: u8,
    ram: Vec<(u16, u8)>,
}

#[derive(Debug, Deserialize)]
struct TestPort {
    addr: u16,
    value: u8,
    atype: String,
}

#[derive(Debug, Deserialize)]
struct TestCase {
    name: String,
    #[serde(rename(deserialize = "initial"))]
    initial_state: TestState,
    #[serde(rename(deserialize = "final"))]
    final_state: TestState,
    #[serde(default)]
    ports: Vec<TestPort>,
}

impl TestState {
    pub fn dump(&self) {
        println!(" a: {:02x}   a': {:02x}", self.a, self.af_ >> 8);
        println!(" b: {:02x}   b': {:02x}", self.b, self.bc_ & 0xff);
        println!(" c: {:02x}   c': {:02x}", self.c, self.bc_ >> 8);
        println!(" d: {:02x}   d': {:02x}", self.d, self.de_ & 0xff);
        println!(" e: {:02x}   e': {:02x}", self.e, self.de_ >> 8);
        println!(" f: {:02x}   f': {:02x}", self.f, self.af_ & 0xff);
        println!(" h: {:02x}   h': {:02x}", self.h, self.hl_ >> 8);
        println!(" l: {:02x}   l': {:02x}", self.l, self.hl_ & 0xff);
        println!("pc: {:04x}   sp: {:04x}", self.pc, self.sp);
        println!("ix: {:04x}   iy: {:04x}", self.ix, self.iy);

        println!("ram: ");
        for (addr, byte) in self.ram.iter() {
            println!("{:04x} {:02x} ", *addr, *byte);
        }
    }
}

impl TestCase {
    pub fn dump(&self) {
        println!("{}", self.name);
        println!("initial:");
        self.initial_state.dump();
        println!("final:");
        self.final_state.dump();

        println!("ports: ");
        for port in self.ports.iter() {
            println!("{:04x} {:02x} {}", port.addr, port.value, port.atype);
        }
    }
}


fn init_execute_test(cputype: Z80Type, state: &TestState, ports: &[TestPort]) -> Result<(Z80, System, Rc<RefCell<Bus>>), Error> {
    let mut system = System::default();

    // Insert basic initialization
    let mem = MemoryBlock::new(vec![0; 0x1_0000]);
    system.add_addressable_device(0x00000000, wrap_transmutable(mem)).unwrap();

    // Set up IOREQ as memory space
    let io_ram = wrap_transmutable(MemoryBlock::new(vec![0; 0x10000]));
    let io_bus = Rc::new(RefCell::new(Bus::default()));
    io_bus.borrow_mut().set_ignore_unmapped(true);
    io_bus.borrow_mut().insert(0x0000, io_ram.clone());

    let port = BusPort::new(0, 16, 8, system.bus.clone());
    let ioport = BusPort::new(0, 16, 8, io_bus.clone());
    let mut cpu = Z80::new(cputype, Frequency::from_mhz(10), port, Some(ioport));
    cpu.state.status = Status::Running;

    load_state(&mut cpu, &mut system, io_bus.clone(), state, ports)?;

    Ok((cpu, system, io_bus))
}

fn assert_value<T>(actual: T, expected: T, message: &str) -> Result<(), Error>
where
    T: PartialEq + Debug + UpperHex
{
    if actual == expected {
        Ok(())
    } else {
        Err(Error::assertion(&format!("{:#X} != {:#X}, {}", actual, expected, message)))
    }
}

fn load_state(cpu: &mut Z80, system: &mut System, io_bus: Rc<RefCell<Bus>>, initial: &TestState, ports: &[TestPort]) -> Result<(), Error> {
    cpu.state.reg[0] = initial.b;
    cpu.state.reg[1] = initial.c;
    cpu.state.reg[2] = initial.d;
    cpu.state.reg[3] = initial.e;
    cpu.state.reg[4] = initial.h;
    cpu.state.reg[5] = initial.l;
    cpu.state.reg[6] = initial.a;
    cpu.state.reg[7] = initial.f;
    cpu.state.shadow_reg[0] = (initial.bc_ >> 8) as u8;
    cpu.state.shadow_reg[1] = (initial.bc_ & 0xff) as u8;
    cpu.state.shadow_reg[2] = (initial.de_ >> 8) as u8;
    cpu.state.shadow_reg[3] = (initial.de_ & 0xff) as u8;
    cpu.state.shadow_reg[4] = (initial.hl_ >> 8) as u8;
    cpu.state.shadow_reg[5] = (initial.hl_ & 0xff) as u8;
    cpu.state.shadow_reg[6] = (initial.af_ >> 8) as u8;
    cpu.state.shadow_reg[7] = (initial.af_ & 0xff) as u8;

    cpu.state.ix = initial.ix;
    cpu.state.iy = initial.iy;
    cpu.state.sp = initial.sp;
    cpu.state.pc = initial.pc;

    // Load data bytes into memory
    for (addr, byte) in initial.ram.iter() {
        system.get_bus().write_u8(system.clock, *addr as u64, *byte)?;
    }

    // Load data bytes into io space
    for port in ports.iter() {
        io_bus.borrow_mut().write_u8(system.clock, port.addr as u64, port.value)?;
    }

    Ok(())
}

const IGNORE_FLAG_MASK: u8 = Flags::F3 as u8 | Flags::F5 as u8;

fn assert_state(cpu: &Z80, system: &System, io_bus: Rc<RefCell<Bus>>, expected: &TestState, check_extra_flags: bool, ports: &[TestPort]) -> Result<(), Error> {
    assert_value(cpu.state.reg[0], expected.b, "b")?;
    assert_value(cpu.state.reg[1], expected.c, "c")?;
    assert_value(cpu.state.reg[2], expected.d, "d")?;
    assert_value(cpu.state.reg[3], expected.e, "e")?;
    assert_value(cpu.state.reg[4], expected.h, "h")?;
    assert_value(cpu.state.reg[5], expected.l, "l")?;
    assert_value(cpu.state.reg[6], expected.a, "a")?;
    if check_extra_flags {
        assert_value(cpu.state.reg[7], expected.f, "f")?;
    } else {
        assert_value(cpu.state.reg[7] & !IGNORE_FLAG_MASK, expected.f & !IGNORE_FLAG_MASK, "f")?;
    }
    assert_value(cpu.state.shadow_reg[0], (expected.bc_ >> 8) as u8, "b'")?;
    assert_value(cpu.state.shadow_reg[1], (expected.bc_ & 0xff) as u8, "c'")?;
    assert_value(cpu.state.shadow_reg[2], (expected.de_ >> 8) as u8, "d'")?;
    assert_value(cpu.state.shadow_reg[3], (expected.de_ & 0xff) as u8, "e'")?;
    assert_value(cpu.state.shadow_reg[4], (expected.hl_ >> 8) as u8, "h'")?;
    assert_value(cpu.state.shadow_reg[5], (expected.hl_ & 0xff) as u8, "l'")?;
    assert_value(cpu.state.shadow_reg[6], (expected.af_ >> 8) as u8, "a'")?;
    assert_value(cpu.state.shadow_reg[7], (expected.af_ & 0xff) as u8, "f'")?;

    assert_value(cpu.state.ix, expected.ix, "ix")?;
    assert_value(cpu.state.iy, expected.iy, "iy")?;
    assert_value(cpu.state.sp, expected.sp, "sp")?;
    assert_value(cpu.state.pc, expected.pc, "pc")?;

    let addr_mask = cpu.port.address_mask();

    // Load data bytes into memory
    for (addr, byte) in expected.ram.iter() {
        let actual = system.get_bus().read_u8(system.clock, *addr as Address & addr_mask)?;
        assert_value(actual, *byte, &format!("ram at {:x}", addr))?;
    }

    // Load data bytes into io space
    for port in ports.iter() {
        if port.atype == "w" {
            let actual = io_bus.borrow_mut().read_u8(system.clock, port.addr as u64)?;
            assert_value(actual, port.value, &format!("port value at {:x}", port.addr))?;
        }
    }

    Ok(())
}

fn step_cpu_and_assert(cpu: &mut Z80, system: &System, io_bus: Rc<RefCell<Bus>>, case: &TestCase, check_extra_flags: bool) -> Result<(), Error> {
    let _clock_elapsed = cpu.step(&system)?;

    assert_state(&cpu, &system, io_bus, &case.final_state, check_extra_flags, &case.ports)?;

    Ok(())
}

fn run_test(case: &TestCase, args: &Args) -> Result<(), Error> {
    let (mut cpu, system, io_bus) = init_execute_test(Z80Type::Z80, &case.initial_state, &case.ports).unwrap();
    let mut initial_cpu = cpu.clone();

    let result = step_cpu_and_assert(&mut cpu, &system, io_bus, case, args.check_extra_flags);

    match result {
        Ok(()) => Ok(()),
        Err(err) => {
            if !args.quiet {
                if args.debug {
                    case.dump();
                    println!("");
                    initial_cpu.dump_state(system.clock);
                    cpu.dump_state(system.clock);
                }
                println!("FAILED: {}",  err.msg);
            }
            Err(err)
        },
    }
}

fn test_json_file(path: PathBuf, args: &Args) -> (usize, usize, String) {
    let extension = path.extension().unwrap();

    let cases: Vec<TestCase> = if extension == "gz" {
        let file = File::open(&path).unwrap();
        let mut decoder = GzDecoder::new(file);
        let mut data = String::new();
        decoder.read_to_string(&mut data).unwrap();
        serde_json::from_str(&data).unwrap()
    } else {
        let data = fs::read(&path).unwrap();
        serde_json::from_slice(&data).unwrap()
    };

    let mut passed = 0;
    let mut failed = 0;
    for mut case in cases {
        if let Some(only) = args.only.as_ref() {
            if !case.name.ends_with(only) {
                continue;
            }
        }

        // Sort the ram memory for debugging help
        if args.debug {
            case.initial_state.ram.sort_by_key(|(addr, _)| *addr);
            case.final_state.ram.sort_by_key(|(addr, _)| *addr);
        }

        if !args.quiet {
            println!("Running test {}", case.name);
        }
        let result = run_test(&case, args);

        if let Err(err) = result {
            failed += 1;
            if !args.quiet {
                println!("FAILED: {:?}",  err);
            }
        } else {
            passed += 1
        }
    }

    let name = path.file_name().unwrap().to_str().unwrap();
    let message = if failed == 0 {
        format!("{} completed, all passed!", name)
    } else {
        format!("{} completed: {} passed, {} FAILED", name, passed, failed)
    };

    (passed, failed, message)
}


fn run_all_tests(args: &Args) {
    let mut passed = 0;
    let mut failed = 0;
    let mut messages = vec![];


    let mut tests: Vec<PathBuf> = fs::read_dir(&args.testsuite)
        .unwrap()
        .map(|dirent| dirent.unwrap().path())
        .collect();
    tests.sort();

    let start = SystemTime::now();
    for path in tests {
        // Only test gzip files (the repo has .md files as well)
        let extension = path.extension().unwrap();
        if extension != "json" && extension != "gz" {
            continue;
        }

        let name = path.file_name().unwrap().to_str().unwrap();

        // If specified, only test files that start with a given string
        if let Some(filter) = &args.filter {
            if !name.starts_with(filter) {
                continue;
            }
        }

        if !args.check_undocumented && is_undocumented_instruction(name) {
            continue;
        }

        // Run every test in the file
        let (test_passed, test_failed, message) = test_json_file(path, args);

        // In quiet mode, print each summary as it's received to give a progress update
        if args.quiet {
            println!("{}", message);
        }

        passed += test_passed;
        failed += test_failed;
        messages.push(message);
    }
    let elapsed_secs = start.elapsed().unwrap().as_secs();

    // Print the stored summary if not in quite mode
    if !args.quiet {
        for message in messages {
            println!("{}", message);
        }
    }

    println!("");
    println!("passed: {}, failed: {}, total {:.0}%", passed, failed, ((passed as f32) / (passed as f32 + failed as f32)) * 100.0);
    println!("completed in {}m {}s", elapsed_secs / 60, elapsed_secs % 60);
}

fn is_undocumented_instruction(name: &str) -> bool {
    let mut opcodes: Vec<u8> = name
        .splitn(3, &[' ', '.'])
        .filter_map(|s| u8::from_str_radix(s, 16).ok())
        .collect();
    opcodes.extend(vec![0; 3 - opcodes.len()]);

    match (opcodes[0], opcodes[1]) {
        (0xCB, op) => {
            op >= 0x30 && op <= 0x37
        },
        (0xDD, 0xCB) |
        (0xFD, 0xCB) => {
            !(opcodes[2] & 0x07 == 0x06 && opcodes[2] != 0x36)
        },
        (0xDD, op) |
        (0xFD, op) => {
            let upper3 = op & 0x70;
            let upper4 = op & 0xF0;
            let lower = op & 0x0F;
            !(lower == 0x06 && upper3 >= 0x30 && upper3 <= 0x60) &&
            !(lower == 0x0E && upper4 >= 0x40 && upper3 <= 0xB0) &&
            !(op >= 0x70 && op <= 0x77 && op != 0x76) &&
            !(op >= 0x21 && op <= 0x23 && op >= 0x34 && op <= 0x36 && op >= 0x29 && op <= 0x2B) &&
            !(lower == 0x09 && upper4 <= 0x30) &&
            !(op == 0xE1 || op == 0xE3 || op == 0xE5 || op == 0xE9 || op == 0xF9)
        },
        (0xED, op) => {
            // NOTE this assumes the tests don't have the missing instructions, or the Z180 instructions
            //      so it only checks for the undocumented ones
            op == 0x63 || op == 0x6B || op == 0x70 || op == 0x71
        },
        _ => false
    }
}

