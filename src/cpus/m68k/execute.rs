
use crate::error::Error;
use crate::memory::{Address, AddressSpace};

use super::decode::{Instruction, Target, Size, Direction, ControlRegister, RegisterType};

pub trait Processor {
    fn reset();
    fn step();
}



#[derive(Copy, Clone, Debug, PartialEq)]
pub enum State {
    Init,
    Running,
    Halted,
}

pub struct MC68010 {
    pub state: State,

    pub pc: u32,
    pub sr: u16,
    pub d_reg: [u32; 8],
    pub a_reg: [u32; 7],
    pub msp: u32,
    pub usp: u32,

    pub vbr: u32,
}

const FLAGS_ON_RESET: u16 = 0x2700;

pub const FLAGS_SUPERVISOR: u16 = 0x2000;

pub const ERR_BUS_ERROR: u32 = 2;
pub const ERR_ADDRESS_ERROR: u32 = 3;
pub const ERR_ILLEGAL_INSTRUCTION: u32 = 4;

impl MC68010 {
    pub fn new() -> MC68010 {
        MC68010 {
            state: State::Init,

            pc: 0,
            sr: FLAGS_ON_RESET,
            d_reg: [0; 8],
            a_reg: [0; 7],
            msp: 0,
            usp: 0,

            vbr: 0,
        }
    }

    pub fn reset(&mut self) {
        self.state = State::Init;
        self.pc = 0;
        self.sr = FLAGS_ON_RESET;
        self.d_reg = [0; 8];
        self.a_reg = [0; 7];
        self.msp = 0;
        self.usp = 0;

        self.vbr = 0;
    }

    pub fn is_running(&self) -> bool {
        self.state != State::Halted
    }


    pub fn init(&mut self, space: &mut AddressSpace) -> Result<(), Error> {
        println!("Initializing CPU");

        self.msp = space.read_beu32(0)?;
        self.pc = space.read_beu32(4)?;
        self.state = State::Running;

        Ok(())
    }

    pub fn step(&mut self, space: &mut AddressSpace) -> Result<(), Error> {
        match self.state {
            State::Init => self.init(space),
            State::Halted => Err(Error::new("CPU halted")),
            State::Running => self.execute_one(space),
        }
    }

    fn is_supervisor(&self) -> bool {
        self.sr & FLAGS_SUPERVISOR != 0
    }

    fn push_long(&mut self, space: &mut AddressSpace, value: u32) -> Result<(), Error> {
        let reg = if self.is_supervisor() { &mut self.msp } else { &mut self.usp };
        *reg -= 4;
        space.write_beu32(*reg as Address, value)
    }

    fn execute_one(&mut self, space: &mut AddressSpace) -> Result<(), Error> {
        let addr = self.pc;
        let ins = self.decode_one(space)?;

        println!("{:08x}: {:?}", addr, ins);
        match ins {
            //Instruction::ADD(Target, Target, Size) => {
            //},
            //Instruction::AND(Target, Target, Size) => {
            //},
            //Instruction::ANDtoCCR(u8) => {
            //},
            //Instruction::ANDtoSR(u16) => {
            //},
            //Instruction::ASd(Target, Target, Size, ShiftDirection) => {
            //},
            //Instruction::Bcc(Condition, u16) => {
            //},
            Instruction::BRA(offset) => {
                self.pc = self.pc.wrapping_add(offset as u32) - 2;
            },
            Instruction::BSR(offset) => {
                self.push_long(space, self.pc)?;
                self.pc = self.pc.wrapping_add(offset as u32) - 2;
            },
            //Instruction::BTST(Target, Target, Size) => {
            //},
            //Instruction::BCHG(Target, Target, Size) => {
            //},
            //Instruction::BCLR(Target, Target, Size) => {
            //},
            //Instruction::BSET(Target, Target, Size) => {
            //},
            Instruction::CLR(target, size) => {
                self.set_target_value(space, target, 0, size)?;
            },
            //Instruction::CMP(Target, Target, Size) => {
            //},
            //Instruction::DBcc(Condition, u16) => {
            //},
            //Instruction::DIV(Target, Target, Size, Sign) => {
            //},
            //Instruction::EOR(Target, Target, Size) => {
            //},
            //Instruction::EORtoCCR(u8) => {
            //},
            //Instruction::EORtoSR(u16) => {
            //},
            //Instruction::EXG(Target, Target) => {
            //},
            //Instruction::EXT(u8, Size) => {
            //},
            //Instruction::ILLEGAL => {
            //},
            Instruction::JMP(target) => {
                self.pc = self.get_target_address(target)?;
            },
            Instruction::JSR(target) => {
                self.push_long(space, self.pc)?;
                self.pc = self.get_target_address(target)?;
            },
            Instruction::LEA(target, reg) => {
                let value = self.get_target_address(target)?;
                let addr = self.get_a_reg(reg);
                *addr = value;
            },
            //Instruction::LINK(u8, u16) => {
            //},
            //Instruction::LSd(Target, Target, Size, ShiftDirection) => {
            //},
            Instruction::MOVE(src, dest, size) => {
                let value = self.get_target_value(space, src, size)?;
                self.set_target_value(space, dest, value, size)?;
            },
            Instruction::MOVEfromSR(target) => {
                self.set_target_value(space, target, self.sr as u32, Size::Word)?;
            },
            Instruction::MOVEtoSR(target) => {
                self.sr = self.get_target_value(space, target, Size::Word)? as u16;
            },
            //Instruction::MOVEtoCCR(Target) => {
            //},
            Instruction::MOVEC(target, control_reg, dir) => {
                match dir {
                    Direction::FromTarget => {
                        let value = self.get_target_value(space, target, Size::Long)?;
                        let addr = self.get_control_reg_mut(control_reg);
                        *addr = value;
                    },
                    Direction::ToTarget => {
                        let addr = self.get_control_reg_mut(control_reg);
                        let value = *addr;
                        self.set_target_value(space, target, value, Size::Long)?;
                    },
                }
            },
            //Instruction::MOVEUSP(Target, Direction) => {
            //},
            //Instruction::MOVEM(Target, Size, Direction, u16) => {
            //},
            //Instruction::MOVEQ(u8, u8) => {
            //},
            //Instruction::MUL(Target, Target, Size, Sign) => {
            //},
            //Instruction::NBCD(Target) => {
            //},
            //Instruction::NEG(Target, Size) => {
            //},
            //Instruction::NEGX(Target, Size) => {
            //},
            Instruction::NOP => { },
            //Instruction::NOT(Target, Size) => {
            //},
            //Instruction::OR(Target, Target, Size) => {
            //},
            //Instruction::ORtoCCR(u8) => {
            //},
            Instruction::ORtoSR(value) => {
                self.sr = self.sr | value;
            },
            //Instruction::PEA(Target) => {
            //},
            //Instruction::RESET => {
            //},
            //Instruction::ROd(Target, Target, Size, ShiftDirection) => {
            //},
            //Instruction::ROXd(Target, Target, Size, ShiftDirection) => {
            //},
            //Instruction::RTE => {
            //},
            //Instruction::RTR => {
            //},
            //Instruction::RTS => {
            //},
            //Instruction::STOP(u16) => {
            //},
            //Instruction::SUB(Target, Target, Size) => {
            //},
            //Instruction::SWAP(u8) => {
            //},
            //Instruction::TAS(Target) => {
            //},
            //Instruction::TST(Target, Size) => {
            //},
            //Instruction::TRAP(u8) => {
            //},
            //Instruction::TRAPV => {
            //},
            //Instruction::UNLK(u8) => {
            //},
            _ => { panic!(""); },
        }

        Ok(())
    }

    fn get_target_value(&mut self, space: &mut AddressSpace, target: Target, size: Size) -> Result<u32, Error> {
        match target {
            Target::Immediate(value) => Ok(value),
            Target::DirectDReg(reg) => Ok(get_value_sized(self.d_reg[reg as usize], size)),
            Target::DirectAReg(reg) => Ok(get_value_sized(*self.get_a_reg(reg), size)),
            Target::IndirectAReg(reg) => get_address_sized(space, *self.get_a_reg(reg) as Address, size),
            Target::IndirectARegInc(reg) => {
                let addr = self.get_a_reg(reg);
                let value = get_address_sized(space, *addr as Address, size);
                *addr += size.in_bytes();
                value
            },
            Target::IndirectARegDec(reg) => {
                let addr = self.get_a_reg(reg);
                *addr -= size.in_bytes();
                get_address_sized(space, *addr as Address, size)
            },
            Target::IndirectARegOffset(reg, offset) => {
                let addr = self.get_a_reg(reg);
                get_address_sized(space, (*addr).wrapping_add(offset as u32) as Address, size)
            },
            Target::IndirectARegXRegOffset(reg, rtype, xreg, offset, target_size) => {
                let reg_offset = get_value_sized(self.get_x_reg_value(rtype, xreg), target_size);
                let addr = self.get_a_reg(reg);
                get_address_sized(space, (*addr).wrapping_add(reg_offset).wrapping_add(offset as u32) as Address, size)
            },
            Target::IndirectMemory(addr) => {
                get_address_sized(space, addr as Address, size)
            },
            Target::IndirectPCOffset(offset) => {
                get_address_sized(space, self.pc.wrapping_add(offset as u32) as Address, size)
            },
            Target::IndirectPCXRegOffset(rtype, xreg, offset, target_size) => {
                let reg_offset = get_value_sized(self.get_x_reg_value(rtype, xreg), target_size);
                get_address_sized(space, self.pc.wrapping_add(reg_offset).wrapping_add(offset as u32) as Address, size)
            },
            _ => Err(Error::new(&format!("Unimplemented addressing target: {:?}", target))),
        }
    }

    fn set_target_value(&mut self, space: &mut AddressSpace, target: Target, value: u32, size: Size) -> Result<(), Error> {
        match target {
            Target::DirectDReg(reg) => {
                set_value_sized(&mut self.d_reg[reg as usize], value, size);
            },
            Target::DirectAReg(reg) => {
                set_value_sized(self.get_a_reg(reg), value, size);
            },
            Target::IndirectAReg(reg) => {
                set_address_sized(space, *self.get_a_reg(reg) as Address, value, size)?;
            },
            _ => return Err(Error::new(&format!("Unimplemented addressing target: {:?}", target))),
        }
        Ok(())
    }

    fn get_target_address(&mut self, target: Target) -> Result<u32, Error> {
        let addr = match target {
            Target::IndirectAReg(reg) => *self.get_a_reg(reg),
            Target::IndirectARegOffset(reg, offset) => {
                let addr = self.get_a_reg(reg);
                (*addr).wrapping_add(offset as u32)
            },
            Target::IndirectARegXRegOffset(reg, rtype, xreg, offset, target_size) => {
                let reg_offset = get_value_sized(self.get_x_reg_value(rtype, xreg), target_size);
                let addr = self.get_a_reg(reg);
                (*addr).wrapping_add(reg_offset).wrapping_add(offset as u32)
            },
            Target::IndirectMemory(addr) => {
                addr
            },
            Target::IndirectPCOffset(offset) => {
                self.pc.wrapping_add(offset as u32)
            },
            Target::IndirectPCXRegOffset(rtype, xreg, offset, target_size) => {
                let reg_offset = get_value_sized(self.get_x_reg_value(rtype, xreg), target_size);
                self.pc.wrapping_add(reg_offset).wrapping_add(offset as u32)
            },
            _ => return Err(Error::new(&format!("Unimplemented addressing target: {:?}", target))),
        };
        Ok(addr)
    }

    fn get_control_reg_mut(&mut self, control_reg: ControlRegister) -> &mut u32 {
        match control_reg {
            ControlRegister::VBR => &mut self.vbr,
        }
    }

    #[inline(always)]
    fn get_stack_pointer(&mut self) -> &mut u32 {
        if self.is_supervisor() { &mut self.msp } else { &mut self.usp }
    }

    #[inline(always)]
    fn get_a_reg(&mut self, reg: u8) -> &mut u32 {
        if reg == 7 {
            if self.is_supervisor() { &mut self.msp } else { &mut self.usp }
        } else {
            &mut self.a_reg[reg as usize]
        }
    }

    fn get_x_reg_value(&self, rtype: RegisterType, reg: u8) -> u32 {
        match rtype {
            RegisterType::Data => self.d_reg[reg as usize],
            RegisterType::Address => self.d_reg[reg as usize],
        }
    }
}

fn get_value_sized(value: u32, size: Size) -> u32 {
    match size {
        Size::Byte => { 0x000000FF & value },
        Size::Word => { 0x0000FFFF & value },
        Size::Long => { value },
    }
}

fn get_address_sized(space: &mut AddressSpace, addr: Address, size: Size) -> Result<u32, Error> {
    match size {
        Size::Byte => space.read_u8(addr).map(|value| value as u32),
        Size::Word => space.read_beu16(addr).map(|value| value as u32),
        Size::Long => space.read_beu32(addr),
    }
}

fn set_value_sized(addr: &mut u32, value: u32, size: Size) {
    match size {
        Size::Byte => { *addr = (*addr & 0xFFFFFF00) | (0x000000FF & value); }
        Size::Word => { *addr = (*addr & 0xFFFF0000) | (0x0000FFFF & value); }
        Size::Long => { *addr = value; }
    }
}

fn set_address_sized(space: &mut AddressSpace, addr: Address, value: u32, size: Size) -> Result<(), Error> {
    match size {
        Size::Byte => space.write_u8(addr, value as u8),
        Size::Word => space.write_beu16(addr, value as u16),
        Size::Long => space.write_beu32(addr, value),
    }
}


/*
impl Processor for MC68010 {

}
*/
