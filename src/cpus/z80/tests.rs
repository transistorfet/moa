
#[cfg(test)]
mod decode_tests {
    use crate::system::System;
    use crate::memory::{MemoryBlock, BusPort};
    use crate::devices::{Address, Addressable, wrap_transmutable};

    use super::super::{Z80, Z80Type};
    use super::super::state::Register;
    use super::super::decode::{Instruction, LoadTarget, Target, RegisterPair, IndexRegister, IndexRegisterHalf};

    fn init_decode_test() -> (Z80, System) {
        let mut system = System::new();

        // Insert basic initialization
        let data = vec![0; 0x10000];
        let mem = MemoryBlock::new(data);
        system.add_addressable_device(0x0000, wrap_transmutable(mem)).unwrap();

        // Initialize the CPU and make sure it's in the expected state
        let mut cpu = Z80::new(Z80Type::Z80, 4_000_000, BusPort::new(0, 16, 8, system.bus.clone()));
        cpu.init().unwrap();

        (cpu, system)
    }

    fn load_memory(system: &System, data: &[u8]) {
        for i in 0..data.len() {
            system.get_bus().write_u8(i as Address, data[i]).unwrap();
        }
    }

    fn run_decode_test(data: &[u8]) -> Instruction {
        let (mut cpu, system) = init_decode_test();
        load_memory(&system, data);
        cpu.decode_next().unwrap();
        cpu.decoder.instruction
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
}


#[cfg(test)]
mod execute_tests {
    use crate::system::System;
    use crate::memory::{MemoryBlock, BusPort};
    use crate::devices::{Address, Addressable, wrap_transmutable};

    use super::super::{Z80, Z80Type};
    use super::super::state::{Z80State, Register};
    use super::super::decode::{Instruction, LoadTarget, Target, RegisterPair, Condition};

    struct TestState {
        pc: u16,
        sp: u16,
        ix: u16,
        iy: u16,
        bc: u16,
        de: u16,
        hl: u16,
        af: u16,
    }

    struct TestCase {
        name: &'static str,
        ins: Instruction,
        data: &'static [u8],
        init: TestState,
        fini: TestState,
    }

    const TEST_CASES: &'static [TestCase] = &[
        /*
        TestCase {
            name: ,
            ins: ,
            data: &[ 0x88 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0000 },
            fini: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0000 },
        },
        */

        TestCase {
            name: "adc with no carry",
            ins: Instruction::ADCa(Target::DirectReg(Register::B)),
            data: &[ 0x88 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0100, de: 0x0000, hl: 0x0000, af: 0xFE00 },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0100, de: 0x0000, hl: 0x0000, af: 0xFFA8 },
        },
        TestCase {
            name: "adc with carry already set",
            ins: Instruction::ADCa(Target::DirectReg(Register::B)),
            data: &[ 0x88 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0xFE01 },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0xFFA8 },
        },
        TestCase {
            name: "adc with carry already set while causing a carry",
            ins: Instruction::ADCa(Target::DirectReg(Register::B)),
            data: &[ 0x88 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0100, de: 0x0000, hl: 0x0000, af: 0xFE01 },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0100, de: 0x0000, hl: 0x0000, af: 0x0051 },
        },
        TestCase {
            name: "adc16 with bc",
            ins: Instruction::ADC16(RegisterPair::HL, RegisterPair::BC),
            data: &[ 0xED, 0x4A ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x1010, de: 0x0000, hl: 0x8080, af: 0x0000 },
            fini: TestState { pc: 0x0002, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x1010, de: 0x0000, hl: 0x9090, af: 0x0080 },
        },
        TestCase {
            name: "add a with h",
            ins: Instruction::ADDa(Target::DirectReg(Register::H)),
            data: &[ 0x84 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x2200, af: 0x1000 },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x2200, af: 0x3220 },
        },
        TestCase {
            name: "add a with h with overflow",
            ins: Instruction::ADDa(Target::DirectReg(Register::H)),
            data: &[ 0x84 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0100, af: 0x7F00 },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0100, af: 0x8094 },
        },
        TestCase {
            name: "add hl and bc",
            ins: Instruction::ADD16(RegisterPair::HL, RegisterPair::BC),
            data: &[ 0x09 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x1080, de: 0x0000, hl: 0x0080, af: 0x00FF },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x1080, de: 0x0000, hl: 0x1100, af: 0x00C4 },
        },
        TestCase {
            name: "and with c",
            ins: Instruction::AND(Target::DirectReg(Register::C)),
            data: &[ 0xA1 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x00F0, de: 0x0000, hl: 0x0000, af: 0x55FF },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x00F0, de: 0x0000, hl: 0x0000, af: 0x5014 },
        },
        TestCase {
            name: "bit 3, c",
            ins: Instruction::BIT(3, Target::DirectReg(Register::C)),
            data: &[ 0xCB, 0x59 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x000F, de: 0x0000, hl: 0x0000, af: 0x0043 },
            fini: TestState { pc: 0x0002, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x000F, de: 0x0000, hl: 0x0000, af: 0x0019 },
        },
        TestCase {
            name: "call",
            ins: Instruction::CALL(0x1234),
            data: &[ 0xCD, 0x34, 0x12 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0000 },
            fini: TestState { pc: 0x1234, sp: 0xFFFE, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0000 },
        },
        TestCase {
            name: "call cc true",
            ins: Instruction::CALLcc(Condition::Zero, 0x1234),
            data: &[ 0xCC, 0x34, 0x12 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x00FF },
            fini: TestState { pc: 0x1234, sp: 0xFFFE, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x00FF },
        },
        TestCase {
            name: "call cc false",
            ins: Instruction::CALLcc(Condition::Zero, 0x1234),
            data: &[ 0xCC, 0x34, 0x12 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0000 },
            fini: TestState { pc: 0x0003, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0000 },
        },
        TestCase {
            name: "ccf",
            ins: Instruction::CCF,
            data: &[ 0x3F ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x00FF },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x00D4 },
        },
        TestCase {
            name: "ccf invert",
            ins: Instruction::CCF,
            data: &[ 0x3F ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0000 },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0001 },
        },
        TestCase {
            name: "cp c where not equal",
            ins: Instruction::CP(Target::DirectReg(Register::C)),
            data: &[ 0xB9 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x00F0, de: 0x0000, hl: 0x0000, af: 0x55FF },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x00F0, de: 0x0000, hl: 0x0000, af: 0x5523 },
        },
        TestCase {
            name: "cp c where equal",
            ins: Instruction::CP(Target::DirectReg(Register::C)),
            data: &[ 0xB9 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0055, de: 0x0000, hl: 0x0000, af: 0x55FF },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0055, de: 0x0000, hl: 0x0000, af: 0x5542 },
        },
        TestCase {
            name: "cpl",
            ins: Instruction::CPL,
            data: &[ 0x2F ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x5500 },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0xAA3A },
        },
        TestCase {
            name: "dec hl",
            ins: Instruction::DEC16(RegisterPair::HL),
            data: &[ 0x2B ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x00FF },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0xFFFF, af: 0x00FF },
        },
        TestCase {
            name: "dec8",
            ins: Instruction::DEC8(Target::DirectReg(Register::C)),
            data: &[ 0x0D ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0000 },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x00ff, de: 0x0000, hl: 0x0000, af: 0x00BA },
        },
        TestCase {
            name: "djnz with jump",
            ins: Instruction::DJNZ(0x10),
            data: &[ 0x10, 0x10 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x00FF },
            fini: TestState { pc: 0x0012, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0xff00, de: 0x0000, hl: 0x0000, af: 0x00FF },
        },
        TestCase {
            name: "djnz without jump",
            ins: Instruction::DJNZ(0x10),
            data: &[ 0x10, 0x10 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0100, de: 0x0000, hl: 0x0000, af: 0x00FF },
            fini: TestState { pc: 0x0002, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x00FF },
        },
        TestCase {
            name: "ex de, hl",
            ins: Instruction::EXhlde,
            data: &[ 0xEB ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x55AA, hl: 0x1234, af: 0x00FF },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x1234, hl: 0x55AA, af: 0x00FF },
        },
        //TestCase {
        //    name: "ex sp location",
        //    ins: Instruction::EXhlde,
        //    data: &[ 0xEB ],
        //    init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x55AA, hl: 0x1234, af: 0x00FF },
        //    fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x1234, hl: 0x55AA, af: 0x00FF },
        //},

        TestCase {
            name: "inc hl",
            ins: Instruction::INC16(RegisterPair::HL),
            data: &[ 0x23 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x00FF },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0001, af: 0x00FF },
        },
        TestCase {
            name: "inc c",
            ins: Instruction::INC8(Target::DirectReg(Register::C)),
            data: &[ 0x0C ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x00FF },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0001, de: 0x0000, hl: 0x0000, af: 0x0001 },
        },
        TestCase {
            name: "jp",
            ins: Instruction::JP(0x1234),
            data: &[ 0xC3, 0x34, 0x12 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0000 },
            fini: TestState { pc: 0x1234, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0000 },
        },
        TestCase {
            name: "jp indirect (HL)",
            ins: Instruction::JPIndirect(RegisterPair::HL),
            data: &[ 0xE9 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x1234, af: 0x0000 },
            fini: TestState { pc: 0x1234, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x1234, af: 0x0000 },
        },
        TestCase {
            name: "jp with true case",
            ins: Instruction::JPcc(Condition::NotCarry, 0x1234),
            data: &[ 0xD2, 0x34, 0x12 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0000 },
            fini: TestState { pc: 0x1234, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0000 },
        },
        TestCase {
            name: "jp with false case",
            ins: Instruction::JPcc(Condition::ParityEven, 0x1234),
            data: &[ 0xEA, 0x34, 0x12 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0000 },
            fini: TestState { pc: 0x0003, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0000 },
        },
        TestCase {
            name: "jr",
            ins: Instruction::JR(16),
            data: &[ 0x18, 0x10 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0000 },
            fini: TestState { pc: 0x0012, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0000 },
        },
        TestCase {
            name: "jr with true case",
            ins: Instruction::JRcc(Condition::Zero, 16),
            data: &[ 0x28, 0x10 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0040 },
            fini: TestState { pc: 0x0012, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0040 },
        },
        TestCase {
            name: "jr with false case",
            ins: Instruction::JRcc(Condition::Zero, 16),
            data: &[ 0x28, 0x10 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0000 },
            fini: TestState { pc: 0x0002, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0000 },
        },
        TestCase {
            name: "ld a, b",
            ins: Instruction::LD(LoadTarget::DirectRegByte(Register::A), LoadTarget::DirectRegByte(Register::B)),
            data: &[ 0x78 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0xFF00, de: 0x0000, hl: 0x0000, af: 0x0000 },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0xFF00, de: 0x0000, hl: 0x0000, af: 0xFF00 },
        },
        TestCase {
            name: "ld a, (hl)",
            ins: Instruction::LD(LoadTarget::DirectRegByte(Register::A), LoadTarget::IndirectRegByte(RegisterPair::HL)),
            data: &[ 0x7E ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0000 },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x7E00 },
        },
        TestCase {
            name: "ld a, (**)",
            ins: Instruction::LD(LoadTarget::DirectRegByte(Register::A), LoadTarget::IndirectByte(0)),
            data: &[ 0x3A, 0x00, 0x00 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0000 },
            fini: TestState { pc: 0x0003, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x3A00 },
        },
        TestCase {
            name: "ldir counting",
            ins: Instruction::LDIR,
            data: &[ 0xED, 0xB0 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0002, de: 0x00FF, hl: 0x0000, af: 0x0000 },
            fini: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0001, de: 0x0100, hl: 0x0001, af: 0x000C },
        },
        TestCase {
            name: "ldir terminating",
            ins: Instruction::LDIR,
            data: &[ 0xED, 0xB0 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0001, de: 0x00FF, hl: 0x0000, af: 0x0000 },
            fini: TestState { pc: 0x0002, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0100, hl: 0x0001, af: 0x0008 },
        },
        TestCase {
            name: "neg",
            ins: Instruction::NEG,
            data: &[ 0xED, 0x44 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x5500 },
            fini: TestState { pc: 0x0002, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0xABBB },
        },
        TestCase {
            name: "or",
            ins: Instruction::OR(Target::DirectReg(Register::B)),
            data: &[ 0xB0 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0xAA00, de: 0x0000, hl: 0x0000, af: 0x5500 },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0xAA00, de: 0x0000, hl: 0x0000, af: 0xFFAC },
        },
        TestCase {
            name: "pop bc",
            ins: Instruction::POP(RegisterPair::BC),
            data: &[ 0xC1 ],
            init: TestState { pc: 0x0000, sp: 0x40FE, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0000 },
            fini: TestState { pc: 0x0001, sp: 0x4100, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0000 },
        },
        TestCase {
            name: "push bc",
            ins: Instruction::PUSH(RegisterPair::BC),
            data: &[ 0xC5 ],
            init: TestState { pc: 0x0000, sp: 0x4100, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0000 },
            fini: TestState { pc: 0x0001, sp: 0x40FE, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0000 },
        },
        TestCase {
            name: "res 0, a",
            ins: Instruction::RES(0, Target::DirectReg(Register::A), None),
            data: &[ 0xCB, 0x87 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0xFF00 },
            fini: TestState { pc: 0x0002, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0xFE00 },
        },
        TestCase {
            name: "rl",
            ins: Instruction::RL(Target::DirectReg(Register::B), None),
            data: &[ 0xCB, 0x10 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x8000, de: 0x0000, hl: 0x0000, af: 0x0000 },
            fini: TestState { pc: 0x0002, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0045 },
        },
        TestCase {
            name: "rlc",
            ins: Instruction::RLC(Target::DirectReg(Register::B), None),
            data: &[ 0xCB, 0x00 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x8000, de: 0x0000, hl: 0x0000, af: 0x0000 },
            fini: TestState { pc: 0x0002, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0100, de: 0x0000, hl: 0x0000, af: 0x0001 },
        },
       TestCase {
            name: "rr",
            ins: Instruction::RR(Target::DirectReg(Register::B), None),
            data: &[ 0xCB, 0x18 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0100, de: 0x0000, hl: 0x0000, af: 0x0000 },
            fini: TestState { pc: 0x0002, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0045 },
        },
        TestCase {
            name: "rrc",
            ins: Instruction::RRC(Target::DirectReg(Register::B), None),
            data: &[ 0xCB, 0x08 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0100, de: 0x0000, hl: 0x0000, af: 0x0000 },
            fini: TestState { pc: 0x0002, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x8000, de: 0x0000, hl: 0x0000, af: 0x0081 },
        },
        TestCase {
            name: "rla",
            ins: Instruction::RLA,
            data: &[ 0x17 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x8000 },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0001 },
        },
        TestCase {
            name: "rlca",
            ins: Instruction::RLCA,
            data: &[ 0x07 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x8000 },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0101 },
        },
       TestCase {
            name: "rra",
            ins: Instruction::RRA,
            data: &[ 0x1F ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0100 },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0001 },
        },
        TestCase {
            name: "rrca",
            ins: Instruction::RRCA,
            data: &[ 0x0F ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0100 },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x8001 },
        },
        TestCase {
            name: "rst",
            ins: Instruction::RST(0x10),
            data: &[ 0xD7 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0000 },
            fini: TestState { pc: 0x0010, sp: 0xFFFE, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0000 },
        },
        TestCase {
            name: "sbc with no carry",
            ins: Instruction::SBCa(Target::DirectReg(Register::B)),
            data: &[ 0x98 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0100, de: 0x0000, hl: 0x0000, af: 0x0100 },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0100, de: 0x0000, hl: 0x0000, af: 0x0042 },
        },
        TestCase {
            name: "sbc with carry already set",
            ins: Instruction::SBCa(Target::DirectReg(Register::B)),
            data: &[ 0x98 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0101 },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0042 },
        },
        TestCase {
            name: "sbc with carry already set while causing a carry",
            ins: Instruction::SBCa(Target::DirectReg(Register::B)),
            data: &[ 0x98 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0100, de: 0x0000, hl: 0x0000, af: 0x0101 },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0100, de: 0x0000, hl: 0x0000, af: 0xFFBB },
        },
        TestCase {
            name: "sbc16 with bc",
            ins: Instruction::SBC16(RegisterPair::HL, RegisterPair::BC),
            data: &[ 0xED, 0x42 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x1010, de: 0x0000, hl: 0x9090, af: 0x0000 },
            fini: TestState { pc: 0x0002, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x1010, de: 0x0000, hl: 0x8080, af: 0x0082 },
        },
        TestCase {
            name: "scf",
            ins: Instruction::SCF,
            data: &[ 0x37 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x00FE },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x00C5 },
        },
        TestCase {
            name: "set 0, a",
            ins: Instruction::SET(0, Target::DirectReg(Register::A), None),
            data: &[ 0xCB, 0xC7 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0000 },
            fini: TestState { pc: 0x0002, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x0100 },
        },
        TestCase {
            name: "sla",
            ins: Instruction::SLA(Target::DirectReg(Register::B), None),
            data: &[ 0xCB, 0x20 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x5500, de: 0x0000, hl: 0x0000, af: 0x0000 },
            fini: TestState { pc: 0x0002, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0xAA00, de: 0x0000, hl: 0x0000, af: 0x00AC },
        },
        TestCase {
            name: "sll",
            ins: Instruction::SLL(Target::DirectReg(Register::B), None),
            data: &[ 0xCB, 0x30 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x5500, de: 0x0000, hl: 0x0000, af: 0x0000 },
            fini: TestState { pc: 0x0002, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0xAB00, de: 0x0000, hl: 0x0000, af: 0x00A8 },
        },
        TestCase {
            name: "sra",
            ins: Instruction::SRA(Target::DirectReg(Register::B), None),
            data: &[ 0xCB, 0x28 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0xAA00, de: 0x0000, hl: 0x0000, af: 0x0000 },
            fini: TestState { pc: 0x0002, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0xD500, de: 0x0000, hl: 0x0000, af: 0x0080 },
        },
        TestCase {
            name: "srl",
            ins: Instruction::SRL(Target::DirectReg(Register::B), None),
            data: &[ 0xCB, 0x38 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0xAA00, de: 0x0000, hl: 0x0000, af: 0x0000 },
            fini: TestState { pc: 0x0002, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x5500, de: 0x0000, hl: 0x0000, af: 0x0004 },
        },
        TestCase {
            name: "sub a with h with overflow",
            ins: Instruction::SUB(Target::DirectReg(Register::H)),
            data: &[ 0x94 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0100, af: 0x8000 },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0100, af: 0x7F3E },
        },
        TestCase {
            name: "xor",
            ins: Instruction::XOR(Target::DirectReg(Register::B)),
            data: &[ 0xA8 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0xAA00, de: 0x0000, hl: 0x0000, af: 0xFF00 },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0xAA00, de: 0x0000, hl: 0x0000, af: 0x5504 },
        },

    ];

    fn init_execute_test() -> (Z80, System) {
        let mut system = System::new();

        // Insert basic initialization
        let data = vec![0; 0x10000];
        let mem = MemoryBlock::new(data);
        system.add_addressable_device(0x0000, wrap_transmutable(mem)).unwrap();

        // Initialize the CPU and make sure it's in the expected state
        let mut cpu = Z80::new(Z80Type::Z80, 4_000_000, BusPort::new(0, 16, 8, system.bus.clone()));
        cpu.init().unwrap();

        (cpu, system)
    }

    fn build_state(state: &TestState) -> Z80State {
        let mut new_state = Z80State::new();
        new_state.pc = state.pc;
        new_state.sp = state.sp;
        new_state.ix = state.ix;
        new_state.iy = state.iy;
        new_state.set_register(Register::B, (state.bc >> 8) as u8);
        new_state.set_register(Register::C, state.bc as u8);
        new_state.set_register(Register::D, (state.de >> 8) as u8);
        new_state.set_register(Register::E, state.de as u8);
        new_state.set_register(Register::H, (state.hl >> 8) as u8);
        new_state.set_register(Register::L, state.hl as u8);
        new_state.set_register(Register::A, (state.af >> 8) as u8);
        new_state.set_register(Register::F, state.af as u8);
        new_state
    }

    fn load_memory(system: &System, data: &[u8]) {
        for i in 0..data.len() {
            system.get_bus().write_u8(i as Address, data[i]).unwrap();
        }
    }

    fn run_test(case: &TestCase) {
        let (mut cpu, system) = init_execute_test();

        let init_state = build_state(&case.init);
        let mut expected_state = build_state(&case.fini);

        load_memory(&system, case.data);
        cpu.state = init_state;

        cpu.decode_next().unwrap();
        assert_eq!(cpu.decoder.instruction, case.ins);

        cpu.execute_current().unwrap();

        // TODO this is a hack to ignore the functioning of the F5, F3 flags for now
        cpu.state.reg[Register::F as usize] &= 0xD7;
        expected_state.reg[Register::F as usize] &= 0xD7;

        assert_eq!(cpu.state, expected_state);
    }

    #[test]
    pub fn run_execute_tests() {
        for case in TEST_CASES {
            println!("Running test {}", case.name);
            run_test(case);
        }
    }
}

