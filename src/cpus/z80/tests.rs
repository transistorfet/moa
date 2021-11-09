
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

    /*
    #[test]
    fn decode_add_ix_bc() {
        let instruction = run_decode_test(&[0xDD, 0x09]);
        assert_eq!(instruction, Instruction::ADD16(RegisterPair::IX, RegisterPair::BC));
    }

    #[test]
    fn decode_ld_b_ixh() {
        let instruction = run_decode_test(&[0xDD, 0x44]);
        assert_eq!(instruction, Instruction::LD(LoadTarget::DirectRegByte(Register::B), LoadTarget::DirectRegHalfByte(IndexRegisterHalf::IXH)));
    }

    #[test]
    fn decode_ld_h_ix_offset() {
        let instruction = run_decode_test(&[0xDD, 0x66, 0x12]);
        assert_eq!(instruction, Instruction::LD(LoadTarget::DirectRegByte(Register::H), LoadTarget::IndirectOffsetByte(IndexRegister::IX, 0x12)));
    }

    #[test]
    fn decode_ld_l_ix_offset() {
        let instruction = run_decode_test(&[0xDD, 0x6E, 0x12]);
        assert_eq!(instruction, Instruction::LD(LoadTarget::DirectRegByte(Register::L), LoadTarget::IndirectOffsetByte(IndexRegister::IX, 0x12)));
    }

    #[test]
    fn decode_add_ixh() {
        let instruction = run_decode_test(&[0xDD, 0x84]);
        assert_eq!(instruction, Instruction::ADDa(Target::DirectRegHalf(IndexRegisterHalf::IXH)));
    }

    #[test]
    fn decode_add_ixl() {
        let instruction = run_decode_test(&[0xDD, 0x85]);
        assert_eq!(instruction, Instruction::ADDa(Target::DirectRegHalf(IndexRegisterHalf::IXL)));
    }
    */
}


#[cfg(test)]
mod execute_tests {
    use crate::system::System;
    use crate::memory::{MemoryBlock, BusPort};
    use crate::devices::{Address, Addressable, wrap_transmutable};

    use super::super::{Z80, Z80Type};
    use super::super::state::{Z80State, Register};
    use super::super::decode::{Instruction, LoadTarget, Target, RegisterPair, IndexRegister, IndexRegisterHalf, Condition};

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

    fn run_execute_test(init_state: Z80State, expected_state: Z80State, instruction: Instruction) {
        let (mut cpu, system) = init_execute_test();

        cpu.state = init_state;
        cpu.decoder.instruction = instruction;

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state, expected_state);
    }


    /////////////////////
    // Execution Tests //
    /////////////////////


    #[test]
    fn execute_adca_b_carry_clear() {
        let mut init_state = Z80State::new();
        init_state.set_register(Register::A, 0xFE);
        init_state.set_register(Register::B, 0x01);
        init_state.set_register(Register::F, 0x00);

        let mut expected_state = init_state.clone();
        expected_state.set_register(Register::A, 0xFF);
        expected_state.set_register(Register::F, 0x90);

        run_execute_test(init_state, expected_state, Instruction::ADCa(Target::DirectReg(Register::B)));
    }

    #[test]
    fn execute_adca_b_carry_set() {
        let mut init_state = Z80State::new();
        init_state.set_register(Register::A, 0xFE);
        init_state.set_register(Register::B, 0x00);
        init_state.set_register(Register::F, 0x01);

        let mut expected_state = init_state.clone();
        expected_state.set_register(Register::A, 0xFF);
        expected_state.set_register(Register::F, 0x90);

        run_execute_test(init_state, expected_state, Instruction::ADCa(Target::DirectReg(Register::B)));
    }

    #[test]
    fn execute_adca_b_carry_set_with_carry() {
        let mut init_state = Z80State::new();
        init_state.set_register(Register::A, 0xFE);
        init_state.set_register(Register::B, 0x01);
        init_state.set_register(Register::F, 0x01);

        let mut expected_state = init_state.clone();
        expected_state.set_register(Register::A, 0x00);
        expected_state.set_register(Register::F, 0x41);

        run_execute_test(init_state, expected_state, Instruction::ADCa(Target::DirectReg(Register::B)));
    }

    #[test]
    fn execute_adc16_ixl() {
        let mut init_state = Z80State::new();
        init_state.set_register(Register::H, 0x80);
        init_state.set_register(Register::L, 0x80);
        init_state.ix = 0x1010;

        let mut expected_state = init_state.clone();
        expected_state.set_register(Register::H, 0x90);
        expected_state.set_register(Register::L, 0x90);
        expected_state.set_register(Register::F, 0x90);

        run_execute_test(init_state, expected_state, Instruction::ADC16(RegisterPair::HL, RegisterPair::IX));
    }

    #[test]
    fn execute_adda_h() {
        let mut init_state = Z80State::new();
        init_state.set_register(Register::A, 0x10);
        init_state.set_register(Register::H, 0x22);

        let mut expected_state = init_state.clone();
        expected_state.set_register(Register::A, 0x32);
        expected_state.set_register(Register::F, 0x10);

        run_execute_test(init_state, expected_state, Instruction::ADDa(Target::DirectReg(Register::H)));
    }

    #[test]
    fn execute_adda_h_with_overflow() {
        let mut init_state = Z80State::new();
        init_state.set_register(Register::A, 0x7F);
        init_state.set_register(Register::H, 0x01);

        let mut expected_state = init_state.clone();
        expected_state.set_register(Register::A, 0x80);
        expected_state.set_register(Register::F, 0x84);

        run_execute_test(init_state, expected_state, Instruction::ADDa(Target::DirectReg(Register::H)));
    }

    #[test]
    fn execute_add16_ixl() {
        let mut init_state = Z80State::new();
        init_state.ix = 0x1080;
        init_state.set_register(Register::H, 0x00);
        init_state.set_register(Register::L, 0x80);
        init_state.set_register(Register::F, 0xFF);     // S and Z flags should not be affected

        let mut expected_state = init_state.clone();
        expected_state.set_register(Register::H, 0x11);
        expected_state.set_register(Register::L, 0x00);
        expected_state.set_register(Register::F, 0xFC);

        run_execute_test(init_state, expected_state, Instruction::ADD16(RegisterPair::HL, RegisterPair::IX));
    }

    #[test]
    fn execute_and_c() {
        let mut init_state = Z80State::new();
        init_state.set_register(Register::A, 0x55);
        init_state.set_register(Register::C, 0xF0);
        init_state.set_register(Register::F, 0xFF);

        let mut expected_state = init_state.clone();
        expected_state.set_register(Register::A, 0x50);
        expected_state.set_register(Register::F, 0x14);

        run_execute_test(init_state, expected_state, Instruction::AND(Target::DirectReg(Register::C)));
    }

    #[test]
    fn execute_bit() {
        let mut init_state = Z80State::new();
        init_state.set_register(Register::C, 0x0F);
        init_state.set_register(Register::F, 0xFF);

        let mut expected_state = init_state.clone();
        expected_state.set_register(Register::F, 0xBD);

        run_execute_test(init_state, expected_state, Instruction::BIT(3, Target::DirectReg(Register::C)));
    }

    #[test]
    fn execute_call() {
        let mut init_state = Z80State::new();

        let mut expected_state = init_state.clone();
        expected_state.pc = 0x1234;
        expected_state.sp = 0xFFFE;

        run_execute_test(init_state, expected_state, Instruction::CALL(0x1234));
    }

    #[test]
    fn execute_call_cc_true() {
        let mut init_state = Z80State::new();
        init_state.set_register(Register::F, 0xFF);

        let mut expected_state = init_state.clone();
        expected_state.pc = 0x1234;
        expected_state.sp = 0xFFFE;

        run_execute_test(init_state, expected_state, Instruction::CALLcc(Condition::Zero, 0x1234));
    }

    #[test]
    fn execute_call_cc_false() {
        let mut init_state = Z80State::new();
        init_state.set_register(Register::F, 0xFF);

        let expected_state = init_state.clone();

        run_execute_test(init_state, expected_state, Instruction::CALLcc(Condition::NotZero, 0x1234));
    }

    #[test]
    fn execute_ccf() {
        let mut init_state = Z80State::new();
        init_state.set_register(Register::F, 0xFF);

        let mut expected_state = init_state.clone();
        expected_state.set_register(Register::F, 0xFC);

        run_execute_test(init_state, expected_state, Instruction::CCF);
    }

    #[test]
    fn execute_ccf_invert() {
        let mut init_state = Z80State::new();
        init_state.set_register(Register::F, 0x00);

        let mut expected_state = init_state.clone();
        expected_state.set_register(Register::F, 0x01);

        run_execute_test(init_state, expected_state, Instruction::CCF);
    }

    #[test]
    fn execute_cp_c_diff() {
        let mut init_state = Z80State::new();
        init_state.set_register(Register::A, 0x55);
        init_state.set_register(Register::C, 0xF0);
        init_state.set_register(Register::F, 0xFF);

        let mut expected_state = init_state.clone();
        expected_state.set_register(Register::F, 0x03);

        run_execute_test(init_state, expected_state, Instruction::CP(Target::DirectReg(Register::C)));
    }

    #[test]
    fn execute_cp_c_equal() {
        let mut init_state = Z80State::new();
        init_state.set_register(Register::A, 0x55);
        init_state.set_register(Register::C, 0x55);
        init_state.set_register(Register::F, 0xFF);

        let mut expected_state = init_state.clone();
        expected_state.set_register(Register::F, 0x42);

        run_execute_test(init_state, expected_state, Instruction::CP(Target::DirectReg(Register::C)));
    }

    #[test]
    fn execute_cpl() {
        let mut init_state = Z80State::new();
        init_state.set_register(Register::A, 0x55);
        init_state.set_register(Register::F, 0x00);

        let mut expected_state = init_state.clone();
        expected_state.set_register(Register::A, 0xAA);
        expected_state.set_register(Register::F, 0x12);

        run_execute_test(init_state, expected_state, Instruction::CPL);
    }
}

