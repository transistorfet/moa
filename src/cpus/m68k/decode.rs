
use crate::error::Error;
use crate::memory::{Address, AddressSpace};

use super::execute::MC68010;
use super::execute::ERR_ILLEGAL_INSTRUCTION;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Sign {
    Signed,
    Unsigned,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Direction {
    FromTarget,
    ToTarget,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ShiftDirection {
    Right,
    Left,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum RegisterType {
    Data,
    Address,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ControlRegister {
    VBR,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Size {
    Byte,
    Word,
    Long,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Condition {
    True,
    False,
    High,
    LowOrSame,
    CarryClear,
    CarrySet,
    NotEqual,
    Equal,
    OverflowClear,
    OverflowSet,
    Plus,
    Minus,
    GreaterThanOrEqual,
    LessThan,
    GreaterThan,
    LessThanOrEqual,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Target {
    Immediate(u32),
    DirectDReg(u8),
    DirectAReg(u8),
    IndirectAReg(u8),
    IndirectARegInc(u8),
    IndirectARegDec(u8),
    IndirectARegOffset(u8, i32),
    IndirectARegXRegOffset(u8, RegisterType, u8, i32, Size),
    IndirectMemory(u32),
    IndirectPCOffset(i32),
    IndirectPCXRegOffset(RegisterType, u8, i32, Size),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Instruction {
    //ABCD
    ADD(Target, Target, Size),
    AND(Target, Target, Size),
    ANDtoCCR(u8),
    ANDtoSR(u16),
    ASd(Target, Target, Size, ShiftDirection),

    Bcc(Condition, i16),
    BRA(i16),
    BSR(i16),
    BTST(Target, Target, Size),
    BCHG(Target, Target, Size),
    BCLR(Target, Target, Size),
    BSET(Target, Target, Size),

    CLR(Target, Size),
    CMP(Target, Target, Size),

    DBcc(Condition, u16),
    DIV(Target, Target, Size, Sign),

    EOR(Target, Target, Size),
    EORtoCCR(u8),
    EORtoSR(u16),
    EXG(Target, Target),
    EXT(u8, Size),

    ILLEGAL,

    JMP(Target),
    JSR(Target),

    LEA(Target, u8),
    LINK(u8, u16),
    LSd(Target, Target, Size, ShiftDirection),

    MOVE(Target, Target, Size),
    MOVEfromSR(Target),
    MOVEtoSR(Target),
    MOVEtoCCR(Target),
    MOVEC(Target, ControlRegister, Direction),
    MOVEUSP(Target, Direction),
    MOVEM(Target, Size, Direction, u16),
    MOVEQ(u8, u8),
    MUL(Target, Target, Size, Sign),

    NBCD(Target),
    NEG(Target, Size),
    NEGX(Target, Size),

    NOP,
    NOT(Target, Size),

    OR(Target, Target, Size),
    ORtoCCR(u8),
    ORtoSR(u16),

    PEA(Target),

    RESET,
    ROd(Target, Target, Size, ShiftDirection),
    ROXd(Target, Target, Size, ShiftDirection),
    RTE,
    RTR,
    RTS,

    //SBCD
    //Scc
    STOP(u16),
    SUB(Target, Target, Size),
    SWAP(u8),

    TAS(Target),
    TST(Target, Size),
    TRAP(u8),
    TRAPV,

    UNLK(u8),
}


const OPCG_BIT_OPS: u8 = 0x0;
const OPCG_MOVE_BYTE: u8 = 0x1;
const OPCG_MOVE_LONG: u8 = 0x2;
const OPCG_MOVE_WORD: u8 = 0x3;
const OPCG_MISC: u8 = 0x04;
const OPCG_ADDQ_SUBQ: u8 = 0x5;
const OPCG_BRANCH: u8 = 0x6;
const OPCG_MOVEQ: u8 = 0x7;
const OPCG_DIV_OR: u8 = 0x8;
const OPCG_SUB: u8 = 0x9;
const OPCG_RESERVED1: u8 = 0xA;
const OPCG_CMP_EOR: u8 = 0xB;
const OPCG_MUL_AND: u8 = 0xC;
const OPCG_ADD: u8 = 0xD;
const OPCG_SHIFT: u8 = 0xE;
const OPCG_RESERVED2: u8 = 0xF;


impl MC68010 {
    fn read_instruction_word(&mut self, space: &mut AddressSpace) -> Result<u16, Error> {
        let word = space.read_beu16(self.pc as Address)?;
        //debug!("{:#010x} {:#06x?}", self.pc, word);
        self.pc += 2;
        Ok(word)
    }

    fn read_instruction_long(&mut self, space: &mut AddressSpace) -> Result<u32, Error> {
        let word = space.read_beu32(self.pc as Address)?;
        //debug!("{:#010x} {:#010x}", self.pc, word);
        self.pc += 4;
        Ok(word)
    }

    pub fn decode_one(&mut self, space: &mut AddressSpace) -> Result<Instruction, Error> {
        let ins = self.read_instruction_word(space)?;

        match ((ins & 0xF000) >> 12) as u8 {
            OPCG_BIT_OPS => {
                let optype = (ins & 0x0F00) >> 8;

                if (ins & 0x3F) == 0b111100 {
                    match (ins & 0x00C0) >> 6 {
                        0b00 => {
                            let data = self.read_instruction_word(space)?;
                            match optype {
                                0b0000 => Ok(Instruction::ORtoCCR(data as u8)),
                                0b0001 => Ok(Instruction::ANDtoCCR(data as u8)),
                                0b1010 => Ok(Instruction::EORtoCCR(data as u8)),
                                _ => return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION)),
                            }
                        },
                        0b01 => {
                            let data = self.read_instruction_word(space)?;
                            match optype {
                                0b0000 => Ok(Instruction::ORtoSR(data)),
                                0b0001 => Ok(Instruction::ANDtoSR(data)),
                                0b1010 => Ok(Instruction::EORtoSR(data)),
                                _ => return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION)),
                            }
                        },
                        _ => return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION)),
                    }
                } else if (ins & 0x0100) == 0x0100 || (ins & 0x0F00) == 0x0800 {
                    let bitnum = if (ins & 0x0100) == 0x0100 {
                        Target::DirectDReg(get_high_reg(ins))
                    } else {
                        Target::Immediate(self.read_instruction_word(space)? as u32)
                    };

                    let target = self.decode_lower_effective_address(space, ins, Some(Size::Byte))?;
                    let size = match target {
                        Target::DirectAReg(_) | Target::DirectDReg(_) => Size::Long,
                        _ => Size::Byte,
                    };

                    match (ins & 0x00C0) >> 6 {
                        0b00 => Ok(Instruction::BTST(bitnum, target, size)),
                        0b01 => Ok(Instruction::BCHG(bitnum, target, size)),
                        0b10 => Ok(Instruction::BCLR(bitnum, target, size)),
                        0b11 => Ok(Instruction::BSET(bitnum, target, size)),
                        _ => return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION)),
                    }

                } else {
                    let size = get_size(ins);
                    let target = self.decode_lower_effective_address(space, ins, size)?;
                    let data = match size {
                        Some(Size::Long) => self.read_instruction_long(space)?,
                        Some(_) => self.read_instruction_word(space)? as u32,
                        None => return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION)),
                    };

                    match optype {
                        0b0000 => Ok(Instruction::OR(Target::Immediate(data), target, size.unwrap())),
                        0b0010 => Ok(Instruction::AND(Target::Immediate(data), target, size.unwrap())),
                        0b0100 => Ok(Instruction::SUB(Target::Immediate(data), target, size.unwrap())),
                        0b0110 => Ok(Instruction::ADD(Target::Immediate(data), target, size.unwrap())),
                        0b1010 => Ok(Instruction::EOR(Target::Immediate(data), target, size.unwrap())),
                        0b1100 => Ok(Instruction::CMP(Target::Immediate(data), target, size.unwrap())),
                        _ => return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION)),
                    }
                }
            },
            OPCG_MOVE_BYTE => {
                let src = self.decode_lower_effective_address(space, ins, Some(Size::Byte))?;
                let dest = self.decode_upper_effective_address(space, ins, Some(Size::Byte))?;
                Ok(Instruction::MOVE(src, dest, Size::Byte))
            },
            OPCG_MOVE_LONG => {
                let src = self.decode_lower_effective_address(space, ins, Some(Size::Long))?;
                let dest = self.decode_upper_effective_address(space, ins, Some(Size::Long))?;
                Ok(Instruction::MOVE(src, dest, Size::Long))
            },
            OPCG_MOVE_WORD => {
                let src = self.decode_lower_effective_address(space, ins, Some(Size::Word))?;
                let dest = self.decode_upper_effective_address(space, ins, Some(Size::Word))?;
                Ok(Instruction::MOVE(src, dest, Size::Word))
            },
            OPCG_MISC => {
                if (ins & 0b000101000000) == 0b000100000000 {
                    // CHK Instruction
                    panic!("Not Implemented");
                } else if (ins & 0b000111000000) == 0b000111000000 {
                    let src = self.decode_lower_effective_address(space, ins, None)?;
                    let dest = get_high_reg(ins);
                    Ok(Instruction::LEA(src, dest))
                } else if (ins & 0b100000000000) == 0b000000000000 {
                    let target = self.decode_lower_effective_address(space, ins, Some(Size::Word))?;
                    match (ins & 0x0700) >> 8 {
                        0b000 => {
                            match get_size(ins) {
                                Some(size) => Ok(Instruction::NEGX(target, size)),
                                None => Ok(Instruction::MOVEfromSR(target)),
                            }
                        },
                        0b010 => {
                            match get_size(ins) {
                                Some(size) => Ok(Instruction::CLR(target, size)),
                                None => return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION)),
                            }
                        },
                        0b100 => {
                            match get_size(ins) {
                                Some(size) => Ok(Instruction::NEG(target, size)),
                                None => Ok(Instruction::MOVEtoCCR(target)),
                            }
                        },
                        0b110 => {
                            match get_size(ins) {
                                Some(size) => Ok(Instruction::NOT(target, size)),
                                None => Ok(Instruction::MOVEtoSR(target)),
                            }
                        },
                        _ => return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION)),
                    }
                } else if (ins & 0b101110000000) == 0b100010000000 {
                    let mode = get_low_mode(ins);
                    let size = if (ins & 0x0040) == 0 { Size::Word } else { Size::Long };

                    if mode == 0b000 {
                        Ok(Instruction::EXT(get_low_reg(ins), size))
                    } else {
                        let target = self.decode_lower_effective_address(space, ins, None)?;
                        let data = self.read_instruction_word(space)?;
                        let dir = if (ins & 0x0200) == 0 { Direction::ToTarget } else { Direction::FromTarget };
                        Ok(Instruction::MOVEM(target, size, dir, data))
                    }
                } else if (ins & 0b111100000000) == 0b100000000000 {
                    let subselect = (ins & 0x01C0) >> 6;
                    let mode = get_low_mode(ins);
                    match (subselect, mode) {
                        (0b000, _) => {
                            let target = self.decode_lower_effective_address(space, ins, Some(Size::Byte))?;
                            Ok(Instruction::NBCD(target))
                        },
                        (0b001, 0b000) => {
                            Ok(Instruction::SWAP(get_low_reg(ins)))
                        },
                        (0b001, _) => {
                            let target = self.decode_lower_effective_address(space, ins, None)?;
                            Ok(Instruction::PEA(target))
                        },
                        _ => return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION)),
                    }
                } else if (ins & 0b111100000000) == 0b101000000000 {
                    let target = self.decode_lower_effective_address(space, ins, Some(Size::Word))?;
                    match get_size(ins) {
                        Some(size) => Ok(Instruction::TST(target, size)),
                        None => Ok(Instruction::TAS(target)),
                    }
                } else if (ins & 0b111110000000) == 0b111010000000 {
                    let target = self.decode_lower_effective_address(space, ins, None)?;
                    if (ins & 0b01000000) == 0 {
                        Ok(Instruction::JSR(target))
                    } else {
                        Ok(Instruction::JMP(target))
                    }
                } else if (ins & 0b111111110000) == 0b111001000000 {
                    Ok(Instruction::TRAP((ins & 0x000F) as u8))
                } else if (ins & 0b111111110000) == 0b111001010000 {
                    let reg = get_low_reg(ins);
                    if (ins & 0b1000) == 0 {
                        let data = self.read_instruction_word(space)?;
                        Ok(Instruction::LINK(reg, data))
                    } else {
                        Ok(Instruction::UNLK(reg))
                    }
                } else if (ins & 0b111111110000) == 0b111001100000 {
                    let reg = get_low_reg(ins);
                    let dir = if (ins & 0b1000) == 0 { Direction::FromTarget } else { Direction::ToTarget };
                    Ok(Instruction::MOVEUSP(Target::DirectAReg(reg), dir))
                } else {
                    match ins & 0x0FFF {
                        0xAFC => Ok(Instruction::ILLEGAL),
                        0xE70 => Ok(Instruction::RESET),
                        0xE71 => Ok(Instruction::NOP),
                        0xE72 => {
                            let data = self.read_instruction_word(space)?;
                            Ok(Instruction::STOP(data))
                        },
                        0xE73 => Ok(Instruction::RTE),
                        0xE75 => Ok(Instruction::RTS),
                        0xE76 => Ok(Instruction::TRAPV),
                        0xE77 => Ok(Instruction::RTR),
                        0xE7A | 0xE7B => {
                            let dir = if ins & 0x01 == 0 { Direction::ToTarget } else { Direction::FromTarget };
                            let ins2 = self.read_instruction_word(space)?;
                            let target = match ins2 & 0x8000 {
                                0 => Target::DirectDReg(((ins2 & 0x7000) >> 12) as u8),
                                _ => Target::DirectAReg(((ins2 & 0x7000) >> 12) as u8),
                            };
                            let creg = match ins2 & 0xFFF {
                                0x801 => ControlRegister::VBR,
                                _ => return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION)),
                            };
                            Ok(Instruction::MOVEC(target, creg, dir))
                        },
                        _ => return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION)),
                    }
                }
            },
            OPCG_ADDQ_SUBQ => {
                let size = match get_size(ins) {
                    Some(size) => size,
                    None => return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION)),
                };

                let target = self.decode_lower_effective_address(space, ins, Some(size))?;
                let mut data = ((ins & 0x0E00) >> 9) as u32;
                if data == 0 {
                    data = 8;
                }

                if (ins & 0x0100) == 0 {
                    Ok(Instruction::ADD(Target::Immediate(data), target, size))
                } else {
                    Ok(Instruction::SUB(Target::Immediate(data), target, size))
                }
            },
            OPCG_BRANCH => {
                let mut disp = ((ins & 0xFF) as i8) as u16;
                if disp == 0 {
                    disp = self.read_instruction_word(space)?;
                }
                let condition = get_condition(ins);
                match condition {
                    Condition::True => Ok(Instruction::BRA(disp as i16)),
                    Condition::False => Ok(Instruction::BSR(disp as i16)),
                    _ => Ok(Instruction::Bcc(condition, disp as i16)),
                }
            },
            OPCG_MOVEQ => {
                // TODO make sure the 9th bit is 0
                let reg = get_high_reg(ins);
                let data = (ins & 0xFF) as u8;
                Ok(Instruction::MOVEQ(data, reg))
            },
            OPCG_DIV_OR => {
                let size = get_size(ins);

                if size.is_none() {
                    let sign = if (ins & 0x0100) == 0 { Sign::Unsigned } else { Sign::Signed };
                    let data_reg = Target::DirectDReg(get_high_reg(ins));
                    let effective_addr = self.decode_lower_effective_address(space, ins, size)?;
                    Ok(Instruction::DIV(effective_addr, data_reg, Size::Word, sign))
                } else if (ins & 0b000111110000) == 0b000100000000 {
                    // TODO SBCD
                    panic!("Not Implemented");
                } else {
                    let data_reg = Target::DirectDReg(get_high_reg(ins));
                    let effective_addr = self.decode_lower_effective_address(space, ins, size)?;
                    let (from, to) = if (ins & 0x0100) == 0 { (effective_addr, data_reg) } else { (data_reg, effective_addr) };
                    Ok(Instruction::OR(from, to, size.unwrap()))
                }
            },
            OPCG_SUB => {
                // TODO need to decode the SUBX instruction (would likely be erroneously decoded atm)
                let reg = get_high_reg(ins);
                let dir = (ins & 0x0100) >> 8;
                let size = get_size(ins);
                match size {
                    Some(size) => {
                        let target = self.decode_lower_effective_address(space, ins, Some(size))?;
                        if dir == 0 {
                            Ok(Instruction::SUB(target, Target::DirectDReg(reg), size))
                        } else {
                            Ok(Instruction::SUB(Target::DirectDReg(reg), target, size))
                        }
                    },
                    None => {
                        let size = if dir == 0 { Size::Word } else { Size::Long };
                        let target = self.decode_lower_effective_address(space, ins, Some(size))?;
                        Ok(Instruction::SUB(target, Target::DirectAReg(reg), size))
                    },
                }
            },
            OPCG_CMP_EOR => {
                let reg = get_high_reg(ins);
                let optype = (ins & 0x0100) >> 8;
                let size = get_size(ins);
                match (optype, size) {
                    (0b1, Some(size)) => {
                        // TODO need to decode the CMPM instruction (mode == 0b001) (would likely be erroneously decoded atm)
                        let target = self.decode_lower_effective_address(space, ins, Some(size))?;
                        Ok(Instruction::EOR(Target::DirectDReg(reg), target, size))
                    },
                    (0b0, Some(size)) => {
                        let target = self.decode_lower_effective_address(space, ins, Some(size))?;
                        Ok(Instruction::CMP(Target::DirectDReg(reg), target, size))
                    },
                    (_, None) => {
                        let size = if optype == 0 { Size::Word } else { Size::Long };
                        let target = self.decode_lower_effective_address(space, ins, Some(size))?;
                        Ok(Instruction::CMP(target, Target::DirectAReg(reg), size))
                    },
                    _ => return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION)),
                }
            },
            OPCG_MUL_AND => {
                let size = get_size(ins);

                if size.is_none() {
                    let sign = if (ins & 0x0100) == 0 { Sign::Unsigned } else { Sign::Signed };
                    let data_reg = Target::DirectDReg(get_high_reg(ins));
                    let effective_addr = self.decode_lower_effective_address(space, ins, size)?;
                    Ok(Instruction::MUL(effective_addr, data_reg, Size::Word, sign))
                } else if (ins & 0b000111110000) == 0b000100000000 {
                    // TODO ABCD or EXG
                    panic!("Not Implemented");
                } else {
                    let data_reg = Target::DirectDReg(get_high_reg(ins));
                    let effective_addr = self.decode_lower_effective_address(space, ins, size)?;
                    let (from, to) = if (ins & 0x0100) == 0 { (effective_addr, data_reg) } else { (data_reg, effective_addr) };
                    Ok(Instruction::AND(from, to, size.unwrap()))
                }
            },
            OPCG_ADD => {
                // TODO need to decode the ADDX instruction (would likely be erroneously decoded atm)
                let reg = get_high_reg(ins);
                let dir = (ins & 0x0100) >> 8;
                let size = get_size(ins);
                match size {
                    Some(size) => {
                        let target = self.decode_lower_effective_address(space, ins, Some(size))?;
                        if dir == 0 {
                            Ok(Instruction::ADD(target, Target::DirectDReg(reg), size))
                        } else {
                            Ok(Instruction::ADD(Target::DirectDReg(reg), target, size))
                        }
                    },
                    None => {
                        let size = if dir == 0 { Size::Word } else { Size::Long };
                        let target = self.decode_lower_effective_address(space, ins, Some(size))?;
                        Ok(Instruction::ADD(target, Target::DirectAReg(reg), size))
                    },
                }
            },
            OPCG_SHIFT => {
                let dir = if (ins & 0x0100) == 0 { ShiftDirection::Right } else { ShiftDirection::Left };
                match get_size(ins) {
                    Some(size) => {
                        let reg = get_low_reg(ins);
                        let rotation = get_high_reg(ins);
                        let count = if (ins & 0x0020) == 0 {
                            Target::Immediate(rotation as u32)
                        } else {
                            Target::DirectDReg(rotation)
                        };

                        match (ins & 0x0018) >> 3 {
                            0b00 => Ok(Instruction::ASd(count, Target::DirectDReg(reg), size, dir)),
                            0b01 => Ok(Instruction::LSd(count, Target::DirectDReg(reg), size, dir)),
                            0b10 => Ok(Instruction::ROXd(count, Target::DirectDReg(reg), size, dir)),
                            0b11 => Ok(Instruction::ROd(count, Target::DirectDReg(reg), size, dir)),
                            _ => return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION)),
                        }
                    },
                    None => {
                        let target = self.decode_lower_effective_address(space, ins, Some(Size::Word))?;
                        let count = Target::Immediate(1);

                        match (ins & 0x0600) >> 9 {
                            0b00 => Ok(Instruction::ASd(count, target, Size::Word, dir)),
                            0b01 => Ok(Instruction::LSd(count, target, Size::Word, dir)),
                            0b10 => Ok(Instruction::ROXd(count, target, Size::Word, dir)),
                            0b11 => Ok(Instruction::ROd(count, target, Size::Word, dir)),
                            _ => return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION)),
                        }
                    },
                }
            },
            _ => return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION)),
        }
    }

    fn decode_lower_effective_address(&mut self, space: &mut AddressSpace, ins: u16, size: Option<Size>) -> Result<Target, Error> {
        let reg = get_low_reg(ins);
        let mode = get_low_mode(ins);
        self.get_mode_as_target(space, mode, reg, size)
    }

    fn decode_upper_effective_address(&mut self, space: &mut AddressSpace, ins: u16, size: Option<Size>) -> Result<Target, Error> {
        let reg = get_high_reg(ins);
        let mode = get_high_mode(ins);
        self.get_mode_as_target(space, mode, reg, size)
    }

    fn decode_brief_extension_word(&self, brief_extension: u16) -> (RegisterType, u8, u16, Size) {
        let data = brief_extension & 0x00FF;
        let xreg = ((brief_extension & 0x7000) >> 12) as u8;
        let size = if (brief_extension & 0x0800) == 0 { Size::Word } else { Size::Long };

        let rtype = if (brief_extension & 0x8000) == 0 { RegisterType::Data } else { RegisterType::Address };

        (rtype, xreg, data, size)
    }

    fn get_mode_as_target(&mut self, space: &mut AddressSpace, mode: u8, reg: u8, size: Option<Size>) -> Result<Target, Error> {
        let value = match mode {
            0b000 => Target::DirectDReg(reg),
            0b001 => Target::DirectAReg(reg),
            0b010 => Target::IndirectAReg(reg),
            0b011 => Target::IndirectARegInc(reg),
            0b100 => Target::IndirectARegDec(reg),
            0b101 => {
                let data = self.read_instruction_word(space)?;
                Target::IndirectARegOffset(reg, (data as i16) as i32)
            },
            0b110 => {
                let brief_extension = self.read_instruction_word(space)?;
                let (rtype, xreg, data, size) = self.decode_brief_extension_word(brief_extension);
                Target::IndirectARegXRegOffset(reg, rtype, xreg, (data as i16) as i32, size)
            },
            0b111 => {
                match reg {
                    0b000 => {
                        let value = self.read_instruction_word(space)? as u32;
                        Target::IndirectMemory(value)
                    },
                    0b001 => {
                        let value = self.read_instruction_long(space)?;
                        Target::IndirectMemory(value)
                    },
                    0b010 => {
                        let data = self.read_instruction_word(space)?;
                        Target::IndirectPCOffset((data as i16) as i32)
                    },
                    0b011 => {
                        let brief_extension = self.read_instruction_word(space)?;
                        let (rtype, xreg, data, size) = self.decode_brief_extension_word(brief_extension);
                        Target::IndirectPCXRegOffset(rtype, xreg, (data as i16) as i32, size)
                    },
                    0b100 => {
                        let data = match size {
                            Some(Size::Byte) | Some(Size::Word) => self.read_instruction_word(space)? as u32,
                            Some(Size::Long) => self.read_instruction_long(space)?,
                            None => return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION)),
                        };
                        Target::Immediate(data)
                    },
                    _ => return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION)),
                }
            },
            _ => return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION)),
        };
        Ok(value)
    }

}

#[inline(always)]
fn get_high_reg(ins: u16) -> u8 {
    ((ins & 0x0E00) >> 9) as u8
}

#[inline(always)]
fn get_low_reg(ins: u16) -> u8 {
    (ins & 0x0007) as u8
}

#[inline(always)]
fn get_high_mode(ins: u16) -> u8 {
    ((ins & 0x01C0) >> 6) as u8
}

#[inline(always)]
fn get_low_mode(ins: u16) -> u8 {
    ((ins & 0x0038) >> 3) as u8
}

#[inline(always)]
fn get_size(ins: u16) -> Option<Size> {
    match (ins & 0x00C0) >> 6 {
        0b00 => Some(Size::Byte),
        0b01 => Some(Size::Word),
        0b10 => Some(Size::Long),
        _ => None,
    }
}

#[inline(always)]
fn get_condition(ins: u16) -> Condition {
    match (ins & 0x0F00) >> 8 {
        0b0000 => Condition::True,
        0b0001 => Condition::False,
        0b0010 => Condition::High,
        0b0011 => Condition::LowOrSame,
        0b0100 => Condition::CarryClear,
        0b0101 => Condition::CarrySet,
        0b0110 => Condition::NotEqual,
        0b0111 => Condition::Equal,
        0b1000 => Condition::OverflowClear,
        0b1001 => Condition::OverflowSet,
        0b1010 => Condition::Plus,
        0b1011 => Condition::Minus,
        0b1100 => Condition::GreaterThanOrEqual,
        0b1101 => Condition::LessThan,
        0b1110 => Condition::GreaterThan,
        0b1111 => Condition::LessThanOrEqual,

        _ => Condition::True,
    }
}


impl Size {
    pub fn in_bytes(&self) -> u32 {
        match self {
            Size::Byte => 1,
            Size::Word => 2,
            Size::Long => 4,
        }
    }

    pub fn in_bits(&self) -> u32 {
        match self {
            Size::Byte => 8,
            Size::Word => 16,
            Size::Long => 32,
        }
    }
}

