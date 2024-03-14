
#[cfg(test)]
mod decode_unit_tests {
    use std::rc::Rc;
    use std::cell::RefCell;
    use femtos::Instant;
    use emulator_hal::bus::{BusAccess, BusAdapter};

    use moa_core::{Bus, BusPort, Address, Addressable, MemoryBlock, Device, Error};

    use crate::M68kType;
    use crate::instructions::{Target, Size, XRegister, BaseRegister, IndexRegister};
    use crate::decode::{M68kDecoder, InstructionDecoding};
    use crate::memory::M68kBusPort;

    const INIT_ADDR: u32 = 0x00000000;

    fn init_decode_test<'a>(cputype: M68kType) -> InstructionDecoding<'a, BusAdapter<u32, u64, Instant, &'a mut dyn Addressable, Error>> {
        let bus = Rc::new(RefCell::new(Bus::default()));
        let mem = MemoryBlock::new(vec![0; 0x0000100]);
        bus.borrow_mut().insert(0x00000000, Device::new(mem));

        let mut bus = bus.borrow_mut();
        let mut adapter: BusAdapter<u32, u64, Instant, &mut dyn Addressable, Error> = BusAdapter::new(
            &mut *bus,
            |addr| addr as u64,
            |err| err.try_into().unwrap(),
        );

        let mut decoder = M68kDecoder::new(cputype, true, 0);
        let mut decoding = InstructionDecoding {
            port: &mut adapter,
            memory: &mut M68kBusPort::default(),
            decoder: &mut decoder,
        };
        decoding
    }

    //
    // Addressing Mode Target Tests
    //

    #[test]
    fn target_direct_d() {
        let mut decoder = init_decode_test(M68kType::MC68010);

        let size = Size::Word;

        let target = decoder.get_mode_as_target(0b000, 0b001, Some(size)).unwrap();
        assert_eq!(target, Target::DirectDReg(1));
    }

    #[test]
    fn target_direct_a() {
        let mut decoder = init_decode_test(M68kType::MC68010);

        let size = Size::Word;

        let target = decoder.get_mode_as_target(0b001, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::DirectAReg(2));
    }

    #[test]
    fn target_indirect_a() {
        let mut decoder = init_decode_test(M68kType::MC68010);

        let size = Size::Long;
        let expected = 0x12345678;

        decoder.port.write_beu32(Instant::START, INIT_ADDR, expected).unwrap();

        let target = decoder.get_mode_as_target(0b010, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectAReg(2));
    }

    #[test]
    fn target_indirect_a_inc() {
        let mut decoder = init_decode_test(M68kType::MC68010);

        let size = Size::Long;
        let expected = 0x12345678;

        decoder.port.write_beu32(Instant::START, INIT_ADDR, expected).unwrap();

        let target = decoder.get_mode_as_target(0b011, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectARegInc(2));
    }

    #[test]
    fn target_indirect_a_dec() {
        let mut decoder = init_decode_test(M68kType::MC68010);

        let size = Size::Long;
        let expected = 0x12345678;

        decoder.port.write_beu32(Instant::START, INIT_ADDR, expected).unwrap();

        let target = decoder.get_mode_as_target(0b100, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectARegDec(2));
    }

    #[test]
    fn target_indirect_a_reg_offset() {
        let mut decoder = init_decode_test(M68kType::MC68010);

        let size = Size::Long;
        let offset = -8;

        decoder.port.write_beu16(Instant::START, INIT_ADDR, (offset as i16) as u16).unwrap();

        let target = decoder.get_mode_as_target(0b101, 0b100, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectRegOffset(BaseRegister::AReg(4), None, offset));
    }

    #[test]
    fn target_indirect_a_reg_brief_extension_word() {
        let mut decoder = init_decode_test(M68kType::MC68010);

        let size = Size::Long;
        let offset = -8;
        let brief_extension = 0x3800 | (((offset as i8) as u8) as u16);

        decoder.port.write_beu16(Instant::START, INIT_ADDR, brief_extension).unwrap();
        decoder.port.write_beu16(Instant::START, INIT_ADDR + 2, (offset as i16) as u16).unwrap();

        let target = decoder.get_mode_as_target(0b110, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectRegOffset(BaseRegister::AReg(2), Some(IndexRegister { xreg: XRegister::DReg(3), scale: 0, size: size }), offset));
    }

    #[test]
    fn target_indirect_a_reg_full_extension_word() {
        let mut decoder = init_decode_test(M68kType::MC68020);

        let size = Size::Word;
        let offset = -1843235 as i32;
        let brief_extension = 0xF330;

        decoder.port.write_beu16(Instant::START, INIT_ADDR, brief_extension).unwrap();
        decoder.port.write_beu32(Instant::START, INIT_ADDR + 2, offset as u32).unwrap();

        let target = decoder.get_mode_as_target(0b110, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectRegOffset(BaseRegister::AReg(2), Some(IndexRegister { xreg: XRegister::AReg(7), scale: 1, size: size }), offset));
    }

    #[test]
    fn target_indirect_a_reg_full_extension_word_no_base() {
        let mut decoder = init_decode_test(M68kType::MC68020);

        let size = Size::Word;
        let offset = -1843235 as i32;
        let brief_extension = 0xF3B0;

        decoder.port.write_beu16(Instant::START, INIT_ADDR, brief_extension).unwrap();
        decoder.port.write_beu32(Instant::START, INIT_ADDR + 2, offset as u32).unwrap();

        let target = decoder.get_mode_as_target(0b110, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectRegOffset(BaseRegister::None, Some(IndexRegister { xreg: XRegister::AReg(7), scale: 1, size: size }), offset));
    }

    #[test]
    fn target_indirect_a_reg_full_extension_word_no_index() {
        let mut decoder = init_decode_test(M68kType::MC68020);

        let size = Size::Word;
        let offset = -1843235 as i32;
        let brief_extension = 0xF370;

        decoder.port.write_beu16(Instant::START, INIT_ADDR, brief_extension).unwrap();
        decoder.port.write_beu32(Instant::START, INIT_ADDR + 2, offset as u32).unwrap();

        let target = decoder.get_mode_as_target(0b110, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectRegOffset(BaseRegister::AReg(2), None, offset));
    }

    #[test]
    fn target_indirect_pc_offset() {
        let mut decoder = init_decode_test(M68kType::MC68010);

        let size = Size::Long;
        let offset = -8;

        decoder.port.write_beu16(Instant::START, INIT_ADDR, (offset as i16) as u16).unwrap();

        let target = decoder.get_mode_as_target(0b111, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectRegOffset(BaseRegister::PC, None, offset));
    }

    #[test]
    fn target_indirect_pc_brief_extension_word() {
        let mut decoder = init_decode_test(M68kType::MC68010);

        let size = Size::Word;
        let offset = -8;
        let brief_extension = 0x3000 | (((offset as i8) as u8) as u16);

        decoder.port.write_beu16(Instant::START, INIT_ADDR, brief_extension).unwrap();
        decoder.port.write_beu16(Instant::START, INIT_ADDR + 2, (offset as i16) as u16).unwrap();

        let target = decoder.get_mode_as_target(0b111, 0b011, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectRegOffset(BaseRegister::PC, Some(IndexRegister { xreg: XRegister::DReg(3), scale: 0, size: size }), offset));
    }

    #[test]
    fn target_indirect_pc_full_extension_word() {
        let mut decoder = init_decode_test(M68kType::MC68020);

        let size = Size::Word;
        let offset = -1843235 as i32;
        let brief_extension = 0xF330;

        decoder.port.write_beu16(Instant::START, INIT_ADDR, brief_extension).unwrap();
        decoder.port.write_beu32(Instant::START, INIT_ADDR + 2, offset as u32).unwrap();

        let target = decoder.get_mode_as_target(0b111, 0b011, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectRegOffset(BaseRegister::PC, Some(IndexRegister { xreg: XRegister::AReg(7), scale: 1, size: size }), offset));
    }


    #[test]
    fn target_indirect_immediate_word() {
        let mut decoder = init_decode_test(M68kType::MC68010);

        let size = Size::Word;
        let expected = 0x1234;

        decoder.port.write_beu16(Instant::START, INIT_ADDR, expected as u16).unwrap();

        let target = decoder.get_mode_as_target(0b111, 0b000, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectMemory(expected, Size::Word));
    }

    #[test]
    fn target_indirect_immediate_long() {
        let mut decoder = init_decode_test(M68kType::MC68010);

        let size = Size::Word;
        let expected = 0x12345678;

        decoder.port.write_beu32(Instant::START, INIT_ADDR, expected).unwrap();

        let target = decoder.get_mode_as_target(0b111, 0b001, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectMemory(expected, Size::Long));
    }

    #[test]
    fn target_immediate() {
        let mut decoder = init_decode_test(M68kType::MC68010);

        let size = Size::Word;
        let expected = 0x1234;

        decoder.port.write_beu16(Instant::START, INIT_ADDR, expected as u16).unwrap();

        let target = decoder.get_mode_as_target(0b111, 0b100, Some(size)).unwrap();
        assert_eq!(target, Target::Immediate(expected));
    }
}


#[cfg(test)]
mod execute_unit_tests {
    use femtos::{Instant, Frequency};
    use emulator_hal::bus::{BusAdapter, BusAccess};

    use moa_core::{System, MemoryBlock, Addressable, Steppable, Device, Error};

    use crate::{M68k, M68kType};
    use crate::execute::{Used, M68kCycle, M68kCycleExecutor};
    use crate::instructions::{Instruction, Target, Size};

    const INIT_STACK: u32 = 0x00002000;
    const INIT_ADDR: u32 = 0x00000010;

    fn run_execute_test<F>(cputype: M68kType, mut test_func: F)
    where
        F: FnMut(M68kCycleExecutor<&mut BusAdapter<u32, u64, Instant, &mut dyn Addressable, Error>>),
    {
        let mut system = System::default();

        // Insert basic initialization
        let data = vec![0; 0x00100000];
        let mem = MemoryBlock::new(data);
        system.add_addressable_device(0x00000000, Device::new(mem)).unwrap();
        system.get_bus().write_beu32(system.clock, 0, INIT_STACK as u32).unwrap();
        system.get_bus().write_beu32(system.clock, 4, INIT_ADDR as u32).unwrap();

        let mut cpu = M68k::from_type(cputype, Frequency::from_mhz(10));
        cpu.step(&system).unwrap();
        let mut cycle = M68kCycle::new(&mut cpu, system.clock);

        let mut bus = system.bus.borrow_mut();
        let mut adapter: BusAdapter<u32, u64, Instant, &mut dyn Addressable, Error> = BusAdapter::new(
            &mut *bus,
            |addr| addr as u64,
            |err| err.try_into().unwrap(),
        );

        let mut executor = cycle.begin(&mut cpu, &mut adapter);
        executor.cycle.decoder.init(true, executor.state.pc);
        assert_eq!(executor.state.pc, INIT_ADDR as u32);
        assert_eq!(executor.state.ssp, INIT_STACK as u32);
        assert_eq!(executor.cycle.decoder.instruction, Instruction::NOP);

        test_func(executor);
    }

    //
    // Addressing Mode Target Tests
    //

    #[test]
    fn target_value_direct_d() {
        run_execute_test(M68kType::MC68010, |mut cycle| {
            let size = Size::Word;
            let expected = 0x1234;
            let target = Target::DirectDReg(1);

            cycle.state.d_reg[1] = expected;
            let result = cycle.get_target_value(target, size, Used::Once).unwrap();
            assert_eq!(result, expected);
        });
    }

    #[test]
    fn target_value_direct_a() {
        run_execute_test(M68kType::MC68010, |mut cycle| {
            let size = Size::Word;
            let expected = 0x1234;
            let target = Target::DirectAReg(2);

            cycle.state.a_reg[2] = expected;
            let result = cycle.get_target_value(target, size, Used::Once).unwrap();
            assert_eq!(result, expected);
        });
    }

    #[test]
    fn target_value_indirect_a() {
        run_execute_test(M68kType::MC68010, |mut cycle| {
            let size = Size::Long;
            let expected = 0x12345678;
            let target = Target::IndirectAReg(2);
            cycle.port.write_beu32(Instant::START, INIT_ADDR, expected).unwrap();

            cycle.state.a_reg[2] = INIT_ADDR as u32;
            let result = cycle.get_target_value(target, size, Used::Once).unwrap();
            assert_eq!(result, expected);
        });
    }

    #[test]
    fn target_value_indirect_a_inc() {
        run_execute_test(M68kType::MC68010, |mut cycle| {
            let size = Size::Long;
            let expected = 0x12345678;
            let target = Target::IndirectARegInc(2);
            cycle.port.write_beu32(Instant::START, INIT_ADDR, expected).unwrap();

            cycle.state.a_reg[2] = INIT_ADDR as u32;
            let result = cycle.get_target_value(target, size, Used::Once).unwrap();
            assert_eq!(result, expected);
            assert_eq!(cycle.state.a_reg[2], (INIT_ADDR as u32) + 4);
        });
    }

    #[test]
    fn target_value_indirect_a_dec() {
        run_execute_test(M68kType::MC68010, |mut cycle| {
            let size = Size::Long;
            let expected = 0x12345678;
            let target = Target::IndirectARegDec(2);
            cycle.port.write_beu32(Instant::START, INIT_ADDR, expected).unwrap();

            cycle.state.a_reg[2] = (INIT_ADDR as u32) + 4;
            let result = cycle.get_target_value(target, size, Used::Once).unwrap();
            assert_eq!(result, expected);
            assert_eq!(cycle.state.a_reg[2], INIT_ADDR as u32);
        });
    }


    #[test]
    fn target_value_immediate() {
        run_execute_test(M68kType::MC68010, |mut cycle| {
            let size = Size::Word;
            let expected = 0x1234;

            let target = Target::Immediate(expected);

            let result = cycle.get_target_value(target, size, Used::Once).unwrap();
            assert_eq!(result, expected);
        });
    }
}
