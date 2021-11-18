
#[cfg(test)]
mod decode_tests {
    use crate::error::{Error, ErrorType};
    use crate::system::System;
    use crate::memory::{MemoryBlock, BusPort};
    use crate::devices::{Address, Addressable, wrap_transmutable};

    use crate::cpus::m68k::{M68k, M68kType};
    use crate::cpus::m68k::state::Exceptions;
    use crate::cpus::m68k::instructions::{Instruction, Target, Size, Sign, XRegister, BaseRegister, IndexRegister, ShiftDirection};

    const INIT_STACK: Address = 0x00002000;
    const INIT_ADDR: Address = 0x00000010;

    struct TestCase {
        cpu: M68kType,
        data: &'static [u16],
        ins: Option<Instruction>,
    }

    const DECODE_TESTS: &'static [TestCase] = &[
        // MC68000
        TestCase { cpu: M68kType::MC68000, data: &[0x4e71],                             ins: Some(Instruction::NOP) },
        TestCase { cpu: M68kType::MC68000, data: &[0x0008, 0x00FF],                     ins: Some(Instruction::OR(Target::Immediate(0xFF), Target::DirectAReg(0), Size::Byte)) },
        TestCase { cpu: M68kType::MC68000, data: &[0x003C, 0x00FF],                     ins: Some(Instruction::ORtoCCR(0xFF)) },
        TestCase { cpu: M68kType::MC68000, data: &[0x007C, 0x1234],                     ins: Some(Instruction::ORtoSR(0x1234)) },
        TestCase { cpu: M68kType::MC68000, data: &[0x0263, 0x1234],                     ins: Some(Instruction::AND(Target::Immediate(0x1234), Target::IndirectARegDec(3), Size::Word)) },
        TestCase { cpu: M68kType::MC68000, data: &[0x023C, 0x1234],                     ins: Some(Instruction::ANDtoCCR(0x34)) },
        TestCase { cpu: M68kType::MC68000, data: &[0x027C, 0xF8FF],                     ins: Some(Instruction::ANDtoSR(0xF8FF)) },
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
        TestCase { cpu: M68kType::MC68000, data: &[0xE300],                             ins: Some(Instruction::ASd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Left)) },
        TestCase { cpu: M68kType::MC68000, data: &[0xE200],                             ins: Some(Instruction::ASd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Right)) },
        TestCase { cpu: M68kType::MC68000, data: &[0xE318],                             ins: Some(Instruction::ROd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Left)) },
        TestCase { cpu: M68kType::MC68000, data: &[0xE218],                             ins: Some(Instruction::ROd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Right)) },

        // MC68030
        TestCase { cpu: M68kType::MC68030, data: &[0x4C3C, 0x0800, 0x0000, 0x0097],                     ins: Some(Instruction::MULL(Target::Immediate(0x97), None, 0, Sign::Signed)) },
        TestCase { cpu: M68kType::MC68030, data: &[0x21BC, 0x0010, 0x14C4, 0x09B0, 0x0010, 0xDF40],     ins: Some(Instruction::MOVE(Target::Immediate(1053892), Target::IndirectRegOffset(BaseRegister::None, Some(IndexRegister { xreg: XRegister::DReg(0), scale: 0, size: Size::Long }), 0x10df40), Size::Long)) },

        // Should Fail
        TestCase { cpu: M68kType::MC68000, data: &[0x21BC, 0x0010, 0x14C4, 0x09B0, 0x0010, 0xDF40],     ins: None },
        TestCase { cpu: M68kType::MC68000, data: &[0xA000],                                             ins: None },
    ];


    fn init_decode_test(cputype: M68kType) -> (M68k, System) {
        let mut system = System::new();

        // Insert basic initialization
        let data = vec![0; 0x00100000];
        let mem = MemoryBlock::new(data);
        system.add_addressable_device(0x00000000, wrap_transmutable(mem)).unwrap();
        system.get_bus().write_beu32(0, INIT_STACK as u32).unwrap();
        system.get_bus().write_beu32(4, INIT_ADDR as u32).unwrap();

        // Initialize the CPU and make sure it's in the expected state
        let port = if cputype <= M68kType::MC68010 {
            BusPort::new(0, 24, 16, system.bus.clone())
        } else {
            BusPort::new(0, 24, 16, system.bus.clone())
        };
        let mut cpu = M68k::new(cputype, 10_000_000, port);
        cpu.init().unwrap();
        assert_eq!(cpu.state.pc, INIT_ADDR as u32);
        assert_eq!(cpu.state.msp, INIT_STACK as u32);

        cpu.decoder.init(INIT_ADDR as u32);
        assert_eq!(cpu.decoder.start, INIT_ADDR as u32);
        assert_eq!(cpu.decoder.instruction, Instruction::NOP);
        (cpu, system)
    }

    fn load_memory(system: &System, data: &[u16]) {
        let mut addr = INIT_ADDR;
        for word in data {
            system.get_bus().write_beu16(addr, *word).unwrap();
            addr += 2;
        }
    }

    fn run_decode_test(case: &TestCase) {
        let (mut cpu, system) = init_decode_test(case.cpu);
        load_memory(&system, case.data);
        match &case.ins {
            Some(ins) => {
                cpu.decode_next().unwrap();
                assert_eq!(cpu.decoder.instruction, ins.clone());
            },
            None => {
                assert_eq!(cpu.decode_next().is_err(), true);
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

    //
    // Addressing Mode Target Tests
    //

    #[test]
    fn target_direct_d() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        let size = Size::Word;

        let target = cpu.decoder.get_mode_as_target(&mut cpu.port, 0b000, 0b001, Some(size)).unwrap();
        assert_eq!(target, Target::DirectDReg(1));
    }

    #[test]
    fn target_direct_a() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        let size = Size::Word;

        let target = cpu.decoder.get_mode_as_target(&mut cpu.port, 0b001, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::DirectAReg(2));
    }

    #[test]
    fn target_indirect_a() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        let size = Size::Long;
        let expected = 0x12345678;

        system.get_bus().write_beu32(INIT_ADDR, expected).unwrap();

        let target = cpu.decoder.get_mode_as_target(&mut cpu.port, 0b010, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectAReg(2));
    }

    #[test]
    fn target_indirect_a_inc() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        let size = Size::Long;
        let expected = 0x12345678;

        system.get_bus().write_beu32(INIT_ADDR, expected).unwrap();

        let target = cpu.decoder.get_mode_as_target(&mut cpu.port, 0b011, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectARegInc(2));
    }

    #[test]
    fn target_indirect_a_dec() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        let size = Size::Long;
        let expected = 0x12345678;

        system.get_bus().write_beu32(INIT_ADDR, expected).unwrap();

        let target = cpu.decoder.get_mode_as_target(&mut cpu.port, 0b100, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectARegDec(2));
    }

    #[test]
    fn target_indirect_a_reg_offset() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        let size = Size::Long;
        let offset = -8;

        system.get_bus().write_beu16(INIT_ADDR, (offset as i16) as u16).unwrap();

        let target = cpu.decoder.get_mode_as_target(&mut cpu.port, 0b101, 0b100, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectRegOffset(BaseRegister::AReg(4), None, offset));
    }

    #[test]
    fn target_indirect_a_reg_brief_extension_word() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        let size = Size::Long;
        let offset = -8;
        let brief_extension = 0x3800 | (((offset as i8) as u8) as u16);

        system.get_bus().write_beu16(INIT_ADDR, brief_extension).unwrap();
        system.get_bus().write_beu16(INIT_ADDR + 2, (offset as i16) as u16).unwrap();

        let target = cpu.decoder.get_mode_as_target(&mut cpu.port, 0b110, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectRegOffset(BaseRegister::AReg(2), Some(IndexRegister { xreg: XRegister::DReg(3), scale: 0, size: size }), offset));
    }

    #[test]
    fn target_indirect_a_reg_full_extension_word() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68020);

        let size = Size::Word;
        let offset = -1843235 as i32;
        let brief_extension = 0xF330;

        system.get_bus().write_beu16(INIT_ADDR, brief_extension).unwrap();
        system.get_bus().write_beu32(INIT_ADDR + 2, offset as u32).unwrap();

        let target = cpu.decoder.get_mode_as_target(&mut cpu.port, 0b110, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectRegOffset(BaseRegister::AReg(2), Some(IndexRegister { xreg: XRegister::AReg(7), scale: 1, size: size }), offset));
    }

    #[test]
    fn target_indirect_a_reg_full_extension_word_no_base() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68020);

        let size = Size::Word;
        let offset = -1843235 as i32;
        let brief_extension = 0xF3B0;

        system.get_bus().write_beu16(INIT_ADDR, brief_extension).unwrap();
        system.get_bus().write_beu32(INIT_ADDR + 2, offset as u32).unwrap();

        let target = cpu.decoder.get_mode_as_target(&mut cpu.port, 0b110, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectRegOffset(BaseRegister::None, Some(IndexRegister { xreg: XRegister::AReg(7), scale: 1, size: size }), offset));
    }

    #[test]
    fn target_indirect_a_reg_full_extension_word_no_index() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68020);

        let size = Size::Word;
        let offset = -1843235 as i32;
        let brief_extension = 0xF370;

        system.get_bus().write_beu16(INIT_ADDR, brief_extension).unwrap();
        system.get_bus().write_beu32(INIT_ADDR + 2, offset as u32).unwrap();

        let target = cpu.decoder.get_mode_as_target(&mut cpu.port, 0b110, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectRegOffset(BaseRegister::AReg(2), None, offset));
    }

    #[test]
    fn target_indirect_pc_offset() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        let size = Size::Long;
        let offset = -8;

        system.get_bus().write_beu16(INIT_ADDR, (offset as i16) as u16).unwrap();

        let target = cpu.decoder.get_mode_as_target(&mut cpu.port, 0b111, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectRegOffset(BaseRegister::PC, None, offset));
    }

    #[test]
    fn target_indirect_pc_brief_extension_word() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        let size = Size::Word;
        let offset = -8;
        let brief_extension = 0x3000 | (((offset as i8) as u8) as u16);

        system.get_bus().write_beu16(INIT_ADDR, brief_extension).unwrap();
        system.get_bus().write_beu16(INIT_ADDR + 2, (offset as i16) as u16).unwrap();

        let target = cpu.decoder.get_mode_as_target(&mut cpu.port, 0b111, 0b011, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectRegOffset(BaseRegister::PC, Some(IndexRegister { xreg: XRegister::DReg(3), scale: 0, size: size }), offset));
    }

    #[test]
    fn target_indirect_pc_full_extension_word() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68020);

        let size = Size::Word;
        let offset = -1843235 as i32;
        let brief_extension = 0xF330;

        system.get_bus().write_beu16(INIT_ADDR, brief_extension).unwrap();
        system.get_bus().write_beu32(INIT_ADDR + 2, offset as u32).unwrap();

        let target = cpu.decoder.get_mode_as_target(&mut cpu.port, 0b111, 0b011, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectRegOffset(BaseRegister::PC, Some(IndexRegister { xreg: XRegister::AReg(7), scale: 1, size: size }), offset));
    }


    #[test]
    fn target_indirect_immediate_word() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        let size = Size::Word;
        let expected = 0x1234;

        system.get_bus().write_beu16(INIT_ADDR, expected as u16).unwrap();

        let target = cpu.decoder.get_mode_as_target(&mut cpu.port, 0b111, 0b000, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectMemory(expected));
    }

    #[test]
    fn target_indirect_immediate_long() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        let size = Size::Word;
        let expected = 0x12345678;

        system.get_bus().write_beu32(INIT_ADDR, expected).unwrap();

        let target = cpu.decoder.get_mode_as_target(&mut cpu.port, 0b111, 0b001, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectMemory(expected));
    }

    #[test]
    fn target_immediate() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        let size = Size::Word;
        let expected = 0x1234;

        system.get_bus().write_beu16(INIT_ADDR, expected as u16).unwrap();

        let target = cpu.decoder.get_mode_as_target(&mut cpu.port, 0b111, 0b100, Some(size)).unwrap();
        assert_eq!(target, Target::Immediate(expected));
    }

    #[test]
    fn target_full_extension_word_unsupported_on_mc68010() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        let brief_extension = 0x0100;

        system.get_bus().write_beu16(INIT_ADDR, brief_extension).unwrap();

        let result = cpu.decoder.get_mode_as_target(&mut cpu.port, 0b110, 0b010, Some(Size::Long));
        match result {
            Err(Error { err: ErrorType::Processor, native, .. }) if native == Exceptions::IllegalInstruction as u32 => { },
            result => panic!("Expected illegal instruction but found: {:?}", result),
        }
    }
}


#[cfg(test)]
mod execute_tests {
    use crate::system::System;
    use crate::memory::{MemoryBlock, BusPort};
    use crate::devices::{Address, Addressable, Steppable, wrap_transmutable};

    use crate::cpus::m68k::{M68k, M68kType};
    use crate::cpus::m68k::state::{M68kState};
    use crate::cpus::m68k::instructions::{Instruction, Target, Size, Sign, ShiftDirection, Condition};

    const INIT_STACK: Address = 0x00002000;
    const INIT_ADDR: Address = 0x00000010;


    struct TestState {
        pc: u32,
        msp: u32,
        usp: u32,
        d0: u32,
        d1: u32,
        a0: u32,
        a1: u32,
        sr: u16,
    }

    struct TestCase {
        name: &'static str,
        ins: Instruction,
        data: &'static [u16],
        cputype: M68kType,
        init: TestState,
        fini: TestState,
    }


    fn init_execute_test(cputype: M68kType) -> (M68k, System) {
        let mut system = System::new();

        // Insert basic initialization
        let data = vec![0; 0x00100000];
        let mem = MemoryBlock::new(data);
        system.add_addressable_device(0x00000000, wrap_transmutable(mem)).unwrap();
        system.get_bus().write_beu32(0, INIT_STACK as u32).unwrap();
        system.get_bus().write_beu32(4, INIT_ADDR as u32).unwrap();

        let port = if cputype <= M68kType::MC68010 {
            BusPort::new(0, 24, 16, system.bus.clone())
        } else {
            BusPort::new(0, 24, 16, system.bus.clone())
        };
        let mut cpu = M68k::new(cputype, 10_000_000, port);
        cpu.step(&system).unwrap();
        cpu.decoder.init(cpu.state.pc);
        assert_eq!(cpu.state.pc, INIT_ADDR as u32);
        assert_eq!(cpu.state.msp, INIT_STACK as u32);
        assert_eq!(cpu.decoder.instruction, Instruction::NOP);
        (cpu, system)
    }

    fn build_state(state: &TestState) -> M68kState {
        let mut new_state = M68kState::new();
        new_state.pc = state.pc;
        new_state.msp = state.msp;
        new_state.usp = state.usp;
        new_state.d_reg[0] = state.d0;
        new_state.d_reg[1] = state.d1;
        new_state.a_reg[0] = state.a0;
        new_state.a_reg[1] = state.a1;
        new_state.sr = state.sr;
        new_state
    }

    fn load_memory(system: &System, data: &[u16]) {
        for i in 0..data.len() {
            system.get_bus().write_beu16((i << 1) as Address, data[i]).unwrap();
        } 
    }

    fn run_test(case: &TestCase) {
        let (mut cpu, system) = init_execute_test(case.cputype);

        let init_state = build_state(&case.init);
        let expected_state = build_state(&case.fini);

        load_memory(&system, case.data);
        cpu.state = init_state;

        cpu.decode_next().unwrap();
        assert_eq!(cpu.decoder.instruction, case.ins);

        cpu.execute_current().unwrap();
        assert_eq!(cpu.state, expected_state);
    }

    #[test]
    pub fn run_execute_tests() {
        for case in TEST_CASES {
            println!("Running test {}", case.name);
            run_test(case);
        }
    }


    const TEST_CASES: &'static [TestCase] = &[
        TestCase {
            name: "nop",
            ins: Instruction::NOP,
            data: &[ 0x4e71 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000002, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
        },
        TestCase {
            name: "addi with no overflow or carry",
            ins: Instruction::ADD(Target::Immediate(0x7f), Target::DirectDReg(0), Size::Byte),
            data: &[ 0x0600, 0x007F ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000004, msp: 0x00000000, usp: 0x00000000, d0: 0x0000007f, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
        },
        TestCase {
            name: "addi with no overflow but negative",
            ins: Instruction::ADD(Target::Immediate(0x80), Target::DirectDReg(0), Size::Byte),
            data: &[ 0x0600, 0x0080 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000001, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000004, msp: 0x00000000, usp: 0x00000000, d0: 0x00000081, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2708 },
        },
        TestCase {
            name: "addi with overflow",
            ins: Instruction::ADD(Target::Immediate(0x7f), Target::DirectDReg(0), Size::Byte),
            data: &[ 0x0600, 0x007F ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000001, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000004, msp: 0x00000000, usp: 0x00000000, d0: 0x00000080, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x270A },
        },
        TestCase {
            name: "addi with carry",
            ins: Instruction::ADD(Target::Immediate(0x80), Target::DirectDReg(0), Size::Byte),
            data: &[ 0x0600, 0x0080 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000080, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000004, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2717 },
        },
        TestCase {
            name: "adda immediate",
            ins: Instruction::ADDA(Target::Immediate(0xF800), 0, Size::Word),
            data: &[ 0xD0FC, 0xF800 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x27FF },
            fini: TestState { pc: 0x00000004, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0xFFFFF800, a1: 0x00000000, sr: 0x27FF },
        },
        TestCase {
            name: "adda register",
            ins: Instruction::ADDA(Target::DirectDReg(0), 0, Size::Word),
            data: &[ 0xD0C0 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x0000F800, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x27FF },
            fini: TestState { pc: 0x00000002, msp: 0x00000000, usp: 0x00000000, d0: 0x0000F800, d1: 0x00000000, a0: 0xFFFFF800, a1: 0x00000000, sr: 0x27FF },
        },
        TestCase {
            name: "andi with sr",
            ins: Instruction::ANDtoSR(0xF8FF),
            data: &[ 0x027C, 0xF8FF ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0xA7AA },
            fini: TestState { pc: 0x00000004, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0xA0AA },
        },
        TestCase {
            name: "asl",
            ins: Instruction::ASd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Left),
            data: &[ 0xE300 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000001, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000002, msp: 0x00000000, usp: 0x00000000, d0: 0x00000002, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
        },
        TestCase {
            name: "asr",
            ins: Instruction::ASd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Right),
            data: &[ 0xE200 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000081, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000002, msp: 0x00000000, usp: 0x00000000, d0: 0x000000C0, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2719 },
        },
        TestCase {
            name: "blt with jump",
            ins: Instruction::Bcc(Condition::LessThan, 8),
            data: &[ 0x6D08 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2709 },
            fini: TestState { pc: 0x0000000A, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2709 },
        },
        TestCase {
            name: "blt with jump",
            ins: Instruction::Bcc(Condition::LessThan, 8),
            data: &[ 0x6D08 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000002, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
        },
        TestCase {
            name: "bchg not zero",
            ins: Instruction::BCHG(Target::Immediate(7), Target::DirectDReg(1), Size::Long),
            data: &[ 0x0841, 0x0007 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x000000FF, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000004, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x0000007F, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
        },
        TestCase {
            name: "bchg zero",
            ins: Instruction::BCHG(Target::Immediate(7), Target::DirectDReg(1), Size::Long),
            data: &[ 0x0841, 0x0007 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000004, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000080, a0: 0x00000000, a1: 0x00000000, sr: 0x2704 },
        },
        TestCase {
            name: "bra 8-bit",
            ins: Instruction::BRA(-32),
            data: &[ 0x60E0 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0xFFFFFFE2, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
        },
        TestCase {
            name: "cmpi equal",
            ins: Instruction::CMP(Target::Immediate(0x20), Target::DirectDReg(0), Size::Byte),
            data: &[ 0x0C00, 0x0020 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000020, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000004, msp: 0x00000000, usp: 0x00000000, d0: 0x00000020, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2704 },
        },
        TestCase {
            name: "cmpi greater than",
            ins: Instruction::CMP(Target::Immediate(0x30), Target::DirectDReg(0), Size::Byte),
            data: &[ 0x0C00, 0x0030 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000020, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000004, msp: 0x00000000, usp: 0x00000000, d0: 0x00000020, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2709 },
        },
        TestCase {
            name: "cmpi less than",
            ins: Instruction::CMP(Target::Immediate(0x10), Target::DirectDReg(0), Size::Byte),
            data: &[ 0x0C00, 0x0010 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000020, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000004, msp: 0x00000000, usp: 0x00000000, d0: 0x00000020, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
        },
        TestCase {
            name: "cmpi no overflow",
            ins: Instruction::CMP(Target::Immediate(0x7F), Target::DirectDReg(0), Size::Byte),
            data: &[ 0x0C00, 0x007F ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000004, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2709 },
        },
        TestCase {
            name: "cmpi no overflow, already negative",
            ins: Instruction::CMP(Target::Immediate(0x8001), Target::DirectDReg(0), Size::Word),
            data: &[ 0x0C40, 0x8001 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000004, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2701 },
        },
        TestCase {
            name: "cmpi with overflow",
            ins: Instruction::CMP(Target::Immediate(0x80), Target::DirectDReg(0), Size::Byte),
            data: &[ 0x0C00, 0x0080 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000004, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x270B },
        },
        TestCase {
            name: "cmpi with overflow 2",
            ins: Instruction::CMP(Target::Immediate(0x8001), Target::DirectDReg(0), Size::Word),
            data: &[ 0x0C40, 0x8001 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000001, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000004, msp: 0x00000000, usp: 0x00000000, d0: 0x00000001, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x270B },
        },
        TestCase {
            name: "cmpi no carry",
            ins: Instruction::CMP(Target::Immediate(0x01), Target::DirectDReg(0), Size::Byte),
            data: &[ 0x0C00, 0x0001 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x000000FF, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000004, msp: 0x00000000, usp: 0x00000000, d0: 0x000000FF, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2708 },
        },
        TestCase {
            name: "cmpi with carry",
            ins: Instruction::CMP(Target::Immediate(0xFF), Target::DirectDReg(0), Size::Byte),
            data: &[ 0x0C00, 0x00FF ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000001, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000004, msp: 0x00000000, usp: 0x00000000, d0: 0x00000001, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2701 },
        },
        TestCase {
            name: "divu",
            ins: Instruction::DIVW(Target::Immediate(0x0245), 0, Sign::Unsigned),
            data: &[ 0x80FC, 0x0245 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00040000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000004, msp: 0x00000000, usp: 0x00000000, d0: 0x007101C3, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
        },
        TestCase {
            name: "eori",
            ins: Instruction::EOR(Target::DirectDReg(1), Target::DirectDReg(0), Size::Long),
            data: &[ 0xB380 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0xAAAA5555, d1: 0x55AA55AA, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000002, msp: 0x00000000, usp: 0x00000000, d0: 0xFF0000FF, d1: 0x55AA55AA, a0: 0x00000000, a1: 0x00000000, sr: 0x2708 },
        },
        TestCase {
            name: "exg",
            ins: Instruction::EXG(Target::DirectDReg(0), Target::DirectAReg(1)),
            data: &[ 0xC189 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x12345678, d1: 0x00000000, a0: 0x00000000, a1: 0x87654321, sr: 0x2700 },
            fini: TestState { pc: 0x00000002, msp: 0x00000000, usp: 0x00000000, d0: 0x87654321, d1: 0x00000000, a0: 0x00000000, a1: 0x12345678, sr: 0x2700 },
        },
        TestCase {
            name: "ext",
            ins: Instruction::EXT(0, Size::Byte, Size::Word),
            data: &[ 0x4880 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x000000CB, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x27FF },
            fini: TestState { pc: 0x00000002, msp: 0x00000000, usp: 0x00000000, d0: 0x0000FFCB, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x27F8 },
        },
        TestCase {
            name: "ext",
            ins: Instruction::EXT(0, Size::Word, Size::Long),
            data: &[ 0x48C0 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x000000CB, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x27FF },
            fini: TestState { pc: 0x00000002, msp: 0x00000000, usp: 0x00000000, d0: 0x000000CB, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x27F0 },
        },
        TestCase {
            name: "muls",
            ins: Instruction::MULW(Target::Immediate(0x0276), 0, Sign::Signed),
            data: &[ 0xC1FC, 0x0276 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000200, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000004, msp: 0x00000000, usp: 0x00000000, d0: 0x0004ec00, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
        },
        TestCase {
            name: "movel",
            ins: Instruction::MOVE(Target::DirectDReg(0), Target::DirectDReg(1), Size::Long),
            data: &[ 0x2200 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0xFEDCBA98, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000002, msp: 0x00000000, usp: 0x00000000, d0: 0xFEDCBA98, d1: 0xFEDCBA98, a0: 0x00000000, a1: 0x00000000, sr: 0x2708 },
        },
        TestCase {
            name: "movea",
            ins: Instruction::MOVEA(Target::DirectDReg(0), 0, Size::Long),
            data: &[ 0x2040 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0xFEDCBA98, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x27FF },
            fini: TestState { pc: 0x00000002, msp: 0x00000000, usp: 0x00000000, d0: 0xFEDCBA98, d1: 0x00000000, a0: 0xFEDCBA98, a1: 0x00000000, sr: 0x27FF },
        },
        TestCase {
            name: "neg",
            ins: Instruction::NEG(Target::DirectDReg(0), Size::Word),
            data: &[ 0x4440 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000080, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000002, msp: 0x00000000, usp: 0x00000000, d0: 0x0000FF80, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2719 },
        },


        TestCase {
            name: "ori",
            ins: Instruction::OR(Target::Immediate(0xFF), Target::DirectAReg(0), Size::Byte),
            data: &[ 0x0008, 0x00FF ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000004, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x000000FF, a1: 0x00000000, sr: 0x2708 },
        },
        TestCase {
            name: "ori with sr",
            ins: Instruction::ORtoSR(0x00AA),
            data: &[ 0x007C, 0x00AA ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0xA755 },
            fini: TestState { pc: 0x00000004, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0xA7FF },
        },



        TestCase {
            name: "rol",
            ins: Instruction::ROd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Left),
            data: &[ 0xE318 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000080, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000002, msp: 0x00000000, usp: 0x00000000, d0: 0x00000001, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2701 },
        },
        TestCase {
            name: "ror",
            ins: Instruction::ROd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Right),
            data: &[ 0xE218 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000001, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000002, msp: 0x00000000, usp: 0x00000000, d0: 0x00000080, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2709 },
        },
        TestCase {
            name: "roxl",
            ins: Instruction::ROXd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Left),
            data: &[ 0xE310 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000080, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000002, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2715 },
        },
        TestCase {
            name: "roxr",
            ins: Instruction::ROXd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Right),
            data: &[ 0xE210 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000001, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000002, msp: 0x00000000, usp: 0x00000000, d0: 0x00000000, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2715 },
        },
        TestCase {
            name: "roxl two bits",
            ins: Instruction::ROXd(Target::Immediate(2), Target::DirectDReg(0), Size::Byte, ShiftDirection::Left),
            data: &[ 0xE510 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000080, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000002, msp: 0x00000000, usp: 0x00000000, d0: 0x00000001, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
        },
        TestCase {
            name: "roxr two bits",
            ins: Instruction::ROXd(Target::Immediate(2), Target::DirectDReg(0), Size::Byte, ShiftDirection::Right),
            data: &[ 0xE410 ],
            cputype: M68kType::MC68010,
            init: TestState { pc: 0x00000000, msp: 0x00000000, usp: 0x00000000, d0: 0x00000001, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2700 },
            fini: TestState { pc: 0x00000002, msp: 0x00000000, usp: 0x00000000, d0: 0x00000080, d1: 0x00000000, a0: 0x00000000, a1: 0x00000000, sr: 0x2708 },
        },
    ];


    //
    // Addressing Mode Target Tests
    //

    #[test]
    fn target_value_direct_d() {
        let (mut cpu, _) = init_execute_test(M68kType::MC68010);

        let size = Size::Word;
        let expected = 0x1234;
        let target = Target::DirectDReg(1);

        cpu.state.d_reg[1] = expected;
        let result = cpu.get_target_value(target, size).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn target_value_direct_a() {
        let (mut cpu, _) = init_execute_test(M68kType::MC68010);

        let size = Size::Word;
        let expected = 0x1234;
        let target = Target::DirectAReg(2);

        cpu.state.a_reg[2] = expected;
        let result = cpu.get_target_value(target, size).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn target_value_indirect_a() {
        let (mut cpu, _) = init_execute_test(M68kType::MC68010);

        let size = Size::Long;
        let expected = 0x12345678;
        let target = Target::IndirectAReg(2);
        cpu.port.write_beu32(INIT_ADDR, expected).unwrap();

        cpu.state.a_reg[2] = INIT_ADDR as u32;
        let result = cpu.get_target_value(target, size).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn target_value_indirect_a_inc() {
        let (mut cpu, _) = init_execute_test(M68kType::MC68010);

        let size = Size::Long;
        let expected = 0x12345678;
        let target = Target::IndirectARegInc(2);
        cpu.port.write_beu32(INIT_ADDR, expected).unwrap();

        cpu.state.a_reg[2] = INIT_ADDR as u32;
        let result = cpu.get_target_value(target, size).unwrap();
        assert_eq!(result, expected);
        assert_eq!(cpu.state.a_reg[2], (INIT_ADDR as u32) + 4);
    }

    #[test]
    fn target_value_indirect_a_dec() {
        let (mut cpu, _) = init_execute_test(M68kType::MC68010);

        let size = Size::Long;
        let expected = 0x12345678;
        let target = Target::IndirectARegDec(2);
        cpu.port.write_beu32(INIT_ADDR, expected).unwrap();

        cpu.state.a_reg[2] = (INIT_ADDR as u32) + 4;
        let result = cpu.get_target_value(target, size).unwrap();
        assert_eq!(result, expected);
        assert_eq!(cpu.state.a_reg[2], INIT_ADDR as u32);
    }


    #[test]
    fn target_value_immediate() {
        let (mut cpu, _) = init_execute_test(M68kType::MC68010);

        let size = Size::Word;
        let expected = 0x1234;

        let target = Target::Immediate(expected);

        let result = cpu.get_target_value(target, size).unwrap();
        assert_eq!(result, expected);
    }
}


