
#[cfg(test)]
mod decode_unit_tests {
    use std::rc::Rc;
    use std::cell::RefCell;

    use moa_core::{Bus, BusPort, ClockTime, Address, Addressable, MemoryBlock, wrap_transmutable};

    use crate::M68kType;
    use crate::instructions::{Target, Size, XRegister, BaseRegister, IndexRegister};
    use crate::decode::M68kDecoder;

    const INIT_ADDR: Address = 0x00000000;

    fn init_decode_test(cputype: M68kType) -> (BusPort, M68kDecoder) {
        let bus = Rc::new(RefCell::new(Bus::default()));
        let mem = MemoryBlock::new(vec![0; 0x0000100]);
        bus.borrow_mut().insert(0x00000000, wrap_transmutable(mem));

        let port = if cputype <= M68kType::MC68010 {
            BusPort::new(0, 24, 16, bus)
        } else {
            BusPort::new(0, 32, 32, bus)
        };
        let decoder = M68kDecoder::new(cputype, ClockTime::START, 0);
        (port, decoder)
    }

    //
    // Addressing Mode Target Tests
    //

    #[test]
    fn target_direct_d() {
        let (mut port, mut decoder) = init_decode_test(M68kType::MC68010);

        let size = Size::Word;

        let target = decoder.get_mode_as_target(&mut port, 0b000, 0b001, Some(size)).unwrap();
        assert_eq!(target, Target::DirectDReg(1));
    }

    #[test]
    fn target_direct_a() {
        let (mut port, mut decoder) = init_decode_test(M68kType::MC68010);

        let size = Size::Word;

        let target = decoder.get_mode_as_target(&mut port, 0b001, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::DirectAReg(2));
    }

    #[test]
    fn target_indirect_a() {
        let (mut port, mut decoder) = init_decode_test(M68kType::MC68010);

        let size = Size::Long;
        let expected = 0x12345678;

        port.write_beu32(ClockTime::START, INIT_ADDR, expected).unwrap();

        let target = decoder.get_mode_as_target(&mut port, 0b010, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectAReg(2));
    }

    #[test]
    fn target_indirect_a_inc() {
        let (mut port, mut decoder) = init_decode_test(M68kType::MC68010);

        let size = Size::Long;
        let expected = 0x12345678;

        port.write_beu32(ClockTime::START, INIT_ADDR, expected).unwrap();

        let target = decoder.get_mode_as_target(&mut port, 0b011, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectARegInc(2));
    }

    #[test]
    fn target_indirect_a_dec() {
        let (mut port, mut decoder) = init_decode_test(M68kType::MC68010);

        let size = Size::Long;
        let expected = 0x12345678;

        port.write_beu32(ClockTime::START, INIT_ADDR, expected).unwrap();

        let target = decoder.get_mode_as_target(&mut port, 0b100, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectARegDec(2));
    }

    #[test]
    fn target_indirect_a_reg_offset() {
        let (mut port, mut decoder) = init_decode_test(M68kType::MC68010);

        let size = Size::Long;
        let offset = -8;

        port.write_beu16(ClockTime::START, INIT_ADDR, (offset as i16) as u16).unwrap();

        let target = decoder.get_mode_as_target(&mut port, 0b101, 0b100, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectRegOffset(BaseRegister::AReg(4), None, offset));
    }

    #[test]
    fn target_indirect_a_reg_brief_extension_word() {
        let (mut port, mut decoder) = init_decode_test(M68kType::MC68010);

        let size = Size::Long;
        let offset = -8;
        let brief_extension = 0x3800 | (((offset as i8) as u8) as u16);

        port.write_beu16(ClockTime::START, INIT_ADDR, brief_extension).unwrap();
        port.write_beu16(ClockTime::START, INIT_ADDR + 2, (offset as i16) as u16).unwrap();

        let target = decoder.get_mode_as_target(&mut port, 0b110, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectRegOffset(BaseRegister::AReg(2), Some(IndexRegister { xreg: XRegister::DReg(3), scale: 0, size: size }), offset));
    }

    #[test]
    fn target_indirect_a_reg_full_extension_word() {
        let (mut port, mut decoder) = init_decode_test(M68kType::MC68020);

        let size = Size::Word;
        let offset = -1843235 as i32;
        let brief_extension = 0xF330;

        port.write_beu16(ClockTime::START, INIT_ADDR, brief_extension).unwrap();
        port.write_beu32(ClockTime::START, INIT_ADDR + 2, offset as u32).unwrap();

        let target = decoder.get_mode_as_target(&mut port, 0b110, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectRegOffset(BaseRegister::AReg(2), Some(IndexRegister { xreg: XRegister::AReg(7), scale: 1, size: size }), offset));
    }

    #[test]
    fn target_indirect_a_reg_full_extension_word_no_base() {
        let (mut port, mut decoder) = init_decode_test(M68kType::MC68020);

        let size = Size::Word;
        let offset = -1843235 as i32;
        let brief_extension = 0xF3B0;

        port.write_beu16(ClockTime::START, INIT_ADDR, brief_extension).unwrap();
        port.write_beu32(ClockTime::START, INIT_ADDR + 2, offset as u32).unwrap();

        let target = decoder.get_mode_as_target(&mut port, 0b110, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectRegOffset(BaseRegister::None, Some(IndexRegister { xreg: XRegister::AReg(7), scale: 1, size: size }), offset));
    }

    #[test]
    fn target_indirect_a_reg_full_extension_word_no_index() {
        let (mut port, mut decoder) = init_decode_test(M68kType::MC68020);

        let size = Size::Word;
        let offset = -1843235 as i32;
        let brief_extension = 0xF370;

        port.write_beu16(ClockTime::START, INIT_ADDR, brief_extension).unwrap();
        port.write_beu32(ClockTime::START, INIT_ADDR + 2, offset as u32).unwrap();

        let target = decoder.get_mode_as_target(&mut port, 0b110, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectRegOffset(BaseRegister::AReg(2), None, offset));
    }

    #[test]
    fn target_indirect_pc_offset() {
        let (mut port, mut decoder) = init_decode_test(M68kType::MC68010);

        let size = Size::Long;
        let offset = -8;

        port.write_beu16(ClockTime::START, INIT_ADDR, (offset as i16) as u16).unwrap();

        let target = decoder.get_mode_as_target(&mut port, 0b111, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectRegOffset(BaseRegister::PC, None, offset));
    }

    #[test]
    fn target_indirect_pc_brief_extension_word() {
        let (mut port, mut decoder) = init_decode_test(M68kType::MC68010);

        let size = Size::Word;
        let offset = -8;
        let brief_extension = 0x3000 | (((offset as i8) as u8) as u16);

        port.write_beu16(ClockTime::START, INIT_ADDR, brief_extension).unwrap();
        port.write_beu16(ClockTime::START, INIT_ADDR + 2, (offset as i16) as u16).unwrap();

        let target = decoder.get_mode_as_target(&mut port, 0b111, 0b011, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectRegOffset(BaseRegister::PC, Some(IndexRegister { xreg: XRegister::DReg(3), scale: 0, size: size }), offset));
    }

    #[test]
    fn target_indirect_pc_full_extension_word() {
        let (mut port, mut decoder) = init_decode_test(M68kType::MC68020);

        let size = Size::Word;
        let offset = -1843235 as i32;
        let brief_extension = 0xF330;

        port.write_beu16(ClockTime::START, INIT_ADDR, brief_extension).unwrap();
        port.write_beu32(ClockTime::START, INIT_ADDR + 2, offset as u32).unwrap();

        let target = decoder.get_mode_as_target(&mut port, 0b111, 0b011, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectRegOffset(BaseRegister::PC, Some(IndexRegister { xreg: XRegister::AReg(7), scale: 1, size: size }), offset));
    }


    #[test]
    fn target_indirect_immediate_word() {
        let (mut port, mut decoder) = init_decode_test(M68kType::MC68010);

        let size = Size::Word;
        let expected = 0x1234;

        port.write_beu16(ClockTime::START, INIT_ADDR, expected as u16).unwrap();

        let target = decoder.get_mode_as_target(&mut port, 0b111, 0b000, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectMemory(expected, Size::Word));
    }

    #[test]
    fn target_indirect_immediate_long() {
        let (mut port, mut decoder) = init_decode_test(M68kType::MC68010);

        let size = Size::Word;
        let expected = 0x12345678;

        port.write_beu32(ClockTime::START, INIT_ADDR, expected).unwrap();

        let target = decoder.get_mode_as_target(&mut port, 0b111, 0b001, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectMemory(expected, Size::Long));
    }

    #[test]
    fn target_immediate() {
        let (mut port, mut decoder) = init_decode_test(M68kType::MC68010);

        let size = Size::Word;
        let expected = 0x1234;

        port.write_beu16(ClockTime::START, INIT_ADDR, expected as u16).unwrap();

        let target = decoder.get_mode_as_target(&mut port, 0b111, 0b100, Some(size)).unwrap();
        assert_eq!(target, Target::Immediate(expected));
    }
}


#[cfg(test)]
mod execute_unit_tests {
    use moa_core::{System, MemoryBlock, BusPort, ClockTime, Frequency, Address, Addressable, Steppable, wrap_transmutable};

    use crate::{M68k, M68kType};
    use crate::execute::Used;
    use crate::instructions::{Instruction, Target, Size};

    const INIT_STACK: Address = 0x00002000;
    const INIT_ADDR: Address = 0x00000010;

    fn init_execute_test(cputype: M68kType) -> (M68k, System) {
        let mut system = System::default();

        // Insert basic initialization
        let data = vec![0; 0x00100000];
        let mem = MemoryBlock::new(data);
        system.add_addressable_device(0x00000000, wrap_transmutable(mem)).unwrap();
        system.get_bus().write_beu32(system.clock, 0, INIT_STACK as u32).unwrap();
        system.get_bus().write_beu32(system.clock, 4, INIT_ADDR as u32).unwrap();

        let port = if cputype <= M68kType::MC68010 {
            BusPort::new(0, 24, 16, system.bus.clone())
        } else {
            BusPort::new(0, 24, 16, system.bus.clone())
        };
        let mut cpu = M68k::new(cputype, Frequency::from_mhz(10), port);
        cpu.step(&system).unwrap();
        cpu.decoder.init(system.clock, cpu.state.pc);
        assert_eq!(cpu.state.pc, INIT_ADDR as u32);
        assert_eq!(cpu.state.ssp, INIT_STACK as u32);
        assert_eq!(cpu.decoder.instruction, Instruction::NOP);
        (cpu, system)
    }

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
        let result = cpu.get_target_value(target, size, Used::Once).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn target_value_direct_a() {
        let (mut cpu, _) = init_execute_test(M68kType::MC68010);

        let size = Size::Word;
        let expected = 0x1234;
        let target = Target::DirectAReg(2);

        cpu.state.a_reg[2] = expected;
        let result = cpu.get_target_value(target, size, Used::Once).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn target_value_indirect_a() {
        let (mut cpu, _) = init_execute_test(M68kType::MC68010);

        let size = Size::Long;
        let expected = 0x12345678;
        let target = Target::IndirectAReg(2);
        cpu.port.write_beu32(ClockTime::START, INIT_ADDR, expected).unwrap();

        cpu.state.a_reg[2] = INIT_ADDR as u32;
        let result = cpu.get_target_value(target, size, Used::Once).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn target_value_indirect_a_inc() {
        let (mut cpu, _) = init_execute_test(M68kType::MC68010);

        let size = Size::Long;
        let expected = 0x12345678;
        let target = Target::IndirectARegInc(2);
        cpu.port.write_beu32(ClockTime::START, INIT_ADDR, expected).unwrap();

        cpu.state.a_reg[2] = INIT_ADDR as u32;
        let result = cpu.get_target_value(target, size, Used::Once).unwrap();
        assert_eq!(result, expected);
        assert_eq!(cpu.state.a_reg[2], (INIT_ADDR as u32) + 4);
    }

    #[test]
    fn target_value_indirect_a_dec() {
        let (mut cpu, _) = init_execute_test(M68kType::MC68010);

        let size = Size::Long;
        let expected = 0x12345678;
        let target = Target::IndirectARegDec(2);
        cpu.port.write_beu32(ClockTime::START, INIT_ADDR, expected).unwrap();

        cpu.state.a_reg[2] = (INIT_ADDR as u32) + 4;
        let result = cpu.get_target_value(target, size, Used::Once).unwrap();
        assert_eq!(result, expected);
        assert_eq!(cpu.state.a_reg[2], INIT_ADDR as u32);
    }


    #[test]
    fn target_value_immediate() {
        let (mut cpu, _) = init_execute_test(M68kType::MC68010);

        let size = Size::Word;
        let expected = 0x1234;

        let target = Target::Immediate(expected);

        let result = cpu.get_target_value(target, size, Used::Once).unwrap();
        assert_eq!(result, expected);
    }
}


