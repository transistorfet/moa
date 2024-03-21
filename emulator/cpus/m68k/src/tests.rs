#[cfg(test)]
mod decode_unit_tests {
    use femtos::Instant;
    use emulator_hal::bus::BusAccess;
    use emulator_hal_memory::MemoryBlock;

    use crate::M68kType;
    use crate::instructions::{Target, Size, XRegister, BaseRegister, IndexRegister};
    use crate::decode::{M68kDecoder, InstructionDecoding};
    use crate::memory::M68kBusPort;

    const INIT_ADDR: u32 = 0x00000000;

    fn run_decode_test<F>(cputype: M68kType, mut test_func: F)
    where
        F: FnMut(&mut InstructionDecoding<'_, MemoryBlock<u32, Instant>, Instant>),
    {
        let mut memory = MemoryBlock::from(vec![0; 0x0000100]);
        let mut decoder = M68kDecoder::new(cputype, true, 0);
        let mut decoding = InstructionDecoding {
            bus: &mut memory,
            memory: &mut M68kBusPort::default(),
            decoder: &mut decoder,
        };

        test_func(&mut decoding);
    }

    //
    // Addressing Mode Target Tests
    //

    #[test]
    fn target_direct_d() {
        run_decode_test(M68kType::MC68010, |decoder| {
            let size = Size::Word;

            let target = decoder.get_mode_as_target(0b000, 0b001, Some(size)).unwrap();
            assert_eq!(target, Target::DirectDReg(1));
        });
    }

    #[test]
    fn target_direct_a() {
        run_decode_test(M68kType::MC68010, |decoder| {
            let size = Size::Word;

            let target = decoder.get_mode_as_target(0b001, 0b010, Some(size)).unwrap();
            assert_eq!(target, Target::DirectAReg(2));
        });
    }

    #[test]
    fn target_indirect_a() {
        run_decode_test(M68kType::MC68010, |decoder| {
            let size = Size::Long;
            let expected = 0x12345678;

            decoder.bus.write_beu32(Instant::START, INIT_ADDR, expected).unwrap();

            let target = decoder.get_mode_as_target(0b010, 0b010, Some(size)).unwrap();
            assert_eq!(target, Target::IndirectAReg(2));
        });
    }

    #[test]
    fn target_indirect_a_inc() {
        run_decode_test(M68kType::MC68010, |decoder| {
            let size = Size::Long;
            let expected = 0x12345678;

            decoder.bus.write_beu32(Instant::START, INIT_ADDR, expected).unwrap();

            let target = decoder.get_mode_as_target(0b011, 0b010, Some(size)).unwrap();
            assert_eq!(target, Target::IndirectARegInc(2));
        });
    }

    #[test]
    fn target_indirect_a_dec() {
        run_decode_test(M68kType::MC68010, |decoder| {
            let size = Size::Long;
            let expected = 0x12345678;

            decoder.bus.write_beu32(Instant::START, INIT_ADDR, expected).unwrap();

            let target = decoder.get_mode_as_target(0b100, 0b010, Some(size)).unwrap();
            assert_eq!(target, Target::IndirectARegDec(2));
        });
    }

    #[test]
    fn target_indirect_a_reg_offset() {
        run_decode_test(M68kType::MC68010, |decoder| {
            let size = Size::Long;
            let offset = -8;

            decoder
                .bus
                .write_beu16(Instant::START, INIT_ADDR, (offset as i16) as u16)
                .unwrap();

            let target = decoder.get_mode_as_target(0b101, 0b100, Some(size)).unwrap();
            assert_eq!(target, Target::IndirectRegOffset(BaseRegister::AReg(4), None, offset));
        });
    }

    #[test]
    fn target_indirect_a_reg_brief_extension_word() {
        run_decode_test(M68kType::MC68010, |decoder| {
            let size = Size::Long;
            let offset = -8;
            let brief_extension = 0x3800 | (((offset as i8) as u8) as u16);

            decoder.bus.write_beu16(Instant::START, INIT_ADDR, brief_extension).unwrap();
            decoder
                .bus
                .write_beu16(Instant::START, INIT_ADDR + 2, (offset as i16) as u16)
                .unwrap();

            let target = decoder.get_mode_as_target(0b110, 0b010, Some(size)).unwrap();
            assert_eq!(
                target,
                Target::IndirectRegOffset(
                    BaseRegister::AReg(2),
                    Some(IndexRegister {
                        xreg: XRegister::DReg(3),
                        scale: 0,
                        size: size
                    }),
                    offset
                )
            );
        });
    }

    #[test]
    fn target_indirect_a_reg_full_extension_word() {
        run_decode_test(M68kType::MC68020, |decoder| {
            let size = Size::Word;
            let offset = -1843235 as i32;
            let brief_extension = 0xF330;

            decoder.bus.write_beu16(Instant::START, INIT_ADDR, brief_extension).unwrap();
            decoder.bus.write_beu32(Instant::START, INIT_ADDR + 2, offset as u32).unwrap();

            let target = decoder.get_mode_as_target(0b110, 0b010, Some(size)).unwrap();
            assert_eq!(
                target,
                Target::IndirectRegOffset(
                    BaseRegister::AReg(2),
                    Some(IndexRegister {
                        xreg: XRegister::AReg(7),
                        scale: 1,
                        size: size
                    }),
                    offset
                )
            );
        });
    }

    #[test]
    fn target_indirect_a_reg_full_extension_word_no_base() {
        run_decode_test(M68kType::MC68020, |decoder| {
            let size = Size::Word;
            let offset = -1843235 as i32;
            let brief_extension = 0xF3B0;

            decoder.bus.write_beu16(Instant::START, INIT_ADDR, brief_extension).unwrap();
            decoder.bus.write_beu32(Instant::START, INIT_ADDR + 2, offset as u32).unwrap();

            let target = decoder.get_mode_as_target(0b110, 0b010, Some(size)).unwrap();
            assert_eq!(
                target,
                Target::IndirectRegOffset(
                    BaseRegister::None,
                    Some(IndexRegister {
                        xreg: XRegister::AReg(7),
                        scale: 1,
                        size: size
                    }),
                    offset
                )
            );
        });
    }

    #[test]
    fn target_indirect_a_reg_full_extension_word_no_index() {
        run_decode_test(M68kType::MC68020, |decoder| {
            let size = Size::Word;
            let offset = -1843235 as i32;
            let brief_extension = 0xF370;

            decoder.bus.write_beu16(Instant::START, INIT_ADDR, brief_extension).unwrap();
            decoder.bus.write_beu32(Instant::START, INIT_ADDR + 2, offset as u32).unwrap();

            let target = decoder.get_mode_as_target(0b110, 0b010, Some(size)).unwrap();
            assert_eq!(target, Target::IndirectRegOffset(BaseRegister::AReg(2), None, offset));
        });
    }

    #[test]
    fn target_indirect_pc_offset() {
        run_decode_test(M68kType::MC68010, |decoder| {
            let size = Size::Long;
            let offset = -8;

            decoder
                .bus
                .write_beu16(Instant::START, INIT_ADDR, (offset as i16) as u16)
                .unwrap();

            let target = decoder.get_mode_as_target(0b111, 0b010, Some(size)).unwrap();
            assert_eq!(target, Target::IndirectRegOffset(BaseRegister::PC, None, offset));
        });
    }

    #[test]
    fn target_indirect_pc_brief_extension_word() {
        run_decode_test(M68kType::MC68010, |decoder| {
            let size = Size::Word;
            let offset = -8;
            let brief_extension = 0x3000 | (((offset as i8) as u8) as u16);

            decoder.bus.write_beu16(Instant::START, INIT_ADDR, brief_extension).unwrap();
            decoder
                .bus
                .write_beu16(Instant::START, INIT_ADDR + 2, (offset as i16) as u16)
                .unwrap();

            let target = decoder.get_mode_as_target(0b111, 0b011, Some(size)).unwrap();
            assert_eq!(
                target,
                Target::IndirectRegOffset(
                    BaseRegister::PC,
                    Some(IndexRegister {
                        xreg: XRegister::DReg(3),
                        scale: 0,
                        size: size
                    }),
                    offset
                )
            );
        });
    }

    #[test]
    fn target_indirect_pc_full_extension_word() {
        run_decode_test(M68kType::MC68020, |decoder| {
            let size = Size::Word;
            let offset = -1843235 as i32;
            let brief_extension = 0xF330;

            decoder.bus.write_beu16(Instant::START, INIT_ADDR, brief_extension).unwrap();
            decoder.bus.write_beu32(Instant::START, INIT_ADDR + 2, offset as u32).unwrap();

            let target = decoder.get_mode_as_target(0b111, 0b011, Some(size)).unwrap();
            assert_eq!(
                target,
                Target::IndirectRegOffset(
                    BaseRegister::PC,
                    Some(IndexRegister {
                        xreg: XRegister::AReg(7),
                        scale: 1,
                        size: size
                    }),
                    offset
                )
            );
        });
    }


    #[test]
    fn target_indirect_immediate_word() {
        run_decode_test(M68kType::MC68010, |decoder| {
            let size = Size::Word;
            let expected = 0x1234;

            decoder.bus.write_beu16(Instant::START, INIT_ADDR, expected as u16).unwrap();

            let target = decoder.get_mode_as_target(0b111, 0b000, Some(size)).unwrap();
            assert_eq!(target, Target::IndirectMemory(expected, Size::Word));
        });
    }

    #[test]
    fn target_indirect_immediate_long() {
        run_decode_test(M68kType::MC68010, |decoder| {
            let size = Size::Word;
            let expected = 0x12345678;

            decoder.bus.write_beu32(Instant::START, INIT_ADDR, expected).unwrap();

            let target = decoder.get_mode_as_target(0b111, 0b001, Some(size)).unwrap();
            assert_eq!(target, Target::IndirectMemory(expected, Size::Long));
        });
    }

    #[test]
    fn target_immediate() {
        run_decode_test(M68kType::MC68010, |decoder| {
            let size = Size::Word;
            let expected = 0x1234;

            decoder.bus.write_beu16(Instant::START, INIT_ADDR, expected as u16).unwrap();

            let target = decoder.get_mode_as_target(0b111, 0b100, Some(size)).unwrap();
            assert_eq!(target, Target::Immediate(expected));
        });
    }
}

#[cfg(test)]
mod execute_unit_tests {
    use femtos::{Instant, Frequency};
    use emulator_hal::bus::BusAccess;
    use emulator_hal::step::Step;
    use emulator_hal_memory::MemoryBlock;

    use crate::{M68k, M68kType};
    use crate::execute::{Used, M68kCycle, M68kCycleExecutor};
    use crate::instructions::{Instruction, Target, Size};

    const INIT_STACK: u32 = 0x00002000;
    const INIT_ADDR: u32 = 0x00000010;

    #[allow(clippy::uninit_vec)]
    fn run_execute_test<F>(cputype: M68kType, mut test_func: F)
    where
        F: FnMut(M68kCycleExecutor<&mut MemoryBlock<u32, Instant>, Instant>),
    {
        // Insert basic initialization
        let len = 0x10_0000;
        let mut data = Vec::with_capacity(len);
        unsafe {
            data.set_len(len);
        }
        let mut memory = MemoryBlock::from(data);
        memory.write_beu32(Instant::START, 0, INIT_STACK as u32).unwrap();
        memory.write_beu32(Instant::START, 4, INIT_ADDR as u32).unwrap();

        let mut cpu = M68k::from_type(cputype, Frequency::from_mhz(10));
        cpu.step(Instant::START, &mut memory).unwrap();
        let cycle = M68kCycle::new(&mut cpu, Instant::START);

        let mut executor = cycle.begin(&mut cpu, &mut memory);
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
            cycle.bus.write_beu32(Instant::START, INIT_ADDR, expected).unwrap();

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
            cycle.bus.write_beu32(Instant::START, INIT_ADDR, expected).unwrap();

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
            cycle.bus.write_beu32(Instant::START, INIT_ADDR, expected).unwrap();

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
