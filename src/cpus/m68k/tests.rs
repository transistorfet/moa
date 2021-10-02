
use crate::memory::{Address, AddressSpace, MemoryBlock};

use super::execute::MC68010;
use super::decode::{Instruction, Target, Size};

const INIT_STACK: Address = 0x00002000;
const INIT_ADDR: Address = 0x00000010;

fn init_test() -> (MC68010, AddressSpace) {
    let mut space = AddressSpace::new();

    // Insert basic initialization
    let mut data = vec![0; 0x00100000];
    let mem = MemoryBlock::new(data);
    space.insert(0x00000000, Box::new(mem));
    space.write_beu32(0, INIT_STACK as u32).unwrap();
    space.write_beu32(4, INIT_ADDR as u32).unwrap();

    let mut cpu = MC68010::new();
    cpu.step(&mut space).unwrap();
    assert_eq!(cpu.state.pc, INIT_ADDR as u32);
    assert_eq!(cpu.state.msp, INIT_STACK as u32);
    assert_eq!(cpu.decoder.instruction, Instruction::NOP);
    (cpu, space)
}

#[cfg(test)]
mod tests {
    use super::{init_test, INIT_ADDR};
    use super::{Instruction, Target, Size};

    #[test]
    fn instruction_nop() {
        let (mut cpu, mut space) = init_test();

        space.write_beu16(INIT_ADDR, 0x4e71).unwrap();
        cpu.decode_next(&mut space).unwrap();
        assert_eq!(cpu.decoder.instruction, Instruction::NOP);
        cpu.execute_current(&mut space).unwrap();
        // TODO you need a way to easily check the entire state (you maybe need to make a special struct for the state)
    }


    #[test]
    fn instruction_ori() {
        let (mut cpu, mut space) = init_test();

        space.write_beu16(INIT_ADDR,     0x0008).unwrap();
        space.write_beu16(INIT_ADDR + 2, 0x00FF).unwrap();
        cpu.decode_next(&mut space).unwrap();
        assert_eq!(cpu.decoder.instruction, Instruction::OR(Target::Immediate(0xFF), Target::DirectAReg(0), Size::Byte));
        cpu.execute_current(&mut space).unwrap();
        assert_eq!(cpu.state.a_reg[0], 0x000000FF);
    }
}

