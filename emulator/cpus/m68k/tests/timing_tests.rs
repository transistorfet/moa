use femtos::{Instant, Frequency};
use emulator_hal::bus::BusAccess;
use emulator_hal_memory::MemoryBlock;

use moa_m68k::{M68k, M68kType, M68kAddress};
use moa_m68k::instructions::{Instruction, Target, Size};
use moa_m68k::timing::M68kInstructionTiming;
use moa_m68k::execute::M68kCycle;

const INIT_STACK: M68kAddress = 0x00002000;
const INIT_ADDR: M68kAddress = 0x00000010;


struct TimingCase {
    cpu: M68kType,
    data: &'static [u16],
    timing: (u16, u16, u16),
    ins: Instruction,
}

const TIMING_TESTS: &'static [TimingCase] = &[TimingCase {
    cpu: M68kType::MC68000,
    data: &[0xD090],
    timing: (14, 14, 6),
    ins: Instruction::ADD(Target::IndirectAReg(0), Target::DirectDReg(0), Size::Long),
}];


fn init_decode_test(cputype: M68kType) -> (M68k<Instant>, M68kCycle<Instant>, MemoryBlock<u32, Instant>) {
    // Insert basic initialization
    let len = 0x10_0000;
    let mut data = Vec::with_capacity(len);
    unsafe {
        data.set_len(len);
    }
    let mut memory = MemoryBlock::from(data);
    memory.write_beu32(Instant::START, 0, INIT_STACK).unwrap();
    memory.write_beu32(Instant::START, 4, INIT_ADDR).unwrap();

    // Initialize the CPU and make sure it's in the expected state
    let cpu = M68k::from_type(cputype, Frequency::from_mhz(10));
    let cycle = M68kCycle::new(&cpu, Instant::START);
    (cpu, cycle, memory)
}

fn load_memory<Bus: BusAccess<u32, Instant = Instant>>(bus: &mut Bus, data: &[u16]) {
    let mut addr = INIT_ADDR;
    for word in data {
        bus.write_beu16(Instant::START, addr, *word).unwrap();
        addr += 2;
    }
}

fn run_timing_test(case: &TimingCase) -> Result<(), String> {
    let (mut cpu, cycle, mut memory) = init_decode_test(case.cpu);
    load_memory(&mut memory, case.data);

    let mut executor = cycle.begin(&mut cpu, &mut memory);
    let mut timing = M68kInstructionTiming::new(case.cpu, 16);

    executor.reset_cpu().unwrap();
    assert_eq!(executor.state.pc, INIT_ADDR);
    assert_eq!(executor.state.ssp, INIT_STACK);

    executor.decode_next().unwrap();
    assert_eq!(executor.cycle.decoder.instruction, case.ins.clone());

    timing.add_instruction(&executor.cycle.decoder.instruction);
    let result = timing.calculate_clocks();
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
        Err(format!("expected {} but found {}", expected, result))
    }
}

#[test]
pub fn run_timing_tests() {
    let mut errors = 0;
    for case in TIMING_TESTS {
        print!("Testing for {:?}...", case.ins);
        match run_timing_test(case) {
            Ok(()) => println!("ok"),
            Err(err) => {
                println!("{:?}", err);
                errors += 1
            },
        }

        if let Err(_) = run_timing_test(case) {
            errors += 1;
        }
    }

    if errors > 0 {
        panic!("{} errors", errors);
    }
}
