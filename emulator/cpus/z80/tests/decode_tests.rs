use femtos::{Instant, Frequency};

use emulator_hal::{BusAccess, Step};
use emulator_hal_memory::MemoryBlock;

use moa_z80::{Z80, Z80Type, Z80Decoder, Instruction, LoadTarget, Target, Register, RegisterPair, IndexRegister, IndexRegisterHalf};

fn init_decode_test() -> (Z80<Instant>, MemoryBlock<Instant>) {
    // Insert basic initialization
    let len = 0x10_0000;
    let mut data = Vec::with_capacity(len);
    unsafe {
        data.set_len(len);
    }
    let mut memory = MemoryBlock::from(data);

    // Initialize the CPU and make sure it's in the expected state
    let mut cpu = Z80::new(Z80Type::Z80, Frequency::from_mhz(4));
    cpu.reset(Instant::START, &mut memory).unwrap();
    cpu.step(Instant::START, &mut memory).unwrap();

    (cpu, memory)
}

fn load_memory(memory: &mut MemoryBlock<Instant>, data: &[u8]) {
    for i in 0..data.len() {
        memory.write_u8(Instant::START, i, data[i]).unwrap();
    }
}

fn run_decode_test(data: &[u8]) -> Instruction {
    let (mut cpu, mut memory) = init_decode_test();
    load_memory(&mut memory, data);
    cpu.step(Instant::START, &mut memory).unwrap();
    cpu.previous_cycle.decoder.instruction
}

#[test]
fn run_all_decode_tests() {
    let mut failures = vec![];

    for (data, expected_instruction) in DECODE_TESTS {
        let instruction = run_decode_test(data);
        if instruction != *expected_instruction {
            failures.push((data, instruction, expected_instruction));
        }
    }

    let fails = failures.len();
    for (data, instruction, expected_instruction) in failures {
        println!("for {:?}\nexpected:\t{:?}\nreceived:\t{:?}\n", data, instruction, expected_instruction);
    }

    if fails > 0 {
        panic!("{} decode tests failed", fails);
    }
}

#[rustfmt::skip]
const DECODE_TESTS: &'static [(&[u8], Instruction)] = &[
    (&[0x00],               Instruction::NOP),
    (&[0x01, 0x01, 0x02],   Instruction::LD(LoadTarget::DirectRegWord(RegisterPair::BC), LoadTarget::ImmediateWord(0x0201))),
    (&[0x02],               Instruction::LD(LoadTarget::IndirectRegByte(RegisterPair::BC), LoadTarget::DirectRegByte(Register::A))),
    (&[0x03],               Instruction::INC16(RegisterPair::BC)),
    (&[0x04],               Instruction::INC8(Target::DirectReg(Register::B))),
    (&[0x05],               Instruction::DEC8(Target::DirectReg(Register::B))),

    (&[0xDD, 0x09],         Instruction::ADD16(RegisterPair::IX, RegisterPair::BC)),
    (&[0xDD, 0x44],         Instruction::LD(LoadTarget::DirectRegByte(Register::B), LoadTarget::DirectRegHalfByte(IndexRegisterHalf::IXH))),
    (&[0xDD, 0x66, 0x12],   Instruction::LD(LoadTarget::DirectRegByte(Register::H), LoadTarget::IndirectOffsetByte(IndexRegister::IX, 0x12))),
    (&[0xDD, 0x6E, 0x12],   Instruction::LD(LoadTarget::DirectRegByte(Register::L), LoadTarget::IndirectOffsetByte(IndexRegister::IX, 0x12))),
    (&[0xDD, 0x84],         Instruction::ADDa(Target::DirectRegHalf(IndexRegisterHalf::IXH))),
    (&[0xDD, 0x85],         Instruction::ADDa(Target::DirectRegHalf(IndexRegisterHalf::IXL))),
];
