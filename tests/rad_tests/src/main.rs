
const DEFAULT_RAD_TESTS: &str = "tests/jsmoo/misc/tests/GeneratedTests/z80/v1/";

use std::io::prelude::*;
use std::fmt::{Debug, UpperHex};
use std::path::PathBuf;
use std::time::SystemTime;
use std::fs::{self, File};

use clap::{Parser, ArgEnum};
use flate2::read::GzDecoder;
use serde_derive::Deserialize;

use moa_core::{System, Error, MemoryBlock, BusPort, Frequency, Address, Addressable, Steppable, wrap_transmutable};

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
struct TestCase {
    name: String,
    #[serde(rename(deserialize = "initial"))]
    initial_state: TestState,
    #[serde(rename(deserialize = "final"))]
    final_state: TestState,
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
    }
}


fn init_execute_test(cputype: Z80Type, state: &TestState) -> Result<(Z80, System), Error> {
    let mut system = System::default();

    // Insert basic initialization
    let data = vec![0; 0x01000000];
    let mem = MemoryBlock::new(data);
    system.add_addressable_device(0x00000000, wrap_transmutable(mem)).unwrap();

    let port = BusPort::new(0, 16, 8, system.bus.clone());
    let mut cpu = Z80::new(cputype, Frequency::from_mhz(10), port);
    cpu.state.status = Status::Running;

    load_state(&mut cpu, &mut system, state)?;

    Ok((cpu, system))
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

fn load_state(cpu: &mut Z80, system: &mut System, initial: &TestState) -> Result<(), Error> {
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

    Ok(())
}

const IGNORE_FLAG_MASK: u8 = Flags::HalfCarry as u8 | Flags::F3 as u8 | Flags::F5 as u8;

fn assert_state(cpu: &Z80, system: &System, expected: &TestState, check_extra_flags: bool) -> Result<(), Error> {
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

    Ok(())
}

fn step_cpu_and_assert(cpu: &mut Z80, system: &System, case: &TestCase, check_extra_flags: bool) -> Result<(), Error> {
    let _clock_elapsed = cpu.step(&system)?;

    assert_state(&cpu, &system, &case.final_state, check_extra_flags)?;

    Ok(())
}

fn run_test(case: &TestCase, args: &Args) -> Result<(), Error> {
    let (mut cpu, system) = init_execute_test(Z80Type::Z80, &case.initial_state).unwrap();
    let mut initial_cpu = cpu.clone();

    let result = step_cpu_and_assert(&mut cpu, &system, case, args.check_extra_flags);

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

        // If specified, only test files that start with a given string
        if let Some(filter) = &args.filter {
            if !path.file_name().unwrap().to_str().unwrap().starts_with(filter) {
                continue;
            }
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

