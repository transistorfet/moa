use femtos::{Instant, Frequency};
use emulator_hal::bus::BusAccess;
use emulator_hal::step::Step;
use emulator_hal_memory::MemoryBlock;

use moa_m68k::{M68k, M68kType, M68kAddress};
use moa_m68k::state::M68kState;
use moa_m68k::execute::{M68kCycle, M68kCycleExecutor};
use moa_m68k::instructions::{Instruction, Target, Size, Sign, Direction, Condition};

const INIT_STACK: M68kAddress = 0x00002000;
const INIT_ADDR: M68kAddress = 0x00000010;

const MEM_ADDR: u32 = 0x00001234;

struct TestState {
    pc: u32,
    ssp: u32,
    usp: u32,
    d0: u32,
    d1: u32,
    a0: u32,
    a1: u32,
    sr: u16,
    mem: u32,
}

struct TestCase {
    name: &'static str,
    ins: Instruction,
    data: &'static [u16],
    cputype: M68kType,
    init: TestState,
    fini: TestState,
}


#[allow(clippy::uninit_vec)]
fn run_execute_test<F>(cputype: M68kType, mut test_func: F)
where
    F: FnMut(M68kCycleExecutor<&mut MemoryBlock<u32, Instant>, Instant>),
{
    // Insert basic initialization
    let len = 0x10_0000;
    let mut data = Vec::with_capacity(len);
    unsafe {
        data.set_len(len);
    }
    let mut memory = MemoryBlock::from(data);
    memory.write_beu32(Instant::START, 0, INIT_STACK).unwrap();
    memory.write_beu32(Instant::START, 4, INIT_ADDR).unwrap();

    let mut cpu = M68k::from_type(cputype, Frequency::from_mhz(10));
    cpu.step(Instant::START, &mut memory).unwrap();

    let cycle = M68kCycle::new(&cpu, Instant::START);
    let executor = cycle.begin(&mut cpu, &mut memory);

    assert_eq!(executor.state.pc, INIT_ADDR);
    assert_eq!(executor.state.ssp, INIT_STACK);
    assert_eq!(executor.cycle.decoder.instruction, Instruction::NOP);

    test_func(executor)
}

fn build_state(state: &TestState) -> M68kState {
    let mut new_state = M68kState::default();
    new_state.pc = state.pc;
    new_state.ssp = state.ssp;
    new_state.usp = state.usp;
    new_state.d_reg[0] = state.d0;
    new_state.d_reg[1] = state.d1;
    new_state.a_reg[0] = state.a0;
    new_state.a_reg[1] = state.a1;
    new_state.sr = state.sr;
    new_state
}

fn load_memory<Bus: BusAccess<u32, Instant = Instant>>(bus: &mut Bus, data: &[u16]) {
    for i in 0..data.len() {
        bus.write_beu16(Instant::START, (i << 1) as u32, data[i]).unwrap();
    }
}

fn run_test(case: &TestCase) {
    run_execute_test(case.cputype, |mut executor| {
        let init_state = build_state(&case.init);
        let expected_state = build_state(&case.fini);
        executor.bus.write_beu32(Instant::START, MEM_ADDR, case.init.mem).unwrap();

        load_memory(&mut executor.bus, case.data);
        *executor.state = init_state;

        executor.decode_next().unwrap();
        assert_eq!(executor.cycle.decoder.instruction, case.ins);

        executor.execute_current().unwrap();
        assert_eq!(*executor.state, expected_state);

        let mem = executor.bus.read_beu32(Instant::START, MEM_ADDR).unwrap();
        assert_eq!(mem, case.fini.mem);
    });
}

#[test]
pub fn run_execute_tests() {
    for case in TEST_CASES {
        println!("Running test {}", case.name);
        run_test(case);
    }
}

#[test]
#[ignore]
pub fn run_assembler_tests() {
    use moa_m68k::assembler::M68kAssembler;

    let mut tests = 0;
    let mut errors = 0;

    for case in TEST_CASES {
        tests += 1;
        let assembly_text = format!("{}", case.ins);
        print!("Testing assembling of {:?} ", assembly_text);
        let mut assembler = M68kAssembler::new(M68kType::MC68000);
        match assembler.assemble_words(&assembly_text) {
            Ok(data) => {
                if data == case.data {
                    print!("pass");
                } else {
                    errors += 1;
                    print!("FAILED");
                    print!("\ngot: [{}], but expected: [{}]", format_hex(&data), format_hex(case.data));
                }
                println!();
            },
            Err(err) => {
                println!("FAILED\n{:?}", err);
                errors += 1;
            },
        }
    }

    if errors > 0 {
        panic!("{} errors out of {} tests", errors, tests);
    }
}

fn format_hex(data: &[u16]) -> String {
    data.iter()
        .map(|word| format!("{:#06x}", word))
        .collect::<Vec<String>>()
        .join(", ")
}

#[rustfmt::skip]
const TEST_CASES: &'static [TestCase] = &[
    TestCase {
        name: "nop",
        ins: Instruction::NOP,
        data: &[ 0x4e71 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000002, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
    },
    TestCase {
        name: "addi with no overflow or carry",
        ins: Instruction::ADD(Target::Immediate(0x7f), Target::DirectDReg(0), Size::Byte),
        data: &[ 0x0600, 0x007F ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0x0000007f, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
    },
    TestCase {
        name: "addi with no overflow but negative",
        ins: Instruction::ADD(Target::Immediate(0x80), Target::DirectDReg(0), Size::Byte),
        data: &[ 0x0600, 0x0080 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000001, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000081, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2708, mem: 0x00000000 },
    },
    TestCase {
        name: "addi with overflow",
        ins: Instruction::ADD(Target::Immediate(0x7f), Target::DirectDReg(0), Size::Byte),
        data: &[ 0x0600, 0x007F ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000001, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000080, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x270A, mem: 0x00000000 },
    },
    TestCase {
        name: "addi with carry",
        ins: Instruction::ADD(Target::Immediate(0x80), Target::DirectDReg(0), Size::Byte),
        data: &[ 0x0600, 0x0080 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000080, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2717, mem: 0x00000000 },
    },
    TestCase {
        name: "adda immediate",
        ins: Instruction::ADDA(Target::Immediate(0xF800), 0, Size::Word),
        data: &[ 0xD0FC, 0xF800 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x27FF, mem: 0x00000000 },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0xFFFFF800, a1: 0x00000000, sr: 0x27FF, mem: 0x00000000 },
    },
    TestCase {
        name: "adda register",
        ins: Instruction::ADDA(Target::DirectDReg(0), 0, Size::Word),
        data: &[ 0xD0C0 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x0000F800, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x27FF, mem: 0x00000000 },
        fini: TestState { pc: 0x00000002, ssp: 0x00000000, usp: 0x00000000, d0: 0x0000F800, d1: 0x00000000, a0: 0xFFFFF800, a1: 0x00000000, sr: 0x27FF, mem: 0x00000000 },
    },
    TestCase {
        name: "addx",
        ins: Instruction::ADDX(Target::DirectDReg(1), Target::DirectDReg(0), Size::Byte),
        data: &[ 0xD101 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x0000007F, d1: 0x0000007F, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000002, ssp: 0x00000000, usp: 0x00000000, d0: 0x000000FE, d1: 0x0000007F, a0: 0x00000000, a1: 0x00000000, sr: 0x270A, mem: 0x00000000 },
    },
    TestCase {
        name: "addx with extend; zero flag not set",
        ins: Instruction::ADDX(Target::DirectDReg(1), Target::DirectDReg(0), Size::Byte),
        data: &[ 0xD101 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x0000007F, d1: 0x0000007F, a0: 0x00000000, a1: 0x00000000, sr: 0x2710, mem: 0x00000000 },
        fini: TestState { pc: 0x00000002, ssp: 0x00000000, usp: 0x00000000, d0: 0x000000FF, d1: 0x0000007F, a0: 0x00000000, a1: 0x00000000, sr: 0x270A, mem: 0x00000000 },
    },
    TestCase {
        name: "addx with extend; zero flag set",
        ins: Instruction::ADDX(Target::DirectDReg(1), Target::DirectDReg(0), Size::Byte),
        data: &[ 0xD101 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x0000007F, d1: 0x0000007F, a0: 0x00000000, a1: 0x00000000, sr: 0x2714, mem: 0x00000000 },
        fini: TestState { pc: 0x00000002, ssp: 0x00000000, usp: 0x00000000, d0: 0x000000FF, d1: 0x0000007F, a0: 0x00000000, a1: 0x00000000, sr: 0x270A, mem: 0x00000000 },
    },
    TestCase {
        name: "addx with extend and carry; zero flag not set",
        ins: Instruction::ADDX(Target::DirectDReg(1), Target::DirectDReg(0), Size::Byte),
        data: &[ 0xD101 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000080, d1: 0x0000007F, a0: 0x00000000, a1: 0x00000000, sr: 0x2710, mem: 0x00000000 },
        fini: TestState { pc: 0x00000002, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x0000007F, a0: 0x00000000, a1: 0x00000000, sr: 0x2711, mem: 0x00000000 },
    },
    TestCase {
        name: "addx with extend and carry; zero flag set",
        ins: Instruction::ADDX(Target::DirectDReg(1), Target::DirectDReg(0), Size::Byte),
        data: &[ 0xD101 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000080, d1: 0x0000007F, a0: 0x00000000, a1: 0x00000000, sr: 0x2714, mem: 0x00000000 },
        fini: TestState { pc: 0x00000002, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x0000007F, a0: 0x00000000, a1: 0x00000000, sr: 0x2715, mem: 0x00000000 },
    },
    TestCase {
        name: "andi with sr",
        ins: Instruction::ANDtoSR(0xF8FF),
        data: &[ 0x027C, 0xF8FF ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0xA7AA, mem: 0x00000000 },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0xA00A, mem: 0x00000000 },
    },
    TestCase {
        name: "andi with sr 2",
        ins: Instruction::ANDtoSR(0xF8FF),
        data: &[ 0x027C, 0xF8FF ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0xA7FA, mem: 0x00000000 },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0xA01A, mem: 0x00000000 },
    },
    TestCase {
        name: "asl",
        ins: Instruction::ASL(Target::Immediate(1), Target::DirectDReg(0), Size::Byte),
        data: &[ 0xE300 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000001, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000002, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000002, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
    },
    TestCase {
        name: "asr",
        ins: Instruction::ASR(Target::Immediate(1), Target::DirectDReg(0), Size::Byte),
        data: &[ 0xE200 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000081, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000002, ssp: 0x00000000, usp: 0x00000000, d0: 0x000000C0, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2719, mem: 0x00000000 },
    },
    TestCase {
        name: "blt with jump",
        ins: Instruction::Bcc(Condition::LessThan, 8),
        data: &[ 0x6D08 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2709, mem: 0x00000000 },
        fini: TestState { pc: 0x0000000A, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2709, mem: 0x00000000 },
    },
    TestCase {
        name: "blt with jump",
        ins: Instruction::Bcc(Condition::LessThan, 8),
        data: &[ 0x6D08 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000002, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
    },
    TestCase {
        name: "bchg not zero",
        ins: Instruction::BCHG(Target::Immediate(7), Target::DirectDReg(1), Size::Long),
        data: &[ 0x0841, 0x0007 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x000000FF, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x0000007F, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
    },
    TestCase {
        name: "bchg zero",
        ins: Instruction::BCHG(Target::Immediate(7), Target::DirectDReg(1), Size::Long),
        data: &[ 0x0841, 0x0007 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000080, a0: 0x00000000, a1: 0x00000000, sr: 0x2704, mem: 0x00000000 },
    },
    TestCase {
        name: "bra 8-bit",
        ins: Instruction::BRA(-32),
        data: &[ 0x60E0 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0xFFFFFFE2, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
    },
    TestCase {
        name: "cmpi equal",
        ins: Instruction::CMP(Target::Immediate(0x20), Target::DirectDReg(0), Size::Byte),
        data: &[ 0x0C00, 0x0020 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000020, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000020, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2704, mem: 0x00000000 },
    },
    TestCase {
        name: "cmpi greater than",
        ins: Instruction::CMP(Target::Immediate(0x30), Target::DirectDReg(0), Size::Byte),
        data: &[ 0x0C00, 0x0030 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000020, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000020, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2709, mem: 0x00000000 },
    },
    TestCase {
        name: "cmpi less than",
        ins: Instruction::CMP(Target::Immediate(0x10), Target::DirectDReg(0), Size::Byte),
        data: &[ 0x0C00, 0x0010 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000020, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000020, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
    },
    TestCase {
        name: "cmpi no overflow",
        ins: Instruction::CMP(Target::Immediate(0x7F), Target::DirectDReg(0), Size::Byte),
        data: &[ 0x0C00, 0x007F ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2709, mem: 0x00000000 },
    },
    TestCase {
        name: "cmpi no overflow, already negative",
        ins: Instruction::CMP(Target::Immediate(0x8001), Target::DirectDReg(0), Size::Word),
        data: &[ 0x0C40, 0x8001 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2701, mem: 0x00000000 },
    },
    TestCase {
        name: "cmpi with overflow",
        ins: Instruction::CMP(Target::Immediate(0x80), Target::DirectDReg(0), Size::Byte),
        data: &[ 0x0C00, 0x0080 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x270B, mem: 0x00000000 },
    },
    TestCase {
        name: "cmpi with overflow 2",
        ins: Instruction::CMP(Target::Immediate(0x8001), Target::DirectDReg(0), Size::Word),
        data: &[ 0x0C40, 0x8001 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000001, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000001, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x270B, mem: 0x00000000 },
    },
    TestCase {
        name: "cmpi no carry",
        ins: Instruction::CMP(Target::Immediate(0x01), Target::DirectDReg(0), Size::Byte),
        data: &[ 0x0C00, 0x0001 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x000000FF, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0x000000FF, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2708, mem: 0x00000000 },
    },
    TestCase {
        name: "cmpi with carry",
        ins: Instruction::CMP(Target::Immediate(0xFF), Target::DirectDReg(0), Size::Byte),
        data: &[ 0x0C00, 0x00FF ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000001, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000001, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2701, mem: 0x00000000 },
    },
    TestCase {
        name: "divu",
        ins: Instruction::DIVW(Target::Immediate(0x0245), 0, Sign::Unsigned),
        data: &[ 0x80FC, 0x0245 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00040000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0x007101C3, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
    },
    TestCase {
        name: "divs",
        ins: Instruction::DIVW(Target::Immediate(48), 0, Sign::Signed),
        data: &[ 0x81FC, 0x0030 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0xFFFFEB00, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0x0000FF90, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2708, mem: 0x00000000 },
    },
    TestCase {
        name: "eori",
        ins: Instruction::EOR(Target::DirectDReg(1), Target::DirectDReg(0), Size::Long),
        data: &[ 0xB380 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0xAAAA5555, d1: 0x55AA55AA, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000002, ssp: 0x00000000, usp: 0x00000000, d0: 0xFF0000FF, d1: 0x55AA55AA, a0: 0x00000000, a1: 0x00000000, sr: 0x2708, mem: 0x00000000 },
    },
    TestCase {
        name: "exg",
        ins: Instruction::EXG(Target::DirectDReg(0), Target::DirectAReg(1)),
        data: &[ 0xC189 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x12345678, d1: 0x00000000, a0: 0x00000000, a1: 0x87654321, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000002, ssp: 0x00000000, usp: 0x00000000, d0: 0x87654321, d1: 0x00000000, a0: 0x00000000, a1: 0x12345678, sr: 0x2700, mem: 0x00000000 },
    },
    TestCase {
        name: "ext",
        ins: Instruction::EXT(0, Size::Byte, Size::Word),
        data: &[ 0x4880 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x000000CB, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x27FF, mem: 0x00000000 },
        fini: TestState { pc: 0x00000002, ssp: 0x00000000, usp: 0x00000000, d0: 0x0000FFCB, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x27F8, mem: 0x00000000 },
    },
    TestCase {
        name: "ext",
        ins: Instruction::EXT(0, Size::Word, Size::Long),
        data: &[ 0x48C0 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x000000CB, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x27FF, mem: 0x00000000 },
        fini: TestState { pc: 0x00000002, ssp: 0x00000000, usp: 0x00000000, d0: 0x000000CB, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x27F0, mem: 0x00000000 },
    },

    TestCase {
        name: "lsl",
        ins: Instruction::LSL(Target::Immediate(1), Target::DirectDReg(0), Size::Byte),
        data: &[ 0xE308 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000001, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x271F, mem: 0x00000000 },
        fini: TestState { pc: 0x00000002, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000002, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
    },
    TestCase {
        name: "lsl with bit out",
        ins: Instruction::LSL(Target::Immediate(1), Target::DirectDReg(0), Size::Byte),
        data: &[ 0xE308 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000081, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000002, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000002, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2711, mem: 0x00000000 },
    },
    TestCase {
        name: "lsr",
        ins: Instruction::LSR(Target::Immediate(1), Target::DirectDReg(0), Size::Byte),
        data: &[ 0xE208 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000081, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000002, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000040, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2711, mem: 0x00000000 },
    },

    TestCase {
        name: "muls",
        ins: Instruction::MULW(Target::Immediate(0x0276), 0, Sign::Signed),
        data: &[ 0xC1FC, 0x0276 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000200, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0x0004ec00, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
    },
    TestCase {
        name: "movel",
        ins: Instruction::MOVE(Target::DirectDReg(0), Target::DirectDReg(1), Size::Long),
        data: &[ 0x2200 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0xFEDCBA98, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000002, ssp: 0x00000000, usp: 0x00000000, d0: 0xFEDCBA98, d1: 0xFEDCBA98, a0: 0x00000000, a1: 0x00000000, sr: 0x2708, mem: 0x00000000 },
    },
    TestCase {
        name: "movea",
        ins: Instruction::MOVEA(Target::DirectDReg(0), 0, Size::Long),
        data: &[ 0x2040 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0xFEDCBA98, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x27FF, mem: 0x00000000 },
        fini: TestState { pc: 0x00000002, ssp: 0x00000000, usp: 0x00000000, d0: 0xFEDCBA98, d1: 0x00000000, a0: 0xFEDCBA98, a1: 0x00000000, sr: 0x27FF, mem: 0x00000000 },
    },

    // MOVEM
    TestCase {
        name: "movem word to target",
        ins: Instruction::MOVEM(Target::IndirectAReg(0), Size::Word, Direction::ToTarget, 0x0003),
        data: &[ 0x4890, 0x0003 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0xABCD1234, d1: 0xEFEF5678, a0:   MEM_ADDR, a1: 0x00000000, sr: 0x27FF, mem: 0x00000000 },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0xABCD1234, d1: 0xEFEF5678, a0:   MEM_ADDR, a1: 0x00000000, sr: 0x27FF, mem: 0x12345678 },
    },
    TestCase {
        name: "movem long to target",
        ins: Instruction::MOVEM(Target::IndirectAReg(0), Size::Long, Direction::ToTarget, 0x0001),
        data: &[ 0x48D0, 0x0001 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0xABCD1234, d1: 0x00000000, a0:   MEM_ADDR, a1: 0x00000000, sr: 0x27FF, mem: 0x00000000 },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0xABCD1234, d1: 0x00000000, a0:   MEM_ADDR, a1: 0x00000000, sr: 0x27FF, mem: 0xABCD1234 },
    },
    TestCase {
        name: "movem long from target",
        ins: Instruction::MOVEM(Target::IndirectAReg(0), Size::Long, Direction::FromTarget, 0x0001),
        data: &[ 0x4CD0, 0x0001 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0:   MEM_ADDR, a1: 0x00000000, sr: 0x27FF, mem: 0xABCD1234 },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0xABCD1234, d1: 0x00000000, a0:   MEM_ADDR, a1: 0x00000000, sr: 0x27FF, mem: 0xABCD1234 },
    },
    TestCase {
        name: "movem word from target inc",
        ins: Instruction::MOVEM(Target::IndirectARegInc(0), Size::Word, Direction::FromTarget, 0x0001),
        data: &[ 0x4C98, 0x0001 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0xFFFFFFFF, d1: 0x00000000, a0:   MEM_ADDR, a1: 0x00000000, sr: 0x27FF, mem: 0xABCD1234 },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0xFFFFABCD, d1: 0x00000000, a0: MEM_ADDR+2, a1: 0x00000000, sr: 0x27FF, mem: 0xABCD1234 },
    },
    TestCase {
        name: "movem long to target dec",
        ins: Instruction::MOVEM(Target::IndirectARegDec(0), Size::Long, Direction::ToTarget, 0x8000),
        data: &[ 0x48E0, 0x8000 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0xABCD1234, d1: 0x00000000, a0: MEM_ADDR+4, a1: 0x00000000, sr: 0x27FF, mem: 0x00000000 },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0xABCD1234, d1: 0x00000000, a0:   MEM_ADDR, a1: 0x00000000, sr: 0x27FF, mem: 0xABCD1234 },
    },


    // MOVEP
    TestCase {
        name: "movep word to even memory",
        ins: Instruction::MOVEP(0, 0, 0, Size::Word, Direction::ToTarget),
        data: &[ 0x0188, 0x0000 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x000055AA, d1: 0x00000000, a0:   MEM_ADDR, a1: 0x00000000, sr: 0x27FF, mem: 0xFFFFFFFF },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0x000055AA, d1: 0x00000000, a0:   MEM_ADDR, a1: 0x00000000, sr: 0x27FF, mem: 0x55FFAAFF },
    },
    TestCase {
        name: "movep word to odd memory",
        ins: Instruction::MOVEP(0, 0, 1, Size::Word, Direction::ToTarget),
        data: &[ 0x0188, 0x0001 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x000055AA, d1: 0x00000000, a0:   MEM_ADDR, a1: 0x00000000, sr: 0x27FF, mem: 0xFFFFFFFF },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0x000055AA, d1: 0x00000000, a0:   MEM_ADDR, a1: 0x00000000, sr: 0x27FF, mem: 0xFF55FFAA },
    },
    TestCase {
        name: "movep long to even memory upper",
        ins: Instruction::MOVEP(0, 0, 0, Size::Long, Direction::ToTarget),
        data: &[ 0x01C8, 0x0000 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0xAABBCCDD, d1: 0x00000000, a0:   MEM_ADDR, a1: 0x00000000, sr: 0x27FF, mem: 0xFFFFFFFF },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0xAABBCCDD, d1: 0x00000000, a0:   MEM_ADDR, a1: 0x00000000, sr: 0x27FF, mem: 0xAAFFBBFF },
    },
    TestCase {
        name: "movep long to even memory lower",
        ins: Instruction::MOVEP(0, 0, 0, Size::Long, Direction::ToTarget),
        data: &[ 0x01C8, 0x0000 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0xAABBCCDD, d1: 0x00000000, a0: MEM_ADDR-4, a1: 0x00000000, sr: 0x27FF, mem: 0xFFFFFFFF },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0xAABBCCDD, d1: 0x00000000, a0: MEM_ADDR-4, a1: 0x00000000, sr: 0x27FF, mem: 0xCCFFDDFF },
    },
    TestCase {
        name: "movep word from even memory",
        ins: Instruction::MOVEP(0, 0, 0, Size::Word, Direction::FromTarget),
        data: &[ 0x0108, 0x0000 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0:   MEM_ADDR, a1: 0x00000000, sr: 0x27FF, mem: 0x55FFAAFF },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0x000055AA, d1: 0x00000000, a0:   MEM_ADDR, a1: 0x00000000, sr: 0x27FF, mem: 0x55FFAAFF },
    },
    TestCase {
        name: "movep word from odd memory",
        ins: Instruction::MOVEP(0, 0, 1, Size::Word, Direction::FromTarget),
        data: &[ 0x0108, 0x0001 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0:   MEM_ADDR, a1: 0x00000000, sr: 0x27FF, mem: 0xFF55FFAA },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0x000055AA, d1: 0x00000000, a0:   MEM_ADDR, a1: 0x00000000, sr: 0x27FF, mem: 0xFF55FFAA },
    },
    // TODO not sure if these cases are correct
    TestCase {
        name: "movep long from even memory upper",
        ins: Instruction::MOVEP(0, 0, 0, Size::Long, Direction::FromTarget),
        data: &[ 0x0148, 0x0000 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0:   MEM_ADDR, a1: 0x00000000, sr: 0x27FF, mem: 0xAAFFBBFF },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0xAABBCCDD, d1: 0x00000000, a0:   MEM_ADDR, a1: 0x00000000, sr: 0x27FF, mem: 0xAAFFBBFF },
    },
    TestCase {
        name: "movep long from even memory lower",
        ins: Instruction::MOVEP(0, 0, 0, Size::Long, Direction::FromTarget),
        data: &[ 0x0148, 0x0000 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: MEM_ADDR-4, a1: 0x00000000, sr: 0x27FF, mem: 0xCCFFDDFF },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0x0000CCDD, d1: 0x00000000, a0: MEM_ADDR-4, a1: 0x00000000, sr: 0x27FF, mem: 0xCCFFDDFF },
    },


    // NEG
    TestCase {
        name: "neg",
        ins: Instruction::NEG(Target::DirectDReg(0), Size::Word),
        data: &[ 0x4440 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000080, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000002, ssp: 0x00000000, usp: 0x00000000, d0: 0x0000FF80, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2719, mem: 0x00000000 },
    },


    TestCase {
        name: "ori",
        ins: Instruction::OR(Target::Immediate(0xFF), Target::DirectAReg(0), Size::Byte),
        data: &[ 0x0008, 0x00FF ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x000000FF, a1: 0x00000000, sr: 0x2708, mem: 0x00000000 },
    },
    TestCase {
        name: "ori with sr",
        ins: Instruction::ORtoSR(0x00AA),
        data: &[ 0x007C, 0x00AA ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0xA755, mem: 0x00000000 },
        fini: TestState { pc: 0x00000004, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0xA71F, mem: 0x00000000 },
    },



    TestCase {
        name: "rol",
        ins: Instruction::ROL(Target::Immediate(1), Target::DirectDReg(0), Size::Byte),
        data: &[ 0xE318 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000080, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000002, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000001, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2701, mem: 0x00000000 },
    },
    TestCase {
        name: "ror",
        ins: Instruction::ROR(Target::Immediate(1), Target::DirectDReg(0), Size::Byte),
        data: &[ 0xE218 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000001, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000002, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000080, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2709, mem: 0x00000000 },
    },
    TestCase {
        name: "roxl",
        ins: Instruction::ROXL(Target::Immediate(1), Target::DirectDReg(0), Size::Byte),
        data: &[ 0xE310 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000080, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000002, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2715, mem: 0x00000000 },
    },
    TestCase {
        name: "roxr",
        ins: Instruction::ROXR(Target::Immediate(1), Target::DirectDReg(0), Size::Byte),
        data: &[ 0xE210 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000001, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000002, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2715, mem: 0x00000000 },
    },
    TestCase {
        name: "roxl two bits",
        ins: Instruction::ROXL(Target::Immediate(2), Target::DirectDReg(0), Size::Byte),
        data: &[ 0xE510 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000080, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000002, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000001, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
    },
    TestCase {
        name: "roxr two bits",
        ins: Instruction::ROXR(Target::Immediate(2), Target::DirectDReg(0), Size::Byte),
        data: &[ 0xE410 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000001, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000002, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000080, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2708, mem: 0x00000000 },
    },

    TestCase {
        name: "subx",
        ins: Instruction::SUBX(Target::DirectDReg(1), Target::DirectDReg(0), Size::Byte),
        data: &[ 0x9101 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x000000FF, d1: 0x0000007F, a0: 0x00000000, a1: 0x00000000, sr: 0x2700, mem: 0x00000000 },
        fini: TestState { pc: 0x00000002, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000080, d1: 0x0000007F, a0: 0x00000000, a1: 0x00000000, sr: 0x2708, mem: 0x00000000 },
    },
    TestCase {
        name: "subx with extend",
        ins: Instruction::SUBX(Target::DirectDReg(1), Target::DirectDReg(0), Size::Byte),
        data: &[ 0x9101 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x000000FF, d1: 0x0000007F, a0: 0x00000000, a1: 0x00000000, sr: 0x2710, mem: 0x00000000 },
        fini: TestState { pc: 0x00000002, ssp: 0x00000000, usp: 0x00000000, d0: 0x0000007F, d1: 0x0000007F, a0: 0x00000000, a1: 0x00000000, sr: 0x2702, mem: 0x00000000 },
    },
    TestCase {
        name: "subx with extend and carry",
        ins: Instruction::SUBX(Target::DirectDReg(1), Target::DirectDReg(0), Size::Byte),
        data: &[ 0x9101 ],
        cputype: M68kType::MC68010,
        init: TestState { pc: 0x00000000, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x0000007F, a0: 0x00000000, a1: 0x00000000, sr: 0x2710, mem: 0x00000000 },
        fini: TestState { pc: 0x00000002, ssp: 0x00000000, usp: 0x00000000, d0: 0x00000080, d1: 0x0000007F, a0: 0x00000000, a1: 0x00000000, sr: 0x2719, mem: 0x00000000 },
    },
];
