
use crate::error::Error;
use crate::memory::{Address, AddressSpace};

use super::decode::{Instruction, Target, Size, Direction, Condition, ControlRegister, RegisterType};

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

    pub current_instruction_addr: u32,
    pub current_instruction: Instruction,
}

const FLAGS_ON_RESET: u16 = 0x2700;

pub const FLAGS_CARRY: u16 = 0x0001;
pub const FLAGS_OVERFLOW: u16 = 0x0002;
pub const FLAGS_ZERO: u16 = 0x0004;
pub const FLAGS_NEGATIVE: u16 = 0x0008;
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

            current_instruction_addr: 0,
            current_instruction: Instruction::NOP,
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

        self.current_instruction_addr = 0;
        self.current_instruction = Instruction::NOP;
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
            State::Running => {
                self.decode_next(space)?;
                self.execute_current(space)?;
                Ok(())
            },
        }
    }

    pub fn dump_state(&self, space: &AddressSpace) {
        println!("State: {:?}", self.state);
        println!("PC: {:#010x}", self.pc);
        println!("SR: {:#06x}", self.sr);
        for i in 0..7 {
            println!("D{}: {:#010x}        A{}:  {:#010x}", i, self.d_reg[i as usize], i, self.a_reg[i as usize]);
        }
        println!("D7: {:#010x}", self.d_reg[7]);
        println!("MSP: {:#010x}", self.msp);
        println!("USP: {:#010x}", self.usp);

        println!("Current Instruction: {:#010x} {:?}", self.current_instruction_addr, self.current_instruction);
        println!("");
        space.dump_memory(self.msp as Address, 0x40);
        println!("");
    }

    fn is_supervisor(&self) -> bool {
        self.sr & FLAGS_SUPERVISOR != 0
    }

    fn push_long(&mut self, space: &mut AddressSpace, value: u32) -> Result<(), Error> {
        let reg = self.get_stack_pointer_mut();
        *reg -= 4;
        //println!("PUSHING {:08x} at {:08x}", value, *reg);
        space.write_beu32(*reg as Address, value)
    }

    fn pop_long(&mut self, space: &mut AddressSpace) -> Result<u32, Error> {
        let reg = self.get_stack_pointer_mut();
        let value = space.read_beu32(*reg as Address)?;
        //println!("POPPING {:08x} at {:08x}", value, *reg);
        *reg += 4;
        Ok(value)
    }

    fn decode_next(&mut self, space: &mut AddressSpace) -> Result<(), Error> {
        self.current_instruction_addr = self.pc;
        self.current_instruction = self.decode_one(space)?;

/*
        // Print instruction bytes for debugging
        let ins_data: Result<String, Error> =
            (0..((self.pc - current_ins_addr) / 2)).map(|offset|
                Ok(format!("{:04x} ", space.read_beu16((current_ins_addr + (offset * 2)) as Address)?))
            ).collect();
        debug!("{:#010x}: {}\n\t{:?}\n", current_ins_addr, ins_data?, ins);

        // Single Step
        self.dump_state();
        let mut buffer = String::new();
        std::io::stdin().read_line(&mut buffer).unwrap();
*/
        Ok(())
    }

    fn execute_current(&mut self, space: &mut AddressSpace) -> Result<(), Error> {
        match self.current_instruction {
            Instruction::ADD(src, dest, size) => {
                let value = self.get_target_value(space, src, size)?;
                let existing = self.get_target_value(space, dest, size)?;
                let (result, overflow) = match size {
                    Size::Byte => {
                        let (result, overflow) = (existing as u8).overflowing_add(value as u8);
                        (result as u32, overflow)
                    },
                    Size::Word => {
                        let (result, overflow) = (existing as u16).overflowing_add(value as u16);
                        (result as u32, overflow)
                    },
                    Size::Long => existing.overflowing_add(value),
                };
                self.set_compare_flags(result as i32, overflow);
                self.set_target_value(space, dest, result, size)?;
            },
            Instruction::AND(src, dest, size) => {
                let value = self.get_target_value(space, src, size)?;
                let existing = self.get_target_value(space, dest, size)?;
                self.set_target_value(space, dest, existing & value, size)?;
                self.set_logic_flags(value, size);
            },
            Instruction::ANDtoCCR(value) => {
                self.sr = self.sr | value as u16;
            },
            Instruction::ANDtoSR(value) => {
                self.sr = self.sr | value;
            },
            //Instruction::ASd(Target, Target, Size, ShiftDirection) => {
            //},
            Instruction::Bcc(cond, offset) => {
                let should_branch = self.get_current_condition(cond);
                if should_branch {
                    self.pc = self.current_instruction_addr.wrapping_add(offset as u32) + 2;
                }
            },
            Instruction::BRA(offset) => {
                self.pc = self.current_instruction_addr.wrapping_add(offset as u32) + 2;
            },
            Instruction::BSR(offset) => {
                self.push_long(space, self.pc)?;
                self.pc = self.current_instruction_addr.wrapping_add(offset as u32) + 2;
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
            Instruction::CMP(src, dest, size) => {
                let value = self.get_target_value(space, src, size)?;
                let existing = self.get_target_value(space, dest, size)?;
                let result = self.subtract_sized_with_flags(existing, value, size);
            },
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
                self.pc = self.get_target_address(target)? - 2;
            },
            Instruction::JSR(target) => {
                self.push_long(space, self.pc)?;
                self.pc = self.get_target_address(target)? - 2;
            },
            Instruction::LEA(target, reg) => {
                let value = self.get_target_address(target)?;
                let addr = self.get_a_reg_mut(reg);
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
            Instruction::MOVEM(target, size, dir, mask) => {
                // TODO moving words requires a sign extension to 32 bits
                if size != Size::Long { panic!("Unsupported size in MOVEM instruction"); }

                if dir == Direction::ToTarget {
                    let mut mask = mask;
                    for i in 0..8 {
                        if (mask & 0x01) != 0 {
                            self.set_target_value(space, target, self.d_reg[i], size)?;
                        }
                        mask >>= 1;
                    }
                    for i in 0..8 {
                        if (mask & 0x01) != 0 {
                            let value = *self.get_a_reg_mut(i);
                            self.set_target_value(space, target, value, size)?;
                        }
                        mask >>= 1;
                    }
                } else {
                    let mut mask = mask;
                    for i in (0..8).rev() {
                        if (mask & 0x01) != 0 {
                            let value = *self.get_a_reg_mut(i);
                            self.set_target_value(space, target, value, size)?;
                        }
                        mask >>= 1;
                    }
                    for i in (0..8).rev() {
                        if (mask & 0x01) != 0 {
                            self.set_target_value(space, target, self.d_reg[i], size)?;
                        }
                        mask >>= 1;
                    }
                }
            },
            Instruction::MOVEQ(data, reg) => {
                let value = ((data as i8) as i32) as u32;
                self.d_reg[reg as usize] = value;
            },
            //Instruction::MUL(Target, Target, Size, Sign) => {
            //},
            //Instruction::NBCD(Target) => {
            //},
            //Instruction::NEG(Target, Size) => {
            //},
            //Instruction::NEGX(Target, Size) => {
            //},
            Instruction::NOP => { },
            //Instruction::NOT(target, size) => {
            //},
            Instruction::OR(src, dest, size) => {
                let value = self.get_target_value(space, src, size)?;
                let existing = self.get_target_value(space, dest, size)?;
                self.set_target_value(space, dest, existing | value, size)?;
                self.set_logic_flags(value, size);
            },
            Instruction::ORtoCCR(value) => {
                self.sr = self.sr | value as u16;
            },
            Instruction::ORtoSR(value) => {
                self.sr = self.sr | value;
            },
            Instruction::PEA(target) => {
                let value = self.get_target_address(target)?;
                self.push_long(space, value)?;
            },
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
            Instruction::RTS => {
                self.pc = self.pop_long(space)?;
            },
            //Instruction::STOP(u16) => {
            //},
            Instruction::SUB(src, dest, size) => {
                let value = self.get_target_value(space, src, size)?;
                let existing = self.get_target_value(space, dest, size)?;
                let result = self.subtract_sized_with_flags(existing, value, size);
                self.set_target_value(space, dest, result, size)?;
            },
            //Instruction::SWAP(u8) => {
            //},
            //Instruction::TAS(Target) => {
            //},
            Instruction::TST(target, size) => {
                let value = self.get_target_value(space, target, size)?;
                self.set_compare_flags(value as i32, false);
            },
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
            Target::DirectAReg(reg) => Ok(get_value_sized(*self.get_a_reg_mut(reg), size)),
            Target::IndirectAReg(reg) => get_address_sized(space, *self.get_a_reg_mut(reg) as Address, size),
            Target::IndirectARegInc(reg) => {
                let addr = self.get_a_reg_mut(reg);
                let result = get_address_sized(space, *addr as Address, size);
                *addr += size.in_bytes();
                result
            },
            Target::IndirectARegDec(reg) => {
                let addr = self.get_a_reg_mut(reg);
                *addr -= size.in_bytes();
                get_address_sized(space, *addr as Address, size)
            },
            Target::IndirectARegOffset(reg, offset) => {
                let addr = self.get_a_reg_mut(reg);
                get_address_sized(space, (*addr).wrapping_add(offset as u32) as Address, size)
            },
            Target::IndirectARegXRegOffset(reg, rtype, xreg, offset, target_size) => {
                let reg_offset = get_value_sized(self.get_x_reg_value(rtype, xreg), target_size);
                let addr = self.get_a_reg_mut(reg);
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
        }
    }

    fn set_target_value(&mut self, space: &mut AddressSpace, target: Target, value: u32, size: Size) -> Result<(), Error> {
        match target {
            Target::DirectDReg(reg) => {
                set_value_sized(&mut self.d_reg[reg as usize], value, size);
            },
            Target::DirectAReg(reg) => {
                set_value_sized(self.get_a_reg_mut(reg), value, size);
            },
            Target::IndirectAReg(reg) => {
                set_address_sized(space, *self.get_a_reg_mut(reg) as Address, value, size)?;
            },
            Target::IndirectARegInc(reg) => {
                let addr = self.get_a_reg_mut(reg);
                set_address_sized(space, *addr as Address, value, size)?;
                *addr += size.in_bytes();
            },
            Target::IndirectARegDec(reg) => {
                let addr = self.get_a_reg_mut(reg);
                *addr -= size.in_bytes();
                set_address_sized(space, *addr as Address, value, size)?;
            },
            Target::IndirectARegOffset(reg, offset) => {
                let addr = self.get_a_reg_mut(reg);
                set_address_sized(space, (*addr).wrapping_add(offset as u32) as Address, value, size)?;
            },
            Target::IndirectARegXRegOffset(reg, rtype, xreg, offset, target_size) => {
                let reg_offset = get_value_sized(self.get_x_reg_value(rtype, xreg), target_size);
                let addr = self.get_a_reg_mut(reg);
                set_address_sized(space, (*addr).wrapping_add(reg_offset).wrapping_add(offset as u32) as Address, value, size)?;
            },
            Target::IndirectMemory(addr) => {
                set_address_sized(space, addr as Address, value, size)?;
            },
            _ => return Err(Error::new(&format!("Unimplemented addressing target: {:?}", target))),
        }
        Ok(())
    }

    fn get_target_address(&mut self, target: Target) -> Result<u32, Error> {
        let addr = match target {
            Target::IndirectAReg(reg) => *self.get_a_reg_mut(reg),
            Target::IndirectARegOffset(reg, offset) => {
                let addr = self.get_a_reg_mut(reg);
                (*addr).wrapping_add(offset as u32)
            },
            Target::IndirectARegXRegOffset(reg, rtype, xreg, offset, target_size) => {
                let reg_offset = get_value_sized(self.get_x_reg_value(rtype, xreg), target_size);
                let addr = self.get_a_reg_mut(reg);
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
            _ => return Err(Error::new(&format!("Invalid addressing target: {:?}", target))),
        };
        Ok(addr)
    }

    fn subtract_sized_with_flags(&mut self, existing: u32, diff: u32, size: Size) -> u32 {
        let (result, overflow) = match size {
            Size::Byte => {
                let (result, overflow) = (existing as u8).overflowing_sub(diff as u8);
                (result as u32, overflow)
            },
            Size::Word => {
                let (result, overflow) = (existing as u16).overflowing_sub(diff as u16);
                (result as u32, overflow)
            },
            Size::Long => existing.overflowing_sub(diff),
        };
        self.set_compare_flags(result as i32, overflow);
        result
    }

    fn get_control_reg_mut(&mut self, control_reg: ControlRegister) -> &mut u32 {
        match control_reg {
            ControlRegister::VBR => &mut self.vbr,
        }
    }

    #[inline(always)]
    fn get_stack_pointer_mut(&mut self) -> &mut u32 {
        if self.is_supervisor() { &mut self.msp } else { &mut self.usp }
    }

    #[inline(always)]
    fn get_a_reg_mut(&mut self, reg: u8) -> &mut u32 {
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

    fn get_flag(&self, flag: u16) -> bool {
        if (self.sr & flag) == 0 {
            false
        } else {
            true
        }
    }

    fn set_compare_flags(&mut self, value: i32, carry: bool) {
        let mut flags = 0x0000;
        if value < 0 {
            flags |= FLAGS_NEGATIVE
        }
        if value == 0 {
            flags |= FLAGS_ZERO
        }
        if carry {
            flags |= FLAGS_CARRY | FLAGS_OVERFLOW;
        }
        self.sr |= (self.sr & 0xFFF0) | flags;
    }

    fn set_logic_flags(&mut self, value: u32, size: Size) {
        let mut flags = 0x0000;
        match size {
            Size::Byte if value & 0x80 != 0 => flags |= FLAGS_NEGATIVE,
            Size::Word if value & 0x8000 != 0 => flags |= FLAGS_NEGATIVE,
            Size::Long if value & 0x80000000 != 0 => flags |= FLAGS_NEGATIVE,
            _ => { },
        }
        if value == 0 {
            flags |= FLAGS_ZERO
        }
        self.sr |= (self.sr & 0xFFF0) | flags;
    }


    fn get_current_condition(&self, cond: Condition) -> bool {
        match cond {
            Condition::True => true,
            Condition::False => false,
            Condition::High => !self.get_flag(FLAGS_CARRY) && !self.get_flag(FLAGS_ZERO),
            Condition::LowOrSame => self.get_flag(FLAGS_CARRY) || self.get_flag(FLAGS_ZERO),
            Condition::CarryClear => !self.get_flag(FLAGS_CARRY),
            Condition::CarrySet => self.get_flag(FLAGS_CARRY),
            Condition::NotEqual => !self.get_flag(FLAGS_ZERO),
            Condition::Equal => self.get_flag(FLAGS_ZERO),
            Condition::OverflowClear => !self.get_flag(FLAGS_OVERFLOW),
            Condition::OverflowSet => self.get_flag(FLAGS_OVERFLOW),
            Condition::Plus => !self.get_flag(FLAGS_NEGATIVE),
            Condition::Minus => self.get_flag(FLAGS_NEGATIVE),
            Condition::GreaterThanOrEqual => (self.get_flag(FLAGS_NEGATIVE) && self.get_flag(FLAGS_OVERFLOW)) || (!self.get_flag(FLAGS_NEGATIVE) && !self.get_flag(FLAGS_OVERFLOW)),
            Condition::LessThan => (self.get_flag(FLAGS_NEGATIVE) && !self.get_flag(FLAGS_OVERFLOW)) || (!self.get_flag(FLAGS_NEGATIVE) && self.get_flag(FLAGS_OVERFLOW)),
            Condition::GreaterThan =>
                (self.get_flag(FLAGS_NEGATIVE) && self.get_flag(FLAGS_OVERFLOW) && !self.get_flag(FLAGS_ZERO))
                || (!self.get_flag(FLAGS_NEGATIVE) && !self.get_flag(FLAGS_OVERFLOW) && !self.get_flag(FLAGS_ZERO)),
            Condition::LessThanOrEqual =>
                self.get_flag(FLAGS_ZERO)
                || (self.get_flag(FLAGS_NEGATIVE) && !self.get_flag(FLAGS_OVERFLOW))
                || (!self.get_flag(FLAGS_NEGATIVE) && self.get_flag(FLAGS_OVERFLOW)),
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
