
#[cfg(test)]
mod decode_tests {
    use crate::error::{Error, ErrorType};
    use crate::system::System;
    use crate::memory::MemoryBlock;
    use crate::devices::{Address, Addressable, Steppable, TransmutableBox, wrap_transmutable, MAX_READ};

    use crate::cpus::m68k::{M68k, M68kType};
    use crate::cpus::m68k::state::Exceptions;
    use crate::cpus::m68k::instructions::{Instruction, Target, Size, Sign, XRegister, BaseRegister, IndexRegister, ShiftDirection};

    const INIT_STACK: Address = 0x00002000;
    const INIT_ADDR: Address = 0x00000010;

    fn init_decode_test(cputype: M68kType) -> (M68k, System) {
        let mut system = System::new();

        // Insert basic initialization
        let data = vec![0; 0x00100000];
        let mem = MemoryBlock::new(data);
        system.add_addressable_device(0x00000000, wrap_transmutable(mem)).unwrap();
        system.get_bus().write_beu32(0, INIT_STACK as u32).unwrap();
        system.get_bus().write_beu32(4, INIT_ADDR as u32).unwrap();

        // Initialize the CPU and make sure it's in the expected state
        let mut cpu = M68k::new(cputype, 10_000_000);
        cpu.init(&system).unwrap();
        assert_eq!(cpu.state.pc, INIT_ADDR as u32);
        assert_eq!(cpu.state.msp, INIT_STACK as u32);
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

    fn get_decode_memory(cpu: &mut M68k, system: &System) -> TransmutableBox {
        let (memory, relative_addr) = system.get_bus().get_device_at(INIT_ADDR, 12).unwrap();
        cpu.decoder.init((INIT_ADDR - relative_addr) as u32, INIT_ADDR as u32);
        memory
    }

    //
    // Addressing Mode Target Tests
    //

    #[test]
    fn target_direct_d() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        let size = Size::Word;
        let expected = 0x1234;

        let memory = get_decode_memory(&mut cpu, &system);
        let target = cpu.decoder.get_mode_as_target(memory.borrow_mut().as_addressable().unwrap(), 0b000, 0b001, Some(size)).unwrap();
        assert_eq!(target, Target::DirectDReg(1));
    }

    #[test]
    fn target_direct_a() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        let size = Size::Word;
        let expected = 0x1234;

        let memory = get_decode_memory(&mut cpu, &system);
        let target = cpu.decoder.get_mode_as_target(memory.borrow_mut().as_addressable().unwrap(), 0b001, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::DirectAReg(2));
    }

    #[test]
    fn target_indirect_a() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        let size = Size::Long;
        let expected_addr = INIT_ADDR;
        let expected = 0x12345678;

        system.get_bus().write_beu32(INIT_ADDR, expected).unwrap();

        let memory = get_decode_memory(&mut cpu, &system);
        let target = cpu.decoder.get_mode_as_target(memory.borrow_mut().as_addressable().unwrap(), 0b010, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectAReg(2));
    }

    #[test]
    fn target_indirect_a_inc() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        let size = Size::Long;
        let expected_addr = INIT_ADDR;
        let expected = 0x12345678;

        system.get_bus().write_beu32(INIT_ADDR, expected).unwrap();

        let memory = get_decode_memory(&mut cpu, &system);
        let target = cpu.decoder.get_mode_as_target(memory.borrow_mut().as_addressable().unwrap(), 0b011, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectARegInc(2));
    }

    #[test]
    fn target_indirect_a_dec() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        let size = Size::Long;
        let expected_addr = INIT_ADDR + 4;
        let expected = 0x12345678;

        system.get_bus().write_beu32(INIT_ADDR, expected).unwrap();

        let memory = get_decode_memory(&mut cpu, &system);
        let target = cpu.decoder.get_mode_as_target(memory.borrow_mut().as_addressable().unwrap(), 0b100, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectARegDec(2));
    }

    #[test]
    fn target_indirect_a_reg_offset() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        let size = Size::Long;
        let offset = -8;

        system.get_bus().write_beu16(INIT_ADDR, (offset as i16) as u16).unwrap();

        let memory = get_decode_memory(&mut cpu, &system);
        let target = cpu.decoder.get_mode_as_target(memory.borrow_mut().as_addressable().unwrap(), 0b101, 0b100, Some(size)).unwrap();
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

        let memory = get_decode_memory(&mut cpu, &system);
        let target = cpu.decoder.get_mode_as_target(memory.borrow_mut().as_addressable().unwrap(), 0b110, 0b010, Some(size)).unwrap();
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

        let memory = get_decode_memory(&mut cpu, &system);
        let target = cpu.decoder.get_mode_as_target(memory.borrow_mut().as_addressable().unwrap(), 0b110, 0b010, Some(size)).unwrap();
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

        let memory = get_decode_memory(&mut cpu, &system);
        let target = cpu.decoder.get_mode_as_target(memory.borrow_mut().as_addressable().unwrap(), 0b110, 0b010, Some(size)).unwrap();
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

        let memory = get_decode_memory(&mut cpu, &system);
        let target = cpu.decoder.get_mode_as_target(memory.borrow_mut().as_addressable().unwrap(), 0b110, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectRegOffset(BaseRegister::AReg(2), None, offset));
    }

    #[test]
    fn target_indirect_pc_offset() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        let size = Size::Long;
        let offset = -8;

        system.get_bus().write_beu16(INIT_ADDR, (offset as i16) as u16).unwrap();

        let memory = get_decode_memory(&mut cpu, &system);
        let target = cpu.decoder.get_mode_as_target(memory.borrow_mut().as_addressable().unwrap(), 0b111, 0b010, Some(size)).unwrap();
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

        let memory = get_decode_memory(&mut cpu, &system);
        let target = cpu.decoder.get_mode_as_target(memory.borrow_mut().as_addressable().unwrap(), 0b111, 0b011, Some(size)).unwrap();
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

        let memory = get_decode_memory(&mut cpu, &system);
        let target = cpu.decoder.get_mode_as_target(memory.borrow_mut().as_addressable().unwrap(), 0b111, 0b011, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectRegOffset(BaseRegister::PC, Some(IndexRegister { xreg: XRegister::AReg(7), scale: 1, size: size }), offset));
    }


    #[test]
    fn target_indirect_immediate_word() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        let size = Size::Word;
        let expected = 0x1234;

        system.get_bus().write_beu16(INIT_ADDR, expected as u16).unwrap();

        let memory = get_decode_memory(&mut cpu, &system);
        let target = cpu.decoder.get_mode_as_target(memory.borrow_mut().as_addressable().unwrap(), 0b111, 0b000, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectMemory(expected));
    }

    #[test]
    fn target_indirect_immediate_long() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        let size = Size::Word;
        let expected = 0x12345678;

        system.get_bus().write_beu32(INIT_ADDR, expected).unwrap();

        let memory = get_decode_memory(&mut cpu, &system);
        let target = cpu.decoder.get_mode_as_target(memory.borrow_mut().as_addressable().unwrap(), 0b111, 0b001, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectMemory(expected));
    }

    #[test]
    fn target_immediate() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        let size = Size::Word;
        let expected = 0x1234;

        system.get_bus().write_beu16(INIT_ADDR, expected as u16).unwrap();

        let memory = get_decode_memory(&mut cpu, &system);
        let target = cpu.decoder.get_mode_as_target(memory.borrow_mut().as_addressable().unwrap(), 0b111, 0b100, Some(size)).unwrap();
        assert_eq!(target, Target::Immediate(expected));
    }

    #[test]
    fn target_full_extension_word_unsupported_on_mc68010() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        let brief_extension = 0x0100;

        system.get_bus().write_beu16(INIT_ADDR, brief_extension).unwrap();

        let memory = get_decode_memory(&mut cpu, &system);
        let result = cpu.decoder.get_mode_as_target(memory.borrow_mut().as_addressable().unwrap(), 0b110, 0b010, Some(Size::Long));
        match result {
            Err(Error { err: ErrorType::Processor, native, .. }) if native == Exceptions::IllegalInstruction as u32 => { },
            result => panic!("Expected illegal instruction but found: {:?}", result),
        }
    }

    //
    // Instruction Decode Tests
    //

    #[test]
    fn instruction_nop() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        load_memory(&system, &[0x4e71]);
        cpu.decode_next(&system).unwrap();

        assert_eq!(cpu.decoder.instruction, Instruction::NOP);
    }

    #[test]
    fn instruction_ori_byte() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        load_memory(&system, &[0x0008, 0x00FF]);
        cpu.decode_next(&system).unwrap();

        assert_eq!(cpu.decoder.instruction, Instruction::OR(Target::Immediate(0xFF), Target::DirectAReg(0), Size::Byte));
    }

    #[test]
    fn instruction_ori_to_ccr() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        load_memory(&system, &[0x003C, 0x00FF]);
        cpu.decode_next(&system).unwrap();

        assert_eq!(cpu.decoder.instruction, Instruction::ORtoCCR(0xFF));
    }

    #[test]
    fn instruction_ori_to_sr() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        load_memory(&system, &[0x007C, 0x1234]);
        cpu.decode_next(&system).unwrap();

        assert_eq!(cpu.decoder.instruction, Instruction::ORtoSR(0x1234));
    }

    #[test]
    fn instruction_andi_word() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        load_memory(&system, &[0x0263, 0x1234]);
        cpu.decode_next(&system).unwrap();

        assert_eq!(cpu.decoder.instruction, Instruction::AND(Target::Immediate(0x1234), Target::IndirectARegDec(3), Size::Word));
    }

    #[test]
    fn instruction_andi_to_ccr() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        load_memory(&system, &[0x023C, 0x1234]);
        cpu.decode_next(&system).unwrap();

        assert_eq!(cpu.decoder.instruction, Instruction::ANDtoCCR(0x34));
    }

    #[test]
    fn instruction_andi_to_sr() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        load_memory(&system, &[0x027C, 0xF8FF]);
        cpu.decode_next(&system).unwrap();

        assert_eq!(cpu.decoder.instruction, Instruction::ANDtoSR(0xF8FF));
    }

    #[test]
    fn instruction_subi() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        load_memory(&system, &[0x0487, 0x1234, 0x5678]);
        cpu.decode_next(&system).unwrap();

        assert_eq!(cpu.decoder.instruction, Instruction::SUB(Target::Immediate(0x12345678), Target::DirectDReg(7), Size::Long));
    }

    #[test]
    fn instruction_addi() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        load_memory(&system, &[0x063A, 0x1234, 0x0055]);
        cpu.decode_next(&system).unwrap();

        assert_eq!(cpu.decoder.instruction, Instruction::ADD(Target::Immediate(0x34), Target::IndirectRegOffset(BaseRegister::PC, None, 0x55), Size::Byte));
    }

    #[test]
    fn instruction_eori_byte() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        load_memory(&system, &[0x0A23, 0x1234]);
        cpu.decode_next(&system).unwrap();

        assert_eq!(cpu.decoder.instruction, Instruction::EOR(Target::Immediate(0x34), Target::IndirectARegDec(3), Size::Byte));
    }

    #[test]
    fn instruction_eori_to_ccr() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        load_memory(&system, &[0x0A3C, 0x1234]);
        cpu.decode_next(&system).unwrap();

        assert_eq!(cpu.decoder.instruction, Instruction::EORtoCCR(0x34));
    }

    #[test]
    fn instruction_eori_to_sr() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        load_memory(&system, &[0x0A7C, 0xF8FF]);
        cpu.decode_next(&system).unwrap();

        assert_eq!(cpu.decoder.instruction, Instruction::EORtoSR(0xF8FF));
    }


    #[test]
    fn instruction_cmpi_equal() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        load_memory(&system, &[0x0C00, 0x0020]);
        cpu.decode_next(&system).unwrap();

        assert_eq!(cpu.decoder.instruction, Instruction::CMP(Target::Immediate(0x20), Target::DirectDReg(0), Size::Byte));
    }

    #[test]
    fn instruction_cmpi_greater() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        load_memory(&system, &[0x0C00, 0x0030]);
        cpu.decode_next(&system).unwrap();

        assert_eq!(cpu.decoder.instruction, Instruction::CMP(Target::Immediate(0x30), Target::DirectDReg(0), Size::Byte));
    }

    #[test]
    fn instruction_cmpi_less() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        load_memory(&system, &[0x0C00, 0x0010]);
        cpu.decode_next(&system).unwrap();

        assert_eq!(cpu.decoder.instruction, Instruction::CMP(Target::Immediate(0x10), Target::DirectDReg(0), Size::Byte));
    }

    #[test]
    fn instruction_movel_full_extension() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68030);

        load_memory(&system, &[0x21bc, 0x0010, 0x14c4, 0x09b0, 0x0010, 0xdf40]);
        cpu.decode_next(&system).unwrap();

        assert_eq!(cpu.decoder.instruction, Instruction::MOVE(Target::Immediate(1053892), Target::IndirectRegOffset(BaseRegister::None, Some(IndexRegister { xreg: XRegister::DReg(0), scale: 0, size: Size::Long }), 0x10df40), Size::Long));
    }

    #[test]
    fn instruction_mulsl() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68030);

        load_memory(&system, &[0x4c3c, 0x0800, 0x0000, 0x0097]);
        cpu.decode_next(&system).unwrap();

        assert_eq!(cpu.decoder.instruction, Instruction::MULL(Target::Immediate(0x97), None, 0, Sign::Signed));
    }

    #[test]
    fn instruction_divs() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        load_memory(&system, &[0x81FC, 0x0003]);
        cpu.decode_next(&system).unwrap();

        assert_eq!(cpu.decoder.instruction, Instruction::DIVW(Target::Immediate(3), 0, Sign::Signed));
    }

    #[test]
    fn instruction_muls() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        load_memory(&system, &[0xC1FC, 0x0276]);
        cpu.decode_next(&system).unwrap();

        assert_eq!(cpu.decoder.instruction, Instruction::MULW(Target::Immediate(0x276), 0, Sign::Signed));
    }

    #[test]
    fn instruction_asli() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        load_memory(&system, &[0xE300]);
        cpu.decode_next(&system).unwrap();

        assert_eq!(cpu.decoder.instruction, Instruction::ASd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Left));
    }

    #[test]
    fn instruction_asri() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        load_memory(&system, &[0xE200]);
        cpu.decode_next(&system).unwrap();

        assert_eq!(cpu.decoder.instruction, Instruction::ASd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Right));
    }

    #[test]
    fn instruction_roli() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        load_memory(&system, &[0xE318]);
        cpu.decode_next(&system).unwrap();

        assert_eq!(cpu.decoder.instruction, Instruction::ROd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Left));
    }

    #[test]
    fn instruction_rori() {
        let (mut cpu, system) = init_decode_test(M68kType::MC68010);

        load_memory(&system, &[0xE218]);
        cpu.decode_next(&system).unwrap();

        assert_eq!(cpu.decoder.instruction, Instruction::ROd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Right));
    }
}


#[cfg(test)]
mod execute_tests {
    use crate::system::System;
    use crate::memory::MemoryBlock;
    use crate::devices::{Address, Addressable, Steppable, wrap_transmutable};

    use crate::cpus::m68k::{M68k, M68kType};
    use crate::cpus::m68k::instructions::{Instruction, Target, Size, Sign, ShiftDirection};

    const INIT_STACK: Address = 0x00002000;
    const INIT_ADDR: Address = 0x00000010;

    fn init_test() -> (M68k, System) {
        let mut system = System::new();

        // Insert basic initialization
        let data = vec![0; 0x00100000];
        let mem = MemoryBlock::new(data);
        system.add_addressable_device(0x00000000, wrap_transmutable(mem)).unwrap();
        system.get_bus().write_beu32(0, INIT_STACK as u32).unwrap();
        system.get_bus().write_beu32(4, INIT_ADDR as u32).unwrap();

        let mut cpu = M68k::new(M68kType::MC68010, 10_000_000);
        cpu.step(&system).unwrap();
        assert_eq!(cpu.state.pc, INIT_ADDR as u32);
        assert_eq!(cpu.state.msp, INIT_STACK as u32);
        assert_eq!(cpu.decoder.instruction, Instruction::NOP);
        (cpu, system)
    }


    #[test]
    fn instruction_nop() {
        let (mut cpu, system) = init_test();

        cpu.decoder.instruction = Instruction::NOP;

        let expected_state = cpu.state.clone();

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state, expected_state);
    }


    #[test]
    fn instruction_ori() {
        let (mut cpu, system) = init_test();

        cpu.decoder.instruction = Instruction::OR(Target::Immediate(0xFF), Target::DirectAReg(0), Size::Byte);

        let mut expected_state = cpu.state.clone();
        expected_state.sr = 0x2708;
        expected_state.a_reg[0] = 0x000000FF;

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state, expected_state);
    }

    #[test]
    fn instruction_cmpi_equal() {
        let (mut cpu, system) = init_test();

        let value = 0x20;
        cpu.state.d_reg[0] = value;
        cpu.decoder.instruction = Instruction::CMP(Target::Immediate(value), Target::DirectDReg(0), Size::Byte);

        let mut expected_state = cpu.state.clone();
        expected_state.sr = 0x2704;

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state, expected_state);
    }

    #[test]
    fn instruction_cmpi_greater() {
        let (mut cpu, system) = init_test();

        cpu.state.d_reg[0] = 0x20;
        cpu.decoder.instruction = Instruction::CMP(Target::Immediate(0x30), Target::DirectDReg(0), Size::Byte);

        let mut expected_state = cpu.state.clone();
        expected_state.sr = 0x2709;

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state, expected_state);
    }

    #[test]
    fn instruction_cmpi_less() {
        let (mut cpu, system) = init_test();

        cpu.state.d_reg[0] = 0x20;
        cpu.decoder.instruction = Instruction::CMP(Target::Immediate(0x10), Target::DirectDReg(0), Size::Byte);

        let mut expected_state = cpu.state.clone();
        expected_state.sr = 0x2700;

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state, expected_state);
    }

    #[test]
    fn instruction_andi_sr() {
        let (mut cpu, system) = init_test();

        cpu.state.sr = 0xA7AA;
        cpu.decoder.instruction = Instruction::ANDtoSR(0xF8FF);

        let mut expected_state = cpu.state.clone();
        expected_state.sr = 0xA0AA;

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state, expected_state);
    }

    #[test]
    fn instruction_ori_sr() {
        let (mut cpu, system) = init_test();

        cpu.state.sr = 0xA755;
        cpu.decoder.instruction = Instruction::ORtoSR(0x00AA);

        let mut expected_state = cpu.state.clone();
        expected_state.sr = 0xA7FF;

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state, expected_state);
    }

    #[test]
    fn instruction_muls() {
        let (mut cpu, system) = init_test();

        let value = 0x0276;
        cpu.state.d_reg[0] = 0x0200;
        cpu.decoder.instruction = Instruction::MULW(Target::Immediate(value), 0, Sign::Signed);

        let mut expected_state = cpu.state.clone();
        expected_state.d_reg[0] = 0x4ec00;

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state, expected_state);
    }

    #[test]
    fn instruction_divu() {
        let (mut cpu, system) = init_test();

        let value = 0x0245;
        cpu.state.d_reg[0] = 0x40000;
        cpu.decoder.instruction = Instruction::DIVW(Target::Immediate(value), 0, Sign::Unsigned);

        let mut expected_state = cpu.state.clone();
        expected_state.d_reg[0] = 0x007101C3;

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state, expected_state);
    }

    #[test]
    fn instruction_asli() {
        let (mut cpu, system) = init_test();

        cpu.state.d_reg[0] = 0x01;
        cpu.decoder.instruction = Instruction::ASd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Left);

        let mut expected_state = cpu.state.clone();
        expected_state.sr = 0x2700;
        expected_state.d_reg[0] = 0x00000002;

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state, expected_state);
    }

    #[test]
    fn instruction_asri() {
        let (mut cpu, system) = init_test();

        cpu.state.d_reg[0] = 0x81;
        cpu.decoder.instruction = Instruction::ASd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Right);

        let mut expected_state = cpu.state.clone();
        expected_state.sr = 0x2719;
        expected_state.d_reg[0] = 0x000000C0;

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state, expected_state);
    }

    #[test]
    fn instruction_roli() {
        let (mut cpu, system) = init_test();

        cpu.state.d_reg[0] = 0x80;
        cpu.decoder.instruction = Instruction::ROd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Left);

        let mut expected_state = cpu.state.clone();
        expected_state.sr = 0x2701;
        expected_state.d_reg[0] = 0x00000001;

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state, expected_state);
    }

    #[test]
    fn instruction_rori() {
        let (mut cpu, system) = init_test();

        cpu.state.d_reg[0] = 0x01;
        cpu.decoder.instruction = Instruction::ROd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Right);

        let mut expected_state = cpu.state.clone();
        expected_state.sr = 0x2709;
        expected_state.d_reg[0] = 0x00000080;

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state, expected_state);
    }

    #[test]
    fn instruction_roxl() {
        let (mut cpu, system) = init_test();

        cpu.state.d_reg[0] = 0x80;
        cpu.state.sr = 0x2700;
        cpu.decoder.instruction = Instruction::ROXd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Left);

        let mut expected_state = cpu.state.clone();
        expected_state.sr = 0x2715;
        expected_state.d_reg[0] = 0x00000000;

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state, expected_state);
    }

    #[test]
    fn instruction_roxr() {
        let (mut cpu, system) = init_test();

        cpu.state.d_reg[0] = 0x01;
        cpu.state.sr = 0x2700;
        cpu.decoder.instruction = Instruction::ROXd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Right);

        let mut expected_state = cpu.state.clone();
        expected_state.sr = 0x2715;
        expected_state.d_reg[0] = 0x00000000;

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state, expected_state);
    }

    #[test]
    fn instruction_roxl_2() {
        let (mut cpu, system) = init_test();

        cpu.state.d_reg[0] = 0x80;
        cpu.state.sr = 0x2700;
        cpu.decoder.instruction = Instruction::ROXd(Target::Immediate(2), Target::DirectDReg(0), Size::Byte, ShiftDirection::Left);

        let mut expected_state = cpu.state.clone();
        expected_state.sr = 0x2700;
        expected_state.d_reg[0] = 0x00000001;

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state, expected_state);
    }

    #[test]
    fn instruction_roxr_2() {
        let (mut cpu, system) = init_test();

        cpu.state.d_reg[0] = 0x01;
        cpu.state.sr = 0x2700;
        cpu.decoder.instruction = Instruction::ROXd(Target::Immediate(2), Target::DirectDReg(0), Size::Byte, ShiftDirection::Right);

        let mut expected_state = cpu.state.clone();
        expected_state.sr = 0x2708;
        expected_state.d_reg[0] = 0x00000080;

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state, expected_state);
    }

    #[test]
    fn instruction_neg_word() {
        let (mut cpu, system) = init_test();

        cpu.state.d_reg[0] = 0x80;
        cpu.state.sr = 0x2700;
        cpu.decoder.instruction = Instruction::NEG(Target::DirectDReg(0), Size::Word);

        let mut expected_state = cpu.state.clone();
        expected_state.sr = 0x2709;
        expected_state.d_reg[0] = 0x0000FF80;

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state, expected_state);
    }



    #[test]
    fn target_value_direct_d() {
        let (mut cpu, system) = init_test();

        let size = Size::Word;
        let expected = 0x1234;
        let target = Target::DirectDReg(1);

        cpu.state.d_reg[1] = expected;
        let result = cpu.get_target_value(&system, target, size).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn target_value_direct_a() {
        let (mut cpu, system) = init_test();

        let size = Size::Word;
        let expected = 0x1234;
        let target = Target::DirectAReg(2);

        cpu.state.a_reg[2] = expected;
        let result = cpu.get_target_value(&system, target, size).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn target_value_indirect_a() {
        let (mut cpu, system) = init_test();

        let size = Size::Long;
        let expected_addr = INIT_ADDR;
        let expected = 0x12345678;
        let target = Target::IndirectAReg(2);
        system.get_bus().write_beu32(INIT_ADDR, expected).unwrap();

        cpu.state.a_reg[2] = INIT_ADDR as u32;
        let result = cpu.get_target_value(&system, target, size).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn target_value_indirect_a_inc() {
        let (mut cpu, system) = init_test();

        let size = Size::Long;
        let expected_addr = INIT_ADDR;
        let expected = 0x12345678;
        let target = Target::IndirectARegInc(2);
        system.get_bus().write_beu32(INIT_ADDR, expected).unwrap();

        cpu.state.a_reg[2] = INIT_ADDR as u32;
        let result = cpu.get_target_value(&system, target, size).unwrap();
        assert_eq!(result, expected);
        assert_eq!(cpu.state.a_reg[2], (INIT_ADDR as u32) + 4);
    }

    #[test]
    fn target_value_indirect_a_dec() {
        let (mut cpu, system) = init_test();

        let size = Size::Long;
        let expected_addr = INIT_ADDR + 4;
        let expected = 0x12345678;
        let target = Target::IndirectARegDec(2);
        system.get_bus().write_beu32(INIT_ADDR, expected).unwrap();

        cpu.state.a_reg[2] = (INIT_ADDR as u32) + 4;
        let result = cpu.get_target_value(&system, target, size).unwrap();
        assert_eq!(result, expected);
        assert_eq!(cpu.state.a_reg[2], INIT_ADDR as u32);
    }


    #[test]
    fn target_value_immediate() {
        let (mut cpu, system) = init_test();

        let size = Size::Word;
        let expected = 0x1234;

        let target = Target::Immediate(expected);

        let result = cpu.get_target_value(&system, target, size).unwrap();
        assert_eq!(result, expected);
    }
}


