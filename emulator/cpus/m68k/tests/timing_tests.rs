
use femtos::{Instant, Frequency};

use moa_core::{System, Error, MemoryBlock, BusPort, Address, Addressable, Device};

use moa_m68k::{M68k, M68kType};
use moa_m68k::instructions::{Instruction, Target, Size};
use moa_m68k::timing::M68kInstructionTiming;
use moa_m68k::execute::M68kCycle;

const INIT_STACK: Address = 0x00002000;
const INIT_ADDR: Address = 0x00000010;


struct TimingCase {
    cpu: M68kType,
    data: &'static [u16],
    timing: (u16, u16, u16),
    ins: Instruction,
}

const TIMING_TESTS: &'static [TimingCase] = &[
    TimingCase { cpu: M68kType::MC68000, data: &[0xD090], timing: ( 14,  14,   6), ins: Instruction::ADD(Target::IndirectAReg(0), Target::DirectDReg(0), Size::Long) },
];


fn init_decode_test(cputype: M68kType) -> (M68k, M68kCycle, System) {
    let mut system = System::default();

    // Insert basic initialization
    let data = vec![0; 0x00100000];
    let mem = MemoryBlock::new(data);
    system.add_addressable_device(0x00000000, Device::new(mem)).unwrap();
    system.get_bus().write_beu32(Instant::START, 0, INIT_STACK as u32).unwrap();
    system.get_bus().write_beu32(Instant::START, 4, INIT_ADDR as u32).unwrap();

    // Initialize the CPU and make sure it's in the expected state
    let cpu = M68k::from_type(cputype, Frequency::from_mhz(10), system.bus.clone(), 0);
    //cpu.reset_cpu().unwrap();
    assert_eq!(cpu.state.pc, INIT_ADDR as u32);
    assert_eq!(cpu.state.ssp, INIT_STACK as u32);

    let cycle = M68kCycle::new(&cpu, system.clock);
    assert_eq!(cycle.decoder.start, INIT_ADDR as u32);
    assert_eq!(cycle.decoder.instruction, Instruction::NOP);
    (cpu, cycle, system)
}

fn load_memory(system: &System, data: &[u16]) {
    let mut addr = INIT_ADDR;
    for word in data {
        system.get_bus().write_beu16(system.clock, addr, *word).unwrap();
        addr += 2;
    }
}

fn run_timing_test(case: &TimingCase) -> Result<(), Error> {
    let (mut cpu, cycle, system) = init_decode_test(case.cpu);
    let mut executor = cycle.begin(&mut cpu);
    let mut timing = M68kInstructionTiming::new(case.cpu, 16);

    load_memory(&system, case.data);
    executor.decode_next().unwrap();
    assert_eq!(executor.cycle.decoder.instruction, case.ins.clone());

    timing.add_instruction(&executor.cycle.decoder.instruction);
    let result = timing.calculate_clocks(false, 1);
    let expected = match case.cpu {
        M68kType::MC68000 => case.timing.0,
        M68kType::MC68010 => case.timing.1,
        _ => case.timing.2,
    };

    //assert_eq!(expected, result);
    if expected == result {
        Ok(())
    } else {
        println!("{:?}", timing);
        Err(Error::new(format!("expected {} but found {}", expected, result)))
    }
}

#[test]
pub fn run_timing_tests() {
    let mut errors = 0;
    for case in TIMING_TESTS {
        // NOTE switched to only show the failures rather than all tests
        //print!("Testing for {:?}...", case.ins);
        //match run_timing_test(case) {
        //    Ok(()) => println!("ok"),
        //    Err(err) => { println!("{}", err.msg); errors += 1 },
        //}

        if let Err(_) = run_timing_test(case) {
            errors += 1;
        }
    }

    if errors > 0 {
        panic!("{} errors", errors);
    }
}

