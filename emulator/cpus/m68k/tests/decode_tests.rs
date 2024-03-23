use femtos::{Instant, Frequency};
use emulator_hal::bus::BusAccess;
use emulator_hal_memory::MemoryBlock;

use moa_m68k::{M68k, M68kType, M68kAddress};
use moa_m68k::instructions::{Instruction, Target, Size, Sign, XRegister, BaseRegister, IndexRegister, Direction};
use moa_m68k::assembler::M68kAssembler;
use moa_m68k::execute::M68kCycle;

const INIT_STACK: M68kAddress = 0x00002000;
const INIT_ADDR: M68kAddress = 0x00000010;

struct TestCase {
    cpu: M68kType,
    data: &'static [u16],
    ins: Option<Instruction>,
}

#[rustfmt::skip]
const DECODE_TESTS: &'static [TestCase] = &[
    // MC68000
    TestCase { cpu: M68kType::MC68000, data: &[0x4e71],                             ins: Some(Instruction::NOP) },
    // TODO I think this one is illegal (which is causing problems for the assembler)
    //TestCase { cpu: M68kType::MC68000, data: &[0x0008, 0x00FF],                     ins: Some(Instruction::OR(Target::Immediate(0xFF), Target::DirectAReg(0), Size::Byte)) },
    TestCase { cpu: M68kType::MC68000, data: &[0x003C, 0x00FF],                     ins: Some(Instruction::ORtoCCR(0xFF)) },
    TestCase { cpu: M68kType::MC68000, data: &[0x007C, 0x1234],                     ins: Some(Instruction::ORtoSR(0x1234)) },
    TestCase { cpu: M68kType::MC68000, data: &[0x0263, 0x1234],                     ins: Some(Instruction::AND(Target::Immediate(0x1234), Target::IndirectARegDec(3), Size::Word)) },
    TestCase { cpu: M68kType::MC68000, data: &[0x0240, 0x1234],                     ins: Some(Instruction::AND(Target::Immediate(0x1234), Target::DirectDReg(0), Size::Word)) },
    TestCase { cpu: M68kType::MC68000, data: &[0x02A3, 0x1234, 0x5678],             ins: Some(Instruction::AND(Target::Immediate(0x12345678), Target::IndirectARegDec(3), Size::Long)) },
    TestCase { cpu: M68kType::MC68000, data: &[0x0280, 0x1234, 0x5678],             ins: Some(Instruction::AND(Target::Immediate(0x12345678), Target::DirectDReg(0), Size::Long)) },
    TestCase { cpu: M68kType::MC68000, data: &[0x023C, 0x1234],                     ins: Some(Instruction::ANDtoCCR(0x34)) },
    TestCase { cpu: M68kType::MC68000, data: &[0x027C, 0xF8FF],                     ins: Some(Instruction::ANDtoSR(0xF8FF)) },
    TestCase { cpu: M68kType::MC68000, data: &[0x4240],                             ins: Some(Instruction::CLR(Target::DirectDReg(0), Size::Word)) },
    TestCase { cpu: M68kType::MC68000, data: &[0x4280],                             ins: Some(Instruction::CLR(Target::DirectDReg(0), Size::Long)) },
    TestCase { cpu: M68kType::MC68000, data: &[0x4250],                             ins: Some(Instruction::CLR(Target::IndirectAReg(0), Size::Word)) },
    TestCase { cpu: M68kType::MC68000, data: &[0x4290],                             ins: Some(Instruction::CLR(Target::IndirectAReg(0), Size::Long)) },
    TestCase { cpu: M68kType::MC68000, data: &[0x0487, 0x1234, 0x5678],             ins: Some(Instruction::SUB(Target::Immediate(0x12345678), Target::DirectDReg(7), Size::Long)) },
    TestCase { cpu: M68kType::MC68000, data: &[0x063A, 0x1234, 0x0055],             ins: Some(Instruction::ADD(Target::Immediate(0x34), Target::IndirectRegOffset(BaseRegister::PC, None, 0x55), Size::Byte)) },
    TestCase { cpu: M68kType::MC68000, data: &[0x0A23, 0x1234],                     ins: Some(Instruction::EOR(Target::Immediate(0x34), Target::IndirectARegDec(3), Size::Byte)) },
    TestCase { cpu: M68kType::MC68000, data: &[0x0A3C, 0x1234],                     ins: Some(Instruction::EORtoCCR(0x34)) },
    TestCase { cpu: M68kType::MC68000, data: &[0x0A7C, 0xF8FF],                     ins: Some(Instruction::EORtoSR(0xF8FF)) },
    TestCase { cpu: M68kType::MC68000, data: &[0x0C00, 0x0020],                     ins: Some(Instruction::CMP(Target::Immediate(0x20), Target::DirectDReg(0), Size::Byte)) },
    TestCase { cpu: M68kType::MC68000, data: &[0x0C00, 0x0030],                     ins: Some(Instruction::CMP(Target::Immediate(0x30), Target::DirectDReg(0), Size::Byte)) },
    TestCase { cpu: M68kType::MC68000, data: &[0x0C00, 0x0010],                     ins: Some(Instruction::CMP(Target::Immediate(0x10), Target::DirectDReg(0), Size::Byte)) },
    TestCase { cpu: M68kType::MC68000, data: &[0x81FC, 0x0003],                     ins: Some(Instruction::DIVW(Target::Immediate(3), 0, Sign::Signed)) },
    TestCase { cpu: M68kType::MC68000, data: &[0xC1FC, 0x0276],                     ins: Some(Instruction::MULW(Target::Immediate(0x276), 0, Sign::Signed)) },
    TestCase { cpu: M68kType::MC68000, data: &[0xCDC5],                             ins: Some(Instruction::MULW(Target::DirectDReg(5), 6, Sign::Signed)) },
    TestCase { cpu: M68kType::MC68000, data: &[0x0108, 0x1234],                     ins: Some(Instruction::MOVEP(0, 0, 0x1234, Size::Word, Direction::FromTarget)) },
    TestCase { cpu: M68kType::MC68000, data: &[0x0148, 0x1234],                     ins: Some(Instruction::MOVEP(0, 0, 0x1234, Size::Long, Direction::FromTarget)) },
    TestCase { cpu: M68kType::MC68000, data: &[0x0188, 0x1234],                     ins: Some(Instruction::MOVEP(0, 0, 0x1234, Size::Word, Direction::ToTarget)) },
    TestCase { cpu: M68kType::MC68000, data: &[0x01C8, 0x1234],                     ins: Some(Instruction::MOVEP(0, 0, 0x1234, Size::Long, Direction::ToTarget)) },
    TestCase { cpu: M68kType::MC68000, data: &[0xE300],                             ins: Some(Instruction::ASL(Target::Immediate(1), Target::DirectDReg(0), Size::Byte)) },
    TestCase { cpu: M68kType::MC68000, data: &[0xE200],                             ins: Some(Instruction::ASR(Target::Immediate(1), Target::DirectDReg(0), Size::Byte)) },
    TestCase { cpu: M68kType::MC68000, data: &[0xE318],                             ins: Some(Instruction::ROL(Target::Immediate(1), Target::DirectDReg(0), Size::Byte)) },
    TestCase { cpu: M68kType::MC68000, data: &[0xE218],                             ins: Some(Instruction::ROR(Target::Immediate(1), Target::DirectDReg(0), Size::Byte)) },
    TestCase { cpu: M68kType::MC68000, data: &[0xA000],                             ins: Some(Instruction::UnimplementedA(0xA000)) },
    TestCase { cpu: M68kType::MC68000, data: &[0xFFFF],                             ins: Some(Instruction::UnimplementedF(0xFFFF)) },

    // MC68030
    TestCase { cpu: M68kType::MC68030, data: &[0x4C3C, 0x0800, 0x0000, 0x0097],                     ins: Some(Instruction::MULL(Target::Immediate(0x97), None, 0, Sign::Signed)) },
    TestCase { cpu: M68kType::MC68030, data: &[0x21BC, 0x0010, 0x14C4, 0x09B0, 0x0010, 0xDF40],     ins: Some(Instruction::MOVE(Target::Immediate(1053892), Target::IndirectRegOffset(BaseRegister::None, Some(IndexRegister { xreg: XRegister::DReg(0), scale: 0, size: Size::Long }), 0x10df40), Size::Long)) },

    // Should Fail
];


fn init_decode_test(cputype: M68kType) -> (M68k<Instant>, M68kCycle<Instant>, MemoryBlock<u32, Instant>) {
    // Insert basic initialization
    let len = 0x2000;
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

fn load_memory<Bus: BusAccess<u32, Instant = Instant>>(memory: &mut Bus, data: &[u16]) {
    let mut addr = INIT_ADDR;
    for word in data {
        memory.write_beu16(Bus::Instant::START, addr, *word).unwrap();
        addr += 2;
    }
}

fn run_decode_test(case: &TestCase) {
    let (mut cpu, cycle, mut memory) = init_decode_test(case.cpu);
    load_memory(&mut memory, case.data);

    match &case.ins {
        Some(ins) => {
            let mut executor = cycle.begin(&mut cpu, &mut memory);
            executor.reset_cpu().unwrap();
            assert_eq!(executor.state.pc, INIT_ADDR);
            assert_eq!(executor.state.ssp, INIT_STACK);
            executor.decode_next().unwrap();
            assert_eq!(executor.cycle.decoder.instruction, ins.clone());
        },
        None => {
            let mut executor = cycle.begin(&mut cpu, &mut memory);
            executor.reset_cpu().unwrap();
            assert_eq!(executor.state.pc, INIT_ADDR);
            assert_eq!(executor.state.ssp, INIT_STACK);
            let next = executor.decode_next();
            println!("{:?}", executor.cycle.decoder.instruction);
            assert!(next.is_err());
        },
    }
}

#[test]
pub fn run_decode_tests() {
    for case in DECODE_TESTS {
        println!("Testing for {:?}", case.ins);
        run_decode_test(case);
    }
}

#[test]
#[ignore]
pub fn run_assembler_tests() {
    let mut tests = 0;
    let mut errors = 0;

    for case in DECODE_TESTS {
        if case.ins.is_some() {
            tests += 1;
            let assembly_text = format!("{}", case.ins.as_ref().unwrap());
            print!("Testing assembling of {:?} ", assembly_text);
            let mut assembler = M68kAssembler::new(M68kType::MC68000);
            match assembler.assemble_words(&assembly_text) {
                Ok(data) => {
                    if data == case.data {
                        print!("pass");
                    } else {
                        errors += 1;
                        print!("FAILED");
                        print!("\nleft: {:?}, right: {:?}", data, case.data);
                    }
                    println!();
                },
                Err(err) => {
                    println!("FAILED\n{:?}", err);
                    errors += 1;
                },
            }
        }
    }

    if errors > 0 {
        panic!("{} errors out of {} tests", errors, tests);
    }
}


/*
#[test]
pub fn run_assembler_opcode_tests() {
    let mut tests = 0;
    let mut errors = 0;

    use super::super::testcases::{TimingCase, TIMING_TESTS};
    for case in TIMING_TESTS {
        tests += 1;
        let assembly_text = format!("{}", case.ins);
        print!("Testing assembling of {:?} from {:?}", assembly_text, case.ins);

        let mut assembler = M68kAssembler::new(M68kType::MC68000);
        match assembler.assemble_words(&assembly_text) {
            Ok(data) => {
                if data[0] == case.data[0] {
                    print!("pass");
                } else {
                    errors += 1;
                    print!("FAILED");
                    print!("\nleft: {:#06x}, right: {:#06x}", data[0], case.data[0]);
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
*/
