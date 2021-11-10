
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
        cpu.init(&system).unwrap();

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
        cpu.decode_next(&system).unwrap();
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
    use super::super::decode::{Instruction, LoadTarget, Target, RegisterPair, IndexRegister, IndexRegisterHalf, Condition};

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
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0100, de: 0x0000, hl: 0x0000, af: 0xFF90 },
        },
        TestCase {
            name: "adc with carry already set",
            ins: Instruction::ADCa(Target::DirectReg(Register::B)),
            data: &[ 0x88 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0xFE01 },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0xFF90 },
        },
        TestCase {
            name: "adc with carry already set while causing a carry",
            ins: Instruction::ADCa(Target::DirectReg(Register::B)),
            data: &[ 0x88 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0100, de: 0x0000, hl: 0x0000, af: 0xFE01 },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0100, de: 0x0000, hl: 0x0000, af: 0x0041 },
        },
        TestCase {
            name: "adc16 with bc",
            ins: Instruction::ADC16(RegisterPair::HL, RegisterPair::BC),
            data: &[ 0xED, 0x4A ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x1010, de: 0x0000, hl: 0x8080, af: 0x0000 },
            fini: TestState { pc: 0x0002, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x1010, de: 0x0000, hl: 0x9090, af: 0x0090 },
        },
        TestCase {
            name: "add a with h",
            ins: Instruction::ADDa(Target::DirectReg(Register::H)),
            data: &[ 0x84 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x2200, af: 0x1000 },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x2200, af: 0x3210 },
        },
        TestCase {
            name: "add a with h with overflow",
            ins: Instruction::ADDa(Target::DirectReg(Register::H)),
            data: &[ 0x84 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0100, af: 0x7F00 },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0100, af: 0x8084 },
        },
        TestCase {
            name: "add hl and bc",
            ins: Instruction::ADD16(RegisterPair::HL, RegisterPair::BC),
            data: &[ 0x09 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x1080, de: 0x0000, hl: 0x0080, af: 0x00FF },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x1080, de: 0x0000, hl: 0x1100, af: 0x00FC },
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
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x000F, de: 0x0000, hl: 0x0000, af: 0x00FF },
            fini: TestState { pc: 0x0002, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x000F, de: 0x0000, hl: 0x0000, af: 0x00BD },
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
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x00FC },
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
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x00F0, de: 0x0000, hl: 0x0000, af: 0x5503 },
        },
        TestCase {
            name: "cp c where not equal",
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
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0xAA12 },
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
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x00ff, de: 0x0000, hl: 0x0000, af: 0x0092 },
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
            name: "inc ix",
            ins: Instruction::INC16(RegisterPair::IX),
            data: &[ 0xDD, 0x23 ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x00FF },
            fini: TestState { pc: 0x0002, sp: 0x0000, ix: 0x0001, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x00FF },
        },
        TestCase {
            name: "inc c",
            ins: Instruction::INC8(Target::DirectReg(Register::C)),
            data: &[ 0x0C ],
            init: TestState { pc: 0x0000, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0000, de: 0x0000, hl: 0x0000, af: 0x00FF },
            fini: TestState { pc: 0x0001, sp: 0x0000, ix: 0x0000, iy: 0x0000, bc: 0x0001, de: 0x0000, hl: 0x0000, af: 0x0001 },
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
        cpu.init(&system).unwrap();

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
        let expected_state = build_state(&case.fini);

        load_memory(&system, case.data);
        cpu.state = init_state;

        cpu.decode_next(&system).unwrap();
        assert_eq!(cpu.decoder.instruction, case.ins);

        cpu.execute_current(&system).unwrap();
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

