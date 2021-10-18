
#[cfg(test)]
mod decode_tests {
    use crate::system::System;
    use crate::memory::MemoryBlock;
    use crate::devices::{Address, Addressable, Steppable, TransmutableBox, wrap_transmutable, MAX_READ};

    use crate::cpus::m68k::{M68k, M68kType};
    use crate::cpus::m68k::instructions::{Instruction, Target, Size, Sign, XRegister, ShiftDirection};

    const INIT_STACK: Address = 0x00002000;
    const INIT_ADDR: Address = 0x00000010;

    fn init_decode_test() -> (M68k, System) {
        let mut system = System::new();

        // Insert basic initialization
        let data = vec![0; 0x00100000];
        let mem = MemoryBlock::new(data);
        system.add_addressable_device(0x00000000, wrap_transmutable(mem)).unwrap();
        system.get_bus().write_beu32(0, INIT_STACK as u32).unwrap();
        system.get_bus().write_beu32(4, INIT_ADDR as u32).unwrap();

        // Initialize the CPU and make sure it's in the expected state
        let mut cpu = M68k::new(M68kType::MC68010);
        cpu.init(&system).unwrap();
        assert_eq!(cpu.state.pc, INIT_ADDR as u32);
        assert_eq!(cpu.state.msp, INIT_STACK as u32);
        assert_eq!(cpu.decoder.instruction, Instruction::NOP);
        (cpu, system)
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
        let (mut cpu, system) = init_decode_test();

        let size = Size::Word;
        let expected = 0x1234;

        let memory = get_decode_memory(&mut cpu, &system);
        let target = cpu.decoder.get_mode_as_target(memory.borrow_mut().as_addressable().unwrap(), 0b000, 0b001, Some(size)).unwrap();
        assert_eq!(target, Target::DirectDReg(1));
    }

    #[test]
    fn target_direct_a() {
        let (mut cpu, system) = init_decode_test();

        let size = Size::Word;
        let expected = 0x1234;

        let memory = get_decode_memory(&mut cpu, &system);
        let target = cpu.decoder.get_mode_as_target(memory.borrow_mut().as_addressable().unwrap(), 0b001, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::DirectAReg(2));
    }

    #[test]
    fn target_indirect_a() {
        let (mut cpu, system) = init_decode_test();

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
        let (mut cpu, system) = init_decode_test();

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
        let (mut cpu, system) = init_decode_test();

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
        let (mut cpu, system) = init_decode_test();

        let size = Size::Long;
        let offset = -8;

        system.get_bus().write_beu16(INIT_ADDR, (offset as i16) as u16).unwrap();

        let memory = get_decode_memory(&mut cpu, &system);
        let target = cpu.decoder.get_mode_as_target(memory.borrow_mut().as_addressable().unwrap(), 0b101, 0b100, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectARegOffset(4, offset));
    }

    #[test]
    fn target_indirect_a_reg_extension_word() {
        let (mut cpu, system) = init_decode_test();

        let size = Size::Long;
        let offset = -8;
        let brief_extension = 0x3800 | (((offset as i8) as u8) as u16);

        system.get_bus().write_beu16(INIT_ADDR, brief_extension).unwrap();
        system.get_bus().write_beu16(INIT_ADDR + 2, (offset as i16) as u16).unwrap();

        let memory = get_decode_memory(&mut cpu, &system);
        let target = cpu.decoder.get_mode_as_target(memory.borrow_mut().as_addressable().unwrap(), 0b110, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectARegXRegOffset(2, XRegister::Data(3), offset, 0, size));
    }

    #[test]
    fn target_indirect_immediate_word() {
        let (mut cpu, system) = init_decode_test();

        let size = Size::Word;
        let expected = 0x1234;

        system.get_bus().write_beu16(INIT_ADDR, expected as u16).unwrap();

        let memory = get_decode_memory(&mut cpu, &system);
        let target = cpu.decoder.get_mode_as_target(memory.borrow_mut().as_addressable().unwrap(), 0b111, 0b000, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectMemory(expected));
    }

    #[test]
    fn target_indirect_immediate_long() {
        let (mut cpu, system) = init_decode_test();

        let size = Size::Word;
        let expected = 0x12345678;

        system.get_bus().write_beu32(INIT_ADDR, expected).unwrap();

        let memory = get_decode_memory(&mut cpu, &system);
        let target = cpu.decoder.get_mode_as_target(memory.borrow_mut().as_addressable().unwrap(), 0b111, 0b001, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectMemory(expected));
    }

    #[test]
    fn target_indirect_pc_offset() {
        let (mut cpu, system) = init_decode_test();

        let size = Size::Long;
        let offset = -8;

        system.get_bus().write_beu16(INIT_ADDR, (offset as i16) as u16).unwrap();

        let memory = get_decode_memory(&mut cpu, &system);
        let target = cpu.decoder.get_mode_as_target(memory.borrow_mut().as_addressable().unwrap(), 0b111, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectPCOffset(offset));
    }

    #[test]
    fn target_indirect_pc_extension_word() {
        let (mut cpu, system) = init_decode_test();

        let size = Size::Word;
        let offset = -8;
        let brief_extension = 0x3000 | (((offset as i8) as u8) as u16);

        system.get_bus().write_beu16(INIT_ADDR, brief_extension).unwrap();
        system.get_bus().write_beu16(INIT_ADDR + 2, (offset as i16) as u16).unwrap();

        let memory = get_decode_memory(&mut cpu, &system);
        let target = cpu.decoder.get_mode_as_target(memory.borrow_mut().as_addressable().unwrap(), 0b111, 0b011, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectPCXRegOffset(XRegister::Data(3), offset, 0, size));
    }

    #[test]
    fn target_immediate() {
        let (mut cpu, system) = init_decode_test();

        let size = Size::Word;
        let expected = 0x1234;

        system.get_bus().write_beu16(INIT_ADDR, expected as u16).unwrap();

        let memory = get_decode_memory(&mut cpu, &system);
        let target = cpu.decoder.get_mode_as_target(memory.borrow_mut().as_addressable().unwrap(), 0b111, 0b100, Some(size)).unwrap();
        assert_eq!(target, Target::Immediate(expected));
    }

    //
    // Instruction Decode Tests
    //

    #[test]
    fn instruction_nop() {
        let (mut cpu, system) = init_decode_test();

        system.get_bus().write_beu16(INIT_ADDR, 0x4e71).unwrap();
        cpu.decode_next(&system).unwrap();
        assert_eq!(cpu.decoder.instruction, Instruction::NOP);
    }

    #[test]
    fn instruction_ori() {
        let (mut cpu, system) = init_decode_test();

        system.get_bus().write_beu16(INIT_ADDR,     0x0008).unwrap();
        system.get_bus().write_beu16(INIT_ADDR + 2, 0x00FF).unwrap();
        cpu.decode_next(&system).unwrap();
        assert_eq!(cpu.decoder.instruction, Instruction::OR(Target::Immediate(0xFF), Target::DirectAReg(0), Size::Byte));
    }

    #[test]
    fn instruction_cmpi_equal() {
        let (mut cpu, system) = init_decode_test();

        system.get_bus().write_beu16(INIT_ADDR,     0x7020).unwrap();
        system.get_bus().write_beu16(INIT_ADDR + 2, 0x0C00).unwrap();
        system.get_bus().write_beu16(INIT_ADDR + 4, 0x0020).unwrap();
        cpu.step(&system).unwrap();
        cpu.decode_next(&system).unwrap();
        assert_eq!(cpu.decoder.instruction, Instruction::CMP(Target::Immediate(0x20), Target::DirectDReg(0), Size::Byte));
    }

    #[test]
    fn instruction_cmpi_greater() {
        let (mut cpu, system) = init_decode_test();

        system.get_bus().write_beu16(INIT_ADDR,     0x7020).unwrap();
        system.get_bus().write_beu16(INIT_ADDR + 2, 0x0C00).unwrap();
        system.get_bus().write_beu16(INIT_ADDR + 4, 0x0030).unwrap();
        cpu.step(&system).unwrap();
        cpu.decode_next(&system).unwrap();
        assert_eq!(cpu.decoder.instruction, Instruction::CMP(Target::Immediate(0x30), Target::DirectDReg(0), Size::Byte));
    }

    #[test]
    fn instruction_cmpi_less() {
        let (mut cpu, system) = init_decode_test();

        system.get_bus().write_beu16(INIT_ADDR,     0x7020).unwrap();
        system.get_bus().write_beu16(INIT_ADDR + 2, 0x0C00).unwrap();
        system.get_bus().write_beu16(INIT_ADDR + 4, 0x0010).unwrap();
        cpu.step(&system).unwrap();
        cpu.decode_next(&system).unwrap();
        assert_eq!(cpu.decoder.instruction, Instruction::CMP(Target::Immediate(0x10), Target::DirectDReg(0), Size::Byte));
    }

    #[test]
    fn instruction_andi_sr() {
        let (mut cpu, system) = init_decode_test();

        system.get_bus().write_beu16(INIT_ADDR,     0x027C).unwrap();
        system.get_bus().write_beu16(INIT_ADDR + 2, 0xF8FF).unwrap();
        cpu.decode_next(&system).unwrap();
        assert_eq!(cpu.decoder.instruction, Instruction::ANDtoSR(0xF8FF));
    }

    #[test]
    fn instruction_muls() {
        let (mut cpu, system) = init_decode_test();

        system.get_bus().write_beu16(INIT_ADDR,     0xC1FC).unwrap();
        system.get_bus().write_beu16(INIT_ADDR + 2, 0x0276).unwrap();
        cpu.decode_next(&system).unwrap();
        assert_eq!(cpu.decoder.instruction, Instruction::MUL(Target::Immediate(0x276), Target::DirectDReg(0), Size::Word, Sign::Signed));
    }

    #[test]
    fn instruction_asli() {
        let (mut cpu, system) = init_decode_test();

        system.get_bus().write_beu16(INIT_ADDR, 0xE300).unwrap();
        cpu.decode_next(&system).unwrap();
        assert_eq!(cpu.decoder.instruction, Instruction::ASd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Left));
    }

    #[test]
    fn instruction_asri() {
        let (mut cpu, system) = init_decode_test();

        system.get_bus().write_beu16(INIT_ADDR, 0xE200).unwrap();
        cpu.decode_next(&system).unwrap();
        assert_eq!(cpu.decoder.instruction, Instruction::ASd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Right));
    }

    #[test]
    fn instruction_roli() {
        let (mut cpu, system) = init_decode_test();

        system.get_bus().write_beu16(INIT_ADDR, 0xE318).unwrap();
        cpu.decode_next(&system).unwrap();
        assert_eq!(cpu.decoder.instruction, Instruction::ROd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Left));
    }

    #[test]
    fn instruction_rori() {
        let (mut cpu, system) = init_decode_test();

        system.get_bus().write_beu16(INIT_ADDR, 0xE218).unwrap();
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

        let mut cpu = M68k::new(M68kType::MC68010);
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

        let previous = cpu.state.clone();
        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state, previous);
    }


    #[test]
    fn instruction_ori() {
        let (mut cpu, system) = init_test();

        cpu.decoder.instruction = Instruction::OR(Target::Immediate(0xFF), Target::DirectAReg(0), Size::Byte);

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state.a_reg[0], 0x000000FF);
    }

    #[test]
    fn instruction_cmpi_equal() {
        let (mut cpu, system) = init_test();

        let value = 0x20;
        cpu.state.d_reg[0] = value;
        cpu.decoder.instruction = Instruction::CMP(Target::Immediate(value), Target::DirectDReg(0), Size::Byte);

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state.sr & 0x0F, 0x04);
    }

    #[test]
    fn instruction_cmpi_greater() {
        let (mut cpu, system) = init_test();

        cpu.state.d_reg[0] = 0x20;
        cpu.decoder.instruction = Instruction::CMP(Target::Immediate(0x30), Target::DirectDReg(0), Size::Byte);

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state.sr & 0x0F, 0x09);
    }

    #[test]
    fn instruction_cmpi_less() {
        let (mut cpu, system) = init_test();

        cpu.state.d_reg[0] = 0x20;
        cpu.decoder.instruction = Instruction::CMP(Target::Immediate(0x10), Target::DirectDReg(0), Size::Byte);

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state.sr & 0x0F, 0x00);
    }

    #[test]
    fn instruction_andi_sr() {
        let (mut cpu, system) = init_test();

        cpu.state.sr = 0xA7AA;
        cpu.decoder.instruction = Instruction::ANDtoSR(0xF8FF);

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state.sr, 0xA0AA);
    }

    #[test]
    fn instruction_ori_sr() {
        let (mut cpu, system) = init_test();

        cpu.state.sr = 0xA755;
        cpu.decoder.instruction = Instruction::ORtoSR(0x00AA);

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state.sr, 0xA7FF);
    }

    #[test]
    fn instruction_muls() {
        let (mut cpu, system) = init_test();

        let value = 0x0276;
        cpu.state.d_reg[0] = 0x0200;
        cpu.decoder.instruction = Instruction::MUL(Target::Immediate(value), Target::DirectDReg(0), Size::Word, Sign::Signed);

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state.d_reg[0], 0x4ec00);
    }

    #[test]
    fn instruction_divu() {
        let (mut cpu, system) = init_test();

        let value = 0x0245;
        cpu.state.d_reg[0] = 0x40000;
        cpu.decoder.instruction = Instruction::DIV(Target::Immediate(value), Target::DirectDReg(0), Size::Word, Sign::Unsigned);

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state.d_reg[0], 0x007101C3);
    }

    #[test]
    fn instruction_asli() {
        let (mut cpu, system) = init_test();

        cpu.state.d_reg[0] = 0x01;
        cpu.decoder.instruction = Instruction::ASd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Left);

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state.d_reg[0], 0x00000002);
        assert_eq!(cpu.state.sr & 0x1F, 0x00);
    }

    #[test]
    fn instruction_asri() {
        let (mut cpu, system) = init_test();

        cpu.state.d_reg[0] = 0x81;
        cpu.decoder.instruction = Instruction::ASd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Right);

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state.d_reg[0], 0x000000C0);
        assert_eq!(cpu.state.sr & 0x1F, 0x19);
    }

    #[test]
    fn instruction_roli() {
        let (mut cpu, system) = init_test();

        cpu.state.d_reg[0] = 0x80;
        cpu.decoder.instruction = Instruction::ROd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Left);

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state.d_reg[0], 0x00000001);
        assert_eq!(cpu.state.sr & 0x1F, 0x01);
    }

    #[test]
    fn instruction_rori() {
        let (mut cpu, system) = init_test();

        cpu.state.d_reg[0] = 0x01;
        cpu.decoder.instruction = Instruction::ROd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Right);

        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state.d_reg[0], 0x00000080);
        assert_eq!(cpu.state.sr & 0x1F, 0x09);
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


