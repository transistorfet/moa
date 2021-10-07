
use crate::memory::{Address, Addressable, MemoryBlock};
use crate::system::{System, Steppable, wrap_addressable};

use super::state::MC68010;
use super::decode::Instruction;

const INIT_STACK: Address = 0x00002000;
const INIT_ADDR: Address = 0x00000010;

fn init_test() -> (MC68010, System) {
    let mut system = System::new();

    // Insert basic initialization
    let data = vec![0; 0x00100000];
    let mem = MemoryBlock::new(data);
    system.add_addressable_device(0x00000000, wrap_addressable(mem)).unwrap();
    system.get_bus().write_beu32(0, INIT_STACK as u32).unwrap();
    system.get_bus().write_beu32(4, INIT_ADDR as u32).unwrap();

    let mut cpu = MC68010::new();
    cpu.step(&system).unwrap();
    assert_eq!(cpu.state.pc, INIT_ADDR as u32);
    assert_eq!(cpu.state.msp, INIT_STACK as u32);
    assert_eq!(cpu.decoder.instruction, Instruction::NOP);
    (cpu, system)
}

#[cfg(test)]
mod tests {
    use super::{init_test, INIT_ADDR};
    use crate::memory::{Address, Addressable};
    use super::super::decode::{Instruction, Target, Size, Sign, ShiftDirection};

    #[test]
    fn instruction_nop() {
        let (mut cpu, mut system) = init_test();

        system.get_bus().write_beu16(INIT_ADDR, 0x4e71).unwrap();
        cpu.decode_next(&system).unwrap();
        assert_eq!(cpu.decoder.instruction, Instruction::NOP);
        cpu.execute_current(&system).unwrap();
        // TODO you need a way to easily check the entire state (you maybe need to make a special struct for the state)
    }


    #[test]
    fn instruction_ori() {
        let (mut cpu, mut system) = init_test();

        system.get_bus().write_beu16(INIT_ADDR,     0x0008).unwrap();
        system.get_bus().write_beu16(INIT_ADDR + 2, 0x00FF).unwrap();
        cpu.decode_next(&system).unwrap();
        assert_eq!(cpu.decoder.instruction, Instruction::OR(Target::Immediate(0xFF), Target::DirectAReg(0), Size::Byte));
        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state.a_reg[0], 0x000000FF);
    }

    #[test]
    fn instruction_cmpi_equal() {
        let (mut cpu, mut system) = init_test();

        system.get_bus().write_beu16(INIT_ADDR,     0x7020).unwrap();
        system.get_bus().write_beu16(INIT_ADDR + 2, 0x0C00).unwrap();
        system.get_bus().write_beu16(INIT_ADDR + 4, 0x0020).unwrap();
        cpu.step(&system).unwrap();
        cpu.decode_next(&system).unwrap();
        assert_eq!(cpu.decoder.instruction, Instruction::CMP(Target::Immediate(0x20), Target::DirectDReg(0), Size::Byte));
        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state.sr & 0x0F, 0x04);
    }

    #[test]
    fn instruction_cmpi_greater() {
        let (mut cpu, mut system) = init_test();

        system.get_bus().write_beu16(INIT_ADDR,     0x7020).unwrap();
        system.get_bus().write_beu16(INIT_ADDR + 2, 0x0C00).unwrap();
        system.get_bus().write_beu16(INIT_ADDR + 4, 0x0030).unwrap();
        cpu.step(&system).unwrap();
        cpu.decode_next(&system).unwrap();
        assert_eq!(cpu.decoder.instruction, Instruction::CMP(Target::Immediate(0x30), Target::DirectDReg(0), Size::Byte));
        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state.sr & 0x0F, 0x009);
    }

    #[test]
    fn instruction_cmpi_less() {
        let (mut cpu, mut system) = init_test();

        system.get_bus().write_beu16(INIT_ADDR,     0x7020).unwrap();
        system.get_bus().write_beu16(INIT_ADDR + 2, 0x0C00).unwrap();
        system.get_bus().write_beu16(INIT_ADDR + 4, 0x0010).unwrap();
        cpu.step(&system).unwrap();
        cpu.decode_next(&system).unwrap();
        assert_eq!(cpu.decoder.instruction, Instruction::CMP(Target::Immediate(0x10), Target::DirectDReg(0), Size::Byte));
        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state.sr & 0x0F, 0x00);
    }

    #[test]
    fn instruction_andi_sr() {
        let (mut cpu, mut system) = init_test();

        system.get_bus().write_beu16(INIT_ADDR,     0x027C).unwrap();
        system.get_bus().write_beu16(INIT_ADDR + 2, 0xF8FF).unwrap();
        cpu.decode_next(&system).unwrap();
        assert_eq!(cpu.decoder.instruction, Instruction::ANDtoSR(0xF8FF));
        //cpu.execute_current(&system).unwrap();
        //assert_eq!(cpu.state.sr & 0x0F, 0x00);
    }

    #[test]
    fn instruction_muls() {
        let (mut cpu, mut system) = init_test();

        system.get_bus().write_beu16(INIT_ADDR,     0xC1FC).unwrap();
        system.get_bus().write_beu16(INIT_ADDR + 2, 0x0276).unwrap();
        cpu.decode_next(&system).unwrap();
        assert_eq!(cpu.decoder.instruction, Instruction::MUL(Target::Immediate(0x276), Target::DirectDReg(0), Size::Word, Sign::Signed));
        //cpu.execute_current(&system).unwrap();
        //assert_eq!(cpu.state.sr & 0x0F, 0x00);
    }

    #[test]
    fn instruction_asli() {
        let (mut cpu, mut system) = init_test();

        system.get_bus().write_beu16(INIT_ADDR, 0xE300).unwrap();
        cpu.decode_next(&system).unwrap();
        assert_eq!(cpu.decoder.instruction, Instruction::ASd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Left));

        cpu.state.d_reg[0] = 0x01;
        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state.d_reg[0], 0x00000002);
        assert_eq!(cpu.state.sr & 0x1F, 0x00);
    }

    #[test]
    fn instruction_asri() {
        let (mut cpu, mut system) = init_test();

        system.get_bus().write_beu16(INIT_ADDR, 0xE200).unwrap();
        cpu.decode_next(&system).unwrap();
        assert_eq!(cpu.decoder.instruction, Instruction::ASd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Right));

        cpu.state.d_reg[0] = 0x81;
        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state.d_reg[0], 0x000000C0);
        assert_eq!(cpu.state.sr & 0x1F, 0x19);
    }

    #[test]
    fn instruction_roli() {
        let (mut cpu, mut system) = init_test();

        system.get_bus().write_beu16(INIT_ADDR, 0xE318).unwrap();
        cpu.decode_next(&system).unwrap();
        assert_eq!(cpu.decoder.instruction, Instruction::ROd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Left));

        cpu.state.d_reg[0] = 0x80;
        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state.d_reg[0], 0x00000001);
        assert_eq!(cpu.state.sr & 0x1F, 0x01);
    }

    #[test]
    fn instruction_rori() {
        let (mut cpu, mut system) = init_test();

        system.get_bus().write_beu16(INIT_ADDR, 0xE218).unwrap();
        cpu.decode_next(&system).unwrap();
        assert_eq!(cpu.decoder.instruction, Instruction::ROd(Target::Immediate(1), Target::DirectDReg(0), Size::Byte, ShiftDirection::Right));

        cpu.state.d_reg[0] = 0x01;
        cpu.execute_current(&system).unwrap();
        assert_eq!(cpu.state.d_reg[0], 0x00000080);
        assert_eq!(cpu.state.sr & 0x1F, 0x09);
    }





    #[test]
    fn target_value_direct_d() {
        let (mut cpu, mut system) = init_test();

        let size = Size::Word;
        let expected = 0x1234;

        let target = cpu.decoder.get_mode_as_target(&system, 0b000, 0b001, Some(size)).unwrap();
        assert_eq!(target, Target::DirectDReg(1));

        cpu.state.d_reg[1] = expected;
        let result = cpu.get_target_value(&system, target, size).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn target_value_direct_a() {
        let (mut cpu, mut system) = init_test();

        let size = Size::Word;
        let expected = 0x1234;

        let target = cpu.decoder.get_mode_as_target(&system, 0b001, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::DirectAReg(2));

        cpu.state.a_reg[2] = expected;
        let result = cpu.get_target_value(&system, target, size).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn target_value_indirect_a() {
        let (mut cpu, mut system) = init_test();

        let size = Size::Long;
        let expected_addr = INIT_ADDR;
        let expected = 0x12345678;

        system.get_bus().write_beu32(INIT_ADDR, expected).unwrap();
        let target = cpu.decoder.get_mode_as_target(&system, 0b010, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectAReg(2));

        cpu.state.a_reg[2] = INIT_ADDR as u32;
        let result = cpu.get_target_value(&system, target, size).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn target_value_indirect_a_inc() {
        let (mut cpu, mut system) = init_test();

        let size = Size::Long;
        let expected_addr = INIT_ADDR;
        let expected = 0x12345678;

        system.get_bus().write_beu32(INIT_ADDR, expected).unwrap();
        let target = cpu.decoder.get_mode_as_target(&system, 0b011, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectARegInc(2));

        cpu.state.a_reg[2] = INIT_ADDR as u32;
        let result = cpu.get_target_value(&system, target, size).unwrap();
        assert_eq!(result, expected);
        assert_eq!(cpu.state.a_reg[2], (INIT_ADDR as u32) + 4);
    }

    #[test]
    fn target_value_indirect_a_dec() {
        let (mut cpu, mut system) = init_test();

        let size = Size::Long;
        let expected_addr = INIT_ADDR + 4;
        let expected = 0x12345678;

        system.get_bus().write_beu32(INIT_ADDR, expected).unwrap();
        let target = cpu.decoder.get_mode_as_target(&system, 0b100, 0b010, Some(size)).unwrap();
        assert_eq!(target, Target::IndirectARegDec(2));

        cpu.state.a_reg[2] = (INIT_ADDR as u32) + 4;
        let result = cpu.get_target_value(&system, target, size).unwrap();
        assert_eq!(result, expected);
        assert_eq!(cpu.state.a_reg[2], INIT_ADDR as u32);
    }


    #[test]
    fn target_value_immediate() {
        let (mut cpu, mut system) = init_test();

        let size = Size::Word;
        let expected = 0x1234;

        system.get_bus().write_beu16(cpu.decoder.end as Address, expected as u16).unwrap();
        let target = cpu.decoder.get_mode_as_target(&system, 0b111, 0b100, Some(size)).unwrap();
        assert_eq!(target, Target::Immediate(expected));

        let result = cpu.get_target_value(&system, target, size).unwrap();
        assert_eq!(result, expected);
    }
}

