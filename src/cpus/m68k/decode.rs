
use std::fmt;

use crate::error::Error;
use crate::system::System;
use crate::memory::Address;
use crate::devices::AddressableDeviceRefMut;

use super::state::{M68kType, ERR_ILLEGAL_INSTRUCTION};


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

    Bcc(Condition, i32),
    BRA(i32),
    BSR(i32),
    BCHG(Target, Target, Size),
    BCLR(Target, Target, Size),
    BSET(Target, Target, Size),
    BTST(Target, Target, Size),
    BKPT(u8),

    CHK(Target, u8, Size),
    CLR(Target, Size),
    CMP(Target, Target, Size),
    CMPA(Target, u8, Size),

    DBcc(Condition, u8, i16),
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
    LINK(u8, i16),
    LSd(Target, Target, Size, ShiftDirection),

    MOVE(Target, Target, Size),
    MOVEA(Target, u8, Size),
    MOVEfromSR(Target),
    MOVEtoSR(Target),
    MOVEfromCCR(Target),
    MOVEtoCCR(Target),
    MOVEC(Target, ControlRegister, Direction),
    MOVEM(Target, Size, Direction, u16),
    MOVEP(u8, Target, Size, Direction),
    MOVEQ(u8, u8),
    MOVEUSP(Target, Direction),
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
    RTD(i16),

    //SBCD
    Scc(Condition, Target),
    STOP(u16),
    SUB(Target, Target, Size),
    SWAP(u8),

    TAS(Target),
    TST(Target, Size),
    TRAP(u8),
    TRAPV,

    UNLK(u8),
}


pub struct M68kDecoder {
    pub cputype: M68kType,
    pub base: u32,
    pub start: u32,
    pub end: u32,
    pub instruction: Instruction,
}

impl M68kDecoder {
    pub fn new(cputype: M68kType, base: u32, start: u32) -> M68kDecoder {
        M68kDecoder {
            cputype,
            base: base,
            start: start,
            end: start,
            instruction: Instruction::NOP,
        }
    }

    #[inline(always)]
    pub fn init(&mut self, base: u32, start: u32) {
        self.base = base;
        self.start = start;
        self.end = start;
    }

    pub fn decode_at(&mut self, system: &System, start: u32) -> Result<(), Error> {
        let (memory, relative_addr) = system.get_bus().get_device_at(start as Address, 12)?;
        self.init(start - relative_addr as u32, start);
        self.instruction = self.decode_one(&mut memory.borrow_mut())?;
        Ok(())
    }

    pub fn decode_one(&mut self, system: &mut AddressableDeviceRefMut<'_>) -> Result<Instruction, Error> {
        let ins = self.read_instruction_word(system)?;

        match ((ins & 0xF000) >> 12) as u8 {
            OPCG_BIT_OPS => {
                let optype = (ins & 0x0F00) >> 8;

                if (ins & 0x3F) == 0b111100 {
                    match (ins & 0x00C0) >> 6 {
                        0b00 => {
                            let data = self.read_instruction_word(system)?;
                            match optype {
                                0b0000 => Ok(Instruction::ORtoCCR(data as u8)),
                                0b0001 => Ok(Instruction::ANDtoCCR(data as u8)),
                                0b1010 => Ok(Instruction::EORtoCCR(data as u8)),
                                _ => return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION)),
                            }
                        },
                        0b01 => {
                            let data = self.read_instruction_word(system)?;
                            match optype {
                                0b0000 => Ok(Instruction::ORtoSR(data)),
                                0b0010 => Ok(Instruction::ANDtoSR(data)),
                                0b1010 => Ok(Instruction::EORtoSR(data)),
                                _ => return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION)),
                            }
                        },
                        _ => return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION)),
                    }
                } else if (ins & 0x138) == 0x108 {
                    let dreg = get_high_reg(ins);
                    let areg = get_low_reg(ins);
                    let dir = if (ins & 0x0800) == 0 { Direction::FromTarget } else { Direction::ToTarget };
                    let size = if (ins & 0x0040) == 0 { Size::Word } else { Size::Long };
                    let offset = sign_extend_to_long(self.read_instruction_word(system)? as u32, Size::Word);
                    Ok(Instruction::MOVEP(dreg, Target::IndirectARegOffset(areg, offset), size, dir))
                } else if (ins & 0x0100) == 0x0100 || (ins & 0x0F00) == 0x0800 {
                    let bitnum = if (ins & 0x0100) == 0x0100 {
                        Target::DirectDReg(get_high_reg(ins))
                    } else {
                        Target::Immediate(self.read_instruction_word(system)? as u32)
                    };

                    let target = self.decode_lower_effective_address(system, ins, Some(Size::Byte))?;
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
                    let data = match size {
                        Some(Size::Byte) => (self.read_instruction_word(system)? as u32 & 0xFF),
                        Some(Size::Word) => self.read_instruction_word(system)? as u32,
                        Some(Size::Long) => self.read_instruction_long(system)?,
                        None => return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION)),
                    };
                    let target = self.decode_lower_effective_address(system, ins, size)?;

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
                let src = self.decode_lower_effective_address(system, ins, Some(Size::Byte))?;
                let dest = self.decode_upper_effective_address(system, ins, Some(Size::Byte))?;
                Ok(Instruction::MOVE(src, dest, Size::Byte))
            },
            OPCG_MOVE_LONG => {
                let src = self.decode_lower_effective_address(system, ins, Some(Size::Long))?;
                let dest = self.decode_upper_effective_address(system, ins, Some(Size::Long))?;
                if let Target::DirectAReg(reg) = dest {
                    Ok(Instruction::MOVEA(src, reg, Size::Long))
                } else {
                    Ok(Instruction::MOVE(src, dest, Size::Long))
                }
            },
            OPCG_MOVE_WORD => {
                let src = self.decode_lower_effective_address(system, ins, Some(Size::Word))?;
                let dest = self.decode_upper_effective_address(system, ins, Some(Size::Word))?;
                if let Target::DirectAReg(reg) = dest {
                    Ok(Instruction::MOVEA(src, reg, Size::Word))
                } else {
                    Ok(Instruction::MOVE(src, dest, Size::Word))
                }
            },
            OPCG_MISC => {
                let ins_0f00 = ins & 0xF00;
                let ins_00f0 = ins & 0x0F0;

                if (ins & 0x180) == 0x180 {
                    if (ins & 0x040) == 0 {
                        let size = match get_size(ins) {
                            Some(Size::Word) => Size::Word,
                            Some(Size::Long) if self.cputype >= M68kType::MC68020 => Size::Long,
                            _ => return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION)),
                        };

                        let reg = get_high_reg(ins);
                        let target = self.decode_lower_effective_address(system, ins, Some(size))?;
                        Ok(Instruction::CHK(target, reg, size))
                    } else {
                        let src = self.decode_lower_effective_address(system, ins, None)?;
                        let dest = get_high_reg(ins);
                        Ok(Instruction::LEA(src, dest))
                    }
                } else if (ins & 0xB80) == 0x880 && (ins & 0x038) != 0 {
                    let mode = get_low_mode(ins);
                    let size = if (ins & 0x0040) == 0 { Size::Word } else { Size::Long };

                    let data = self.read_instruction_word(system)?;
                    let target = self.decode_lower_effective_address(system, ins, None)?;
                    let dir = if (ins & 0x0400) == 0 { Direction::ToTarget } else { Direction::FromTarget };
                    Ok(Instruction::MOVEM(target, size, dir, data))
                } else if (ins & 0x800) == 0 {
                    let target = self.decode_lower_effective_address(system, ins, Some(Size::Word))?;
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
                                None if self.cputype >= M68kType::MC68010 => Ok(Instruction::MOVEfromCCR(target)),
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
                } else if ins_0f00 == 0x800 {
                    let subselect = (ins & 0x01C0) >> 6;
                    let mode = get_low_mode(ins);
                    match (subselect, mode) {
                        (0b000, _) => {
                            let target = self.decode_lower_effective_address(system, ins, Some(Size::Byte))?;
                            Ok(Instruction::NBCD(target))
                        },
                        (0b001, 0b000) => {
                            Ok(Instruction::SWAP(get_low_reg(ins)))
                        },
                        (0b001, 0b001) => {
                            Ok(Instruction::BKPT(get_low_reg(ins)))
                        },
                        (0b001, _) => {
                            let target = self.decode_lower_effective_address(system, ins, None)?;
                            Ok(Instruction::PEA(target))
                        },
                        (0b010, 0b000) |
                        (0b011, 0b000) => {
                            let size = if (ins & 0x0040) == 0 { Size::Word } else { Size::Long };
                            Ok(Instruction::EXT(get_low_reg(ins), size))
                        },
                        _ => return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION)),
                    }
                } else if ins_0f00 == 0xA00 {
                    if (ins & 0x0FF) == 0xFC {
                        Ok(Instruction::ILLEGAL)
                    } else {
                        let target = self.decode_lower_effective_address(system, ins, Some(Size::Word))?;
                        match get_size(ins) {
                            Some(size) => Ok(Instruction::TST(target, size)),
                            None => Ok(Instruction::TAS(target)),
                        }
                    }
                } else if ins_0f00 == 0xE00 {
                    if (ins & 0x80) == 0x80 {
                        let target = self.decode_lower_effective_address(system, ins, None)?;
                        if (ins & 0b01000000) == 0 {
                            Ok(Instruction::JSR(target))
                        } else {
                            Ok(Instruction::JMP(target))
                        }
                    } else if ins_00f0 == 0x40 {
                        Ok(Instruction::TRAP((ins & 0x000F) as u8))
                    } else if ins_00f0 == 0x50 {
                        let reg = get_low_reg(ins);
                        if (ins & 0b1000) == 0 {
                            let data = self.read_instruction_word(system)?;
                            Ok(Instruction::LINK(reg, data as i16))
                        } else {
                            Ok(Instruction::UNLK(reg))
                        }
                    } else if ins_00f0 == 0x60 {
                        let reg = get_low_reg(ins);
                        let dir = if (ins & 0b1000) == 0 { Direction::FromTarget } else { Direction::ToTarget };
                        Ok(Instruction::MOVEUSP(Target::DirectAReg(reg), dir))
                    } else {
                        match ins & 0x00FF {
                            0x70 => Ok(Instruction::RESET),
                            0x71 => Ok(Instruction::NOP),
                            0x72 => {
                                let data = self.read_instruction_word(system)?;
                                Ok(Instruction::STOP(data))
                            },
                            0x73 => Ok(Instruction::RTE),
                            0x74 if self.cputype >= M68kType::MC68010 => {
                                let offset = self.read_instruction_word(system)? as i16;
                                Ok(Instruction::RTD(offset))
                            },
                            0x75 => Ok(Instruction::RTS),
                            0x76 => Ok(Instruction::TRAPV),
                            0x77 => Ok(Instruction::RTR),
                            0x7A | 0x7B if self.cputype >= M68kType::MC68010 => {
                                let dir = if ins & 0x01 == 0 { Direction::ToTarget } else { Direction::FromTarget };
                                let ins2 = self.read_instruction_word(system)?;
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
                } else {
                    return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION));
                }
            },
            OPCG_ADDQ_SUBQ => {
                match get_size(ins) {
                    Some(size) => {
                        let target = self.decode_lower_effective_address(system, ins, Some(size))?;
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
                    None => {
                        let mode = get_low_mode(ins);
                        let condition = get_condition(ins);

                        if mode == 0b001 {
                            let reg = get_low_reg(ins);
                            let disp = self.read_instruction_word(system)? as i16;
                            Ok(Instruction::DBcc(condition, reg, disp))
                        } else {
                            let target = self.decode_lower_effective_address(system, ins, Some(Size::Byte))?;
                            Ok(Instruction::Scc(condition, target))
                        }
                    },
                }
            },
            OPCG_BRANCH => {
                let mut disp = ((ins & 0xFF) as i8) as i32;
                if disp == 0 {
                    disp = (self.read_instruction_word(system)? as i16) as i32;
                } else if disp == 0xff && self.cputype >= M68kType::MC68020 {
                    disp = self.read_instruction_long(system)? as i32;
                }
                let condition = get_condition(ins);
                match condition {
                    Condition::True => Ok(Instruction::BRA(disp)),
                    Condition::False => Ok(Instruction::BSR(disp)),
                    _ => Ok(Instruction::Bcc(condition, disp)),
                }
            },
            OPCG_MOVEQ => {
                if (ins & 0x0100) != 0 {
                    return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION));
                }
                let reg = get_high_reg(ins);
                let data = (ins & 0xFF) as u8;
                Ok(Instruction::MOVEQ(data, reg))
            },
            OPCG_DIV_OR => {
                let size = get_size(ins);

                if size.is_none() {
                    let sign = if (ins & 0x0100) == 0 { Sign::Unsigned } else { Sign::Signed };
                    let data_reg = Target::DirectDReg(get_high_reg(ins));
                    let effective_addr = self.decode_lower_effective_address(system, ins, size)?;
                    Ok(Instruction::DIV(effective_addr, data_reg, Size::Word, sign))
                } else if (ins & 0b000111110000) == 0b000100000000 {
                    // TODO SBCD
                    panic!("Not Implemented");
                } else {
                    let data_reg = Target::DirectDReg(get_high_reg(ins));
                    let effective_addr = self.decode_lower_effective_address(system, ins, size)?;
                    let (from, to) = if (ins & 0x0100) == 0 { (effective_addr, data_reg) } else { (data_reg, effective_addr) };
                    Ok(Instruction::OR(from, to, size.unwrap()))
                }
            },
            OPCG_SUB => {
                let reg = get_high_reg(ins);
                let dir = (ins & 0x0100) >> 8;
                let size = get_size(ins);
                match size {
                    Some(size) => {
                        if (ins & 0b100110000) == 0b100000000 {
                            // TODO implement SUBX
                            panic!("Not Implemented");
                        } else {
                            let target = self.decode_lower_effective_address(system, ins, Some(size))?;
                            if dir == 0 {
                                Ok(Instruction::SUB(target, Target::DirectDReg(reg), size))
                            } else {
                                Ok(Instruction::SUB(Target::DirectDReg(reg), target, size))
                            }
                        }
                    },
                    None => {
                        let size = if dir == 0 { Size::Word } else { Size::Long };
                        let target = self.decode_lower_effective_address(system, ins, Some(size))?;
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
                        if get_low_mode(ins) == 0b001 {
                            Ok(Instruction::CMP(Target::IndirectARegInc(get_low_reg(ins)), Target::IndirectARegInc(reg), size))
                        } else {
                            let target = self.decode_lower_effective_address(system, ins, Some(size))?;
                            Ok(Instruction::EOR(Target::DirectDReg(reg), target, size))
                        }
                    },
                    (0b0, Some(size)) => {
                        let target = self.decode_lower_effective_address(system, ins, Some(size))?;
                        Ok(Instruction::CMP(target, Target::DirectDReg(reg), size))
                    },
                    (_, None) => {
                        let size = if optype == 0 { Size::Word } else { Size::Long };
                        let target = self.decode_lower_effective_address(system, ins, Some(size))?;
                        Ok(Instruction::CMPA(target, reg, size))
                    },
                    _ => return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION)),
                }
            },
            OPCG_MUL_AND => {
                let size = get_size(ins);

                if (ins & 0b000111110000) == 0b000100000000 {
                    // TODO ABCD
                    panic!("Not Implemented");
                } else if (ins & 0b000100110000) == 0b000100000000 {
                    let regx = get_high_reg(ins);
                    let regy = get_low_reg(ins);
                    match (ins & 0x00F8) >> 3 {
                        0b01000 => Ok(Instruction::EXG(Target::DirectDReg(regx), Target::DirectDReg(regy))),
                        0b01001 => Ok(Instruction::EXG(Target::DirectAReg(regx), Target::DirectAReg(regy))),
                        0b10001 => Ok(Instruction::EXG(Target::DirectDReg(regx), Target::DirectAReg(regy))),
                        _ => return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION)),
                    }
                } else if size.is_none() {
                    let sign = if (ins & 0x0100) == 0 { Sign::Unsigned } else { Sign::Signed };
                    let data_reg = Target::DirectDReg(get_high_reg(ins));
                    let effective_addr = self.decode_lower_effective_address(system, ins, Some(Size::Word))?;
                    Ok(Instruction::MUL(effective_addr, data_reg, Size::Word, sign))
                } else {
                    let data_reg = Target::DirectDReg(get_high_reg(ins));
                    let effective_addr = self.decode_lower_effective_address(system, ins, size)?;
                    let (from, to) = if (ins & 0x0100) == 0 { (effective_addr, data_reg) } else { (data_reg, effective_addr) };
                    Ok(Instruction::AND(from, to, size.unwrap()))
                }
            },
            OPCG_ADD => {
                let reg = get_high_reg(ins);
                let dir = (ins & 0x0100) >> 8;
                let size = get_size(ins);
                match size {
                    Some(size) => {
                        if (ins & 0b100110000) == 0b100000000 {
                            // TODO implement ADDX
                            panic!("Not Implemented");
                        } else {
                            let target = self.decode_lower_effective_address(system, ins, Some(size))?;
                            if dir == 0 {
                                Ok(Instruction::ADD(target, Target::DirectDReg(reg), size))
                            } else {
                                Ok(Instruction::ADD(Target::DirectDReg(reg), target, size))
                            }
                        }
                    },
                    None => {
                        let size = if dir == 0 { Size::Word } else { Size::Long };
                        let target = self.decode_lower_effective_address(system, ins, Some(size))?;
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
                            Target::Immediate(if rotation != 0 { rotation as u32 } else { 8 })
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
                        let target = self.decode_lower_effective_address(system, ins, Some(Size::Word))?;
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

    fn read_instruction_word(&mut self, device: &mut AddressableDeviceRefMut<'_>) -> Result<u16, Error> {
        let word = device.read_beu16((self.end - self.base) as Address)?;
        //debug!("{:#010x} {:#06x?}", self.end, word);
        self.end += 2;
        Ok(word)
    }

    fn read_instruction_long(&mut self, device: &mut AddressableDeviceRefMut<'_>) -> Result<u32, Error> {
        let word = device.read_beu32((self.end - self.base) as Address)?;
        //debug!("{:#010x} {:#010x}", self.end, word);
        self.end += 4;
        Ok(word)
    }

    fn decode_lower_effective_address(&mut self, system: &mut AddressableDeviceRefMut<'_>, ins: u16, size: Option<Size>) -> Result<Target, Error> {
        let reg = get_low_reg(ins);
        let mode = get_low_mode(ins);
        self.get_mode_as_target(system, mode, reg, size)
    }

    fn decode_upper_effective_address(&mut self, system: &mut AddressableDeviceRefMut<'_>, ins: u16, size: Option<Size>) -> Result<Target, Error> {
        let reg = get_high_reg(ins);
        let mode = get_high_mode(ins);
        self.get_mode_as_target(system, mode, reg, size)
    }

    fn decode_extension_word(&mut self, system: &mut AddressableDeviceRefMut<'_>, areg: Option<u8>) -> Result<Target, Error> {
        let brief_extension = self.read_instruction_word(system)?;
        let rtype = if (brief_extension & 0x8000) == 0 { RegisterType::Data } else { RegisterType::Address };
        let xreg = ((brief_extension & 0x7000) >> 12) as u8;
        let size = if (brief_extension & 0x0800) == 0 { Size::Word } else { Size::Long };
        let use_full = (brief_extension & 0x0100) == 1;
        let scale = (brief_extension & 0x0600) >> 9 as u8;

        if !use_full {
            let displacement = sign_extend_to_long((brief_extension & 0x00FF) as u32, Size::Byte);
            match areg {
                Some(areg) => Ok(Target::IndirectARegXRegOffset(areg, rtype, xreg, displacement, size)),
                None => Ok(Target::IndirectPCXRegOffset(rtype, xreg, displacement, size)),
            }
        } else if self.cputype >= M68kType::MC68020 {
            let use_base = (brief_extension & 0x0080) == 0;
            let use_index = (brief_extension & 0x0040) == 0;

            panic!("Not Implemented");
        } else {
            Err(Error::processor(ERR_ILLEGAL_INSTRUCTION))
        }
    }

    pub fn get_mode_as_target(&mut self, system: &mut AddressableDeviceRefMut<'_>, mode: u8, reg: u8, size: Option<Size>) -> Result<Target, Error> {
        let value = match mode {
            0b000 => Target::DirectDReg(reg),
            0b001 => Target::DirectAReg(reg),
            0b010 => Target::IndirectAReg(reg),
            0b011 => Target::IndirectARegInc(reg),
            0b100 => Target::IndirectARegDec(reg),
            0b101 => {
                let data = sign_extend_to_long(self.read_instruction_word(system)? as u32, Size::Word);
                Target::IndirectARegOffset(reg, data)
            },
            0b110 => {
                self.decode_extension_word(system, Some(reg))?
            },
            0b111 => {
                match reg {
                    0b000 => {
                        let value = sign_extend_to_long(self.read_instruction_word(system)? as u32, Size::Word) as u32;
                        Target::IndirectMemory(value)
                    },
                    0b001 => {
                        let value = self.read_instruction_long(system)?;
                        Target::IndirectMemory(value)
                    },
                    0b010 => {
                        let data = sign_extend_to_long(self.read_instruction_word(system)? as u32, Size::Word);
                        Target::IndirectPCOffset(data)
                    },
                    0b011 => {
                        self.decode_extension_word(system, None)?
                    },
                    0b100 => {
                        let data = match size {
                            Some(Size::Byte) | Some(Size::Word) => self.read_instruction_word(system)? as u32,
                            Some(Size::Long) => self.read_instruction_long(system)?,
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

pub fn sign_extend_to_long(value: u32, from: Size) -> i32 {
    match from {
        Size::Byte => ((value as u8) as i8) as i32,
        Size::Word => ((value as u16) as i16) as i32,
        Size::Long => value as i32,
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Target::Immediate(value) => write!(f, "#{:08x}", value),
            Target::DirectDReg(reg) => write!(f, "%d{}", reg),
            Target::DirectAReg(reg) => write!(f, "%a{}", reg),
            Target::IndirectAReg(reg) => write!(f, "(%a{})", reg),
            Target::IndirectARegInc(reg) => write!(f, "(%a{})+", reg),
            Target::IndirectARegDec(reg) => write!(f, "-(%a{})", reg),
            Target::IndirectARegOffset(reg, offset) => write!(f, "(%a{} + #{})", reg, offset),
            Target::IndirectARegXRegOffset(reg, rtype, xreg, offset, _) => write!(f, "(%a{} + %{}{} + #{})", reg, if *rtype == RegisterType::Data { 'd' } else { 'a' }, xreg, offset),
            Target::IndirectMemory(value) => write!(f, "(#{:08x})", value),
            Target::IndirectPCOffset(offset) => write!(f, "(%pc + #{})", offset),
            Target::IndirectPCXRegOffset(rtype, xreg, offset, _) => write!(f, "(%pc + %{}{} + #{})", if *rtype == RegisterType::Data { 'd' } else { 'a' }, xreg, offset),
        }
    }
}

