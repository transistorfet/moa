
use std::fmt;

use crate::error::Error;
use crate::system::System;
use crate::devices::{Address, Addressable};

use super::state::{M68kType, Exceptions};


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
const OPCG_CMP_EOR: u8 = 0xB;
const OPCG_MUL_AND: u8 = 0xC;
const OPCG_ADD: u8 = 0xD;
const OPCG_SHIFT: u8 = 0xE;

#[allow(dead_code)]
const OPCG_RESERVED1: u8 = 0xA;
#[allow(dead_code)]
const OPCG_RESERVED2: u8 = 0xF;


pub type Register = u8;

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
pub enum XRegister {
    Data(u8),
    Address(u8),
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum RegOrImmediate {
    DReg(u8),
    Immediate(u8),
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
    DirectDReg(Register),
    DirectAReg(Register),
    IndirectAReg(Register),
    IndirectARegInc(Register),
    IndirectARegDec(Register),
    IndirectARegOffset(Register, i32),
    IndirectARegXRegOffset(Register, XRegister, i32, u8, Size),
    IndirectMemory(u32),
    IndirectPCOffset(i32),
    IndirectPCXRegOffset(XRegister, i32, u8, Size),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Instruction {
    ABCD(Target, Target),
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
    BFCHG(Target, RegOrImmediate, RegOrImmediate),
    BFCLR(Target, RegOrImmediate, RegOrImmediate),
    BFEXTS(Target, RegOrImmediate, RegOrImmediate, Register),
    BFEXTU(Target, RegOrImmediate, RegOrImmediate, Register),
    BFFFO(Target, RegOrImmediate, RegOrImmediate, Register),
    BFINS(Register, Target, RegOrImmediate, RegOrImmediate),
    BFSET(Target, RegOrImmediate, RegOrImmediate),
    BFTST(Target, RegOrImmediate, RegOrImmediate),
    BKPT(u8),

    CHK(Target, Register, Size),
    CLR(Target, Size),
    CMP(Target, Target, Size),
    CMPA(Target, Register, Size),

    DBcc(Condition, Register, i16),
    DIV(Target, Target, Size, Sign),

    EOR(Target, Target, Size),
    EORtoCCR(u8),
    EORtoSR(u16),
    EXG(Target, Target),
    EXT(Register, Size, Size),

    ILLEGAL,

    JMP(Target),
    JSR(Target),

    LEA(Target, Register),
    LINK(Register, i16),
    LSd(Target, Target, Size, ShiftDirection),

    MOVE(Target, Target, Size),
    MOVEA(Target, Register, Size),
    MOVEfromSR(Target),
    MOVEtoSR(Target),
    MOVEfromCCR(Target),
    MOVEtoCCR(Target),
    MOVEC(Target, ControlRegister, Direction),
    MOVEM(Target, Size, Direction, u16),
    MOVEP(Register, Target, Size, Direction),
    MOVEQ(u8, Register),
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

    SBCD(Target, Target),
    Scc(Condition, Target),
    STOP(u16),
    SUB(Target, Target, Size),
    SWAP(Register),

    TAS(Target),
    TST(Target, Size),
    TRAP(u8),
    TRAPV,

    UNLK(Register),
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
        self.instruction = self.decode_one(memory.borrow_mut().as_addressable().unwrap())?;
        Ok(())
    }

    pub fn decode_one(&mut self, memory: &mut dyn Addressable) -> Result<Instruction, Error> {
        let ins = self.read_instruction_word(memory)?;

        match ((ins & 0xF000) >> 12) as u8 {
            OPCG_BIT_OPS => {
                let optype = (ins & 0x0F00) >> 8;

                if (ins & 0x3F) == 0b111100 {
                    match (ins & 0x00C0) >> 6 {
                        0b00 => {
                            let data = self.read_instruction_word(memory)?;
                            match optype {
                                0b0000 => Ok(Instruction::ORtoCCR(data as u8)),
                                0b0001 => Ok(Instruction::ANDtoCCR(data as u8)),
                                0b1010 => Ok(Instruction::EORtoCCR(data as u8)),
                                _ => return Err(Error::processor(Exceptions::IllegalInstruction as u32)),
                            }
                        },
                        0b01 => {
                            let data = self.read_instruction_word(memory)?;
                            match optype {
                                0b0000 => Ok(Instruction::ORtoSR(data)),
                                0b0010 => Ok(Instruction::ANDtoSR(data)),
                                0b1010 => Ok(Instruction::EORtoSR(data)),
                                _ => return Err(Error::processor(Exceptions::IllegalInstruction as u32)),
                            }
                        },
                        _ => return Err(Error::processor(Exceptions::IllegalInstruction as u32)),
                    }
                } else if (ins & 0x138) == 0x108 {
                    let dreg = get_high_reg(ins);
                    let areg = get_low_reg(ins);
                    let dir = if (ins & 0x0800) == 0 { Direction::FromTarget } else { Direction::ToTarget };
                    let size = if (ins & 0x0040) == 0 { Size::Word } else { Size::Long };
                    let offset = sign_extend_to_long(self.read_instruction_word(memory)? as u32, Size::Word);
                    Ok(Instruction::MOVEP(dreg, Target::IndirectARegOffset(areg, offset), size, dir))
                } else if (ins & 0x0100) == 0x0100 || (ins & 0x0F00) == 0x0800 {
                    let bitnum = if (ins & 0x0100) == 0x0100 {
                        Target::DirectDReg(get_high_reg(ins))
                    } else {
                        Target::Immediate(self.read_instruction_word(memory)? as u32)
                    };

                    let target = self.decode_lower_effective_address(memory, ins, Some(Size::Byte))?;
                    let size = match target {
                        Target::DirectAReg(_) | Target::DirectDReg(_) => Size::Long,
                        _ => Size::Byte,
                    };

                    match (ins & 0x00C0) >> 6 {
                        0b00 => Ok(Instruction::BTST(bitnum, target, size)),
                        0b01 => Ok(Instruction::BCHG(bitnum, target, size)),
                        0b10 => Ok(Instruction::BCLR(bitnum, target, size)),
                        0b11 => Ok(Instruction::BSET(bitnum, target, size)),
                        _ => return Err(Error::processor(Exceptions::IllegalInstruction as u32)),
                    }
                } else {
                    let size = get_size(ins);
                    let data = match size {
                        Some(Size::Byte) => (self.read_instruction_word(memory)? as u32 & 0xFF),
                        Some(Size::Word) => self.read_instruction_word(memory)? as u32,
                        Some(Size::Long) => self.read_instruction_long(memory)?,
                        None => return Err(Error::processor(Exceptions::IllegalInstruction as u32)),
                    };
                    let target = self.decode_lower_effective_address(memory, ins, size)?;

                    match optype {
                        0b0000 => Ok(Instruction::OR(Target::Immediate(data), target, size.unwrap())),
                        0b0010 => Ok(Instruction::AND(Target::Immediate(data), target, size.unwrap())),
                        0b0100 => Ok(Instruction::SUB(Target::Immediate(data), target, size.unwrap())),
                        0b0110 => Ok(Instruction::ADD(Target::Immediate(data), target, size.unwrap())),
                        0b1010 => Ok(Instruction::EOR(Target::Immediate(data), target, size.unwrap())),
                        0b1100 => Ok(Instruction::CMP(Target::Immediate(data), target, size.unwrap())),
                        _ => return Err(Error::processor(Exceptions::IllegalInstruction as u32)),
                    }
                }
            },
            OPCG_MOVE_BYTE => {
                let src = self.decode_lower_effective_address(memory, ins, Some(Size::Byte))?;
                let dest = self.decode_upper_effective_address(memory, ins, Some(Size::Byte))?;
                Ok(Instruction::MOVE(src, dest, Size::Byte))
            },
            OPCG_MOVE_LONG => {
                let src = self.decode_lower_effective_address(memory, ins, Some(Size::Long))?;
                let dest = self.decode_upper_effective_address(memory, ins, Some(Size::Long))?;
                if let Target::DirectAReg(reg) = dest {
                    Ok(Instruction::MOVEA(src, reg, Size::Long))
                } else {
                    Ok(Instruction::MOVE(src, dest, Size::Long))
                }
            },
            OPCG_MOVE_WORD => {
                let src = self.decode_lower_effective_address(memory, ins, Some(Size::Word))?;
                let dest = self.decode_upper_effective_address(memory, ins, Some(Size::Word))?;
                if let Target::DirectAReg(reg) = dest {
                    Ok(Instruction::MOVEA(src, reg, Size::Word))
                } else {
                    Ok(Instruction::MOVE(src, dest, Size::Word))
                }
            },
            OPCG_MISC => {
                let ins_0f00 = ins & 0xF00;
                let ins_00f0 = ins & 0x0F0;

                if (ins & 0x180) == 0x180 && (ins & 0x038) != 0 {
                    if (ins & 0x040) == 0 {
                        let size = match get_size(ins) {
                            Some(Size::Word) => Size::Word,
                            Some(Size::Long) if self.cputype >= M68kType::MC68020 => Size::Long,
                            _ => return Err(Error::processor(Exceptions::IllegalInstruction as u32)),
                        };

                        let reg = get_high_reg(ins);
                        let target = self.decode_lower_effective_address(memory, ins, Some(size))?;
                        Ok(Instruction::CHK(target, reg, size))
                    } else {
                        let src = self.decode_lower_effective_address(memory, ins, None)?;
                        let dest = get_high_reg(ins);
                        Ok(Instruction::LEA(src, dest))
                    }
                } else if (ins & 0xB80) == 0x880 && (ins & 0x038) != 0 {
                    let mode = get_low_mode(ins);
                    let size = if (ins & 0x0040) == 0 { Size::Word } else { Size::Long };

                    let data = self.read_instruction_word(memory)?;
                    let target = self.decode_lower_effective_address(memory, ins, None)?;
                    let dir = if (ins & 0x0400) == 0 { Direction::ToTarget } else { Direction::FromTarget };
                    Ok(Instruction::MOVEM(target, size, dir, data))
                } else if (ins & 0x800) == 0 {
                    let target = self.decode_lower_effective_address(memory, ins, Some(Size::Word))?;
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
                                None => return Err(Error::processor(Exceptions::IllegalInstruction as u32)),
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
                        _ => return Err(Error::processor(Exceptions::IllegalInstruction as u32)),
                    }
                } else if ins_0f00 == 0x800 || ins_0f00 == 0x900 {
                    let subselect = (ins & 0x01C0) >> 6;
                    let mode = get_low_mode(ins);
                    match (subselect, mode) {
                        (0b000, _) => {
                            let target = self.decode_lower_effective_address(memory, ins, Some(Size::Byte))?;
                            Ok(Instruction::NBCD(target))
                        },
                        (0b001, 0b000) => {
                            Ok(Instruction::SWAP(get_low_reg(ins)))
                        },
                        (0b001, 0b001) => {
                            Ok(Instruction::BKPT(get_low_reg(ins)))
                        },
                        (0b001, _) => {
                            let target = self.decode_lower_effective_address(memory, ins, None)?;
                            Ok(Instruction::PEA(target))
                        },
                        (0b010, 0b000) => {
                            Ok(Instruction::EXT(get_low_reg(ins), Size::Byte, Size::Word))
                        },
                        (0b011, 0b000) => {
                            Ok(Instruction::EXT(get_low_reg(ins), Size::Word, Size::Long))
                        },
                        (0b111, 0b000) => {
                            Ok(Instruction::EXT(get_low_reg(ins), Size::Byte, Size::Long))
                        },
                        _ => return Err(Error::processor(Exceptions::IllegalInstruction as u32)),
                    }
                } else if ins_0f00 == 0xA00 {
                    if (ins & 0x0FF) == 0xFC {
                        Ok(Instruction::ILLEGAL)
                    } else {
                        let target = self.decode_lower_effective_address(memory, ins, Some(Size::Word))?;
                        match get_size(ins) {
                            Some(size) => Ok(Instruction::TST(target, size)),
                            None => Ok(Instruction::TAS(target)),
                        }
                    }
                } else if ins_0f00 == 0xE00 {
                    if (ins & 0x80) == 0x80 {
                        let target = self.decode_lower_effective_address(memory, ins, None)?;
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
                            let data = self.read_instruction_word(memory)?;
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
                                let data = self.read_instruction_word(memory)?;
                                Ok(Instruction::STOP(data))
                            },
                            0x73 => Ok(Instruction::RTE),
                            0x74 if self.cputype >= M68kType::MC68010 => {
                                let offset = self.read_instruction_word(memory)? as i16;
                                Ok(Instruction::RTD(offset))
                            },
                            0x75 => Ok(Instruction::RTS),
                            0x76 => Ok(Instruction::TRAPV),
                            0x77 => Ok(Instruction::RTR),
                            0x7A | 0x7B if self.cputype >= M68kType::MC68010 => {
                                let dir = if ins & 0x01 == 0 { Direction::ToTarget } else { Direction::FromTarget };
                                let ins2 = self.read_instruction_word(memory)?;
                                let target = match ins2 & 0x8000 {
                                    0 => Target::DirectDReg(((ins2 & 0x7000) >> 12) as u8),
                                    _ => Target::DirectAReg(((ins2 & 0x7000) >> 12) as u8),
                                };
                                let creg = match ins2 & 0xFFF {
                                    0x801 => ControlRegister::VBR,
                                    _ => return Err(Error::processor(Exceptions::IllegalInstruction as u32)),
                                };
                                Ok(Instruction::MOVEC(target, creg, dir))
                            },
                            _ => return Err(Error::processor(Exceptions::IllegalInstruction as u32)),
                        }
                    }
                } else {
                    return Err(Error::processor(Exceptions::IllegalInstruction as u32));
                }
            },
            OPCG_ADDQ_SUBQ => {
                match get_size(ins) {
                    Some(size) => {
                        let target = self.decode_lower_effective_address(memory, ins, Some(size))?;
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
                            let disp = self.read_instruction_word(memory)? as i16;
                            Ok(Instruction::DBcc(condition, reg, disp))
                        } else {
                            let target = self.decode_lower_effective_address(memory, ins, Some(Size::Byte))?;
                            Ok(Instruction::Scc(condition, target))
                        }
                    },
                }
            },
            OPCG_BRANCH => {
                let mut disp = ((ins & 0xFF) as i8) as i32;
                if disp == 0 {
                    disp = (self.read_instruction_word(memory)? as i16) as i32;
                } else if disp == -1 && self.cputype >= M68kType::MC68020 {
                    disp = self.read_instruction_long(memory)? as i32;
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
                    return Err(Error::processor(Exceptions::IllegalInstruction as u32));
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
                    let effective_addr = self.decode_lower_effective_address(memory, ins, size)?;
                    Ok(Instruction::DIV(effective_addr, data_reg, Size::Word, sign))
                } else if (ins & 0x1F0) == 0x100 {
                    let regx = get_high_reg(ins);
                    let regy = get_low_reg(ins);

                    match (ins & 0x08) == 0 {
                        false => Ok(Instruction::SBCD(Target::DirectDReg(regy), Target::DirectDReg(regx))),
                        true => Ok(Instruction::SBCD(Target::IndirectARegDec(regy), Target::IndirectARegDec(regx))),
                    }
                } else {
                    let data_reg = Target::DirectDReg(get_high_reg(ins));
                    let effective_addr = self.decode_lower_effective_address(memory, ins, size)?;
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
                            let mode = (ins & 0x08) == 0;

                            // TODO implement SUBX
                            panic!("Not Implemented");
                        } else {
                            let target = self.decode_lower_effective_address(memory, ins, Some(size))?;
                            if dir == 0 {
                                Ok(Instruction::SUB(target, Target::DirectDReg(reg), size))
                            } else {
                                Ok(Instruction::SUB(Target::DirectDReg(reg), target, size))
                            }
                        }
                    },
                    None => {
                        let size = if dir == 0 { Size::Word } else { Size::Long };
                        let target = self.decode_lower_effective_address(memory, ins, Some(size))?;
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
                            let target = self.decode_lower_effective_address(memory, ins, Some(size))?;
                            Ok(Instruction::EOR(Target::DirectDReg(reg), target, size))
                        }
                    },
                    (0b0, Some(size)) => {
                        let target = self.decode_lower_effective_address(memory, ins, Some(size))?;
                        Ok(Instruction::CMP(target, Target::DirectDReg(reg), size))
                    },
                    (_, None) => {
                        let size = if optype == 0 { Size::Word } else { Size::Long };
                        let target = self.decode_lower_effective_address(memory, ins, Some(size))?;
                        Ok(Instruction::CMPA(target, reg, size))
                    },
                    _ => return Err(Error::processor(Exceptions::IllegalInstruction as u32)),
                }
            },
            OPCG_MUL_AND => {
                let size = get_size(ins);

                if (ins & 0b000111110000) == 0b000100000000 {
                    let regx = get_high_reg(ins);
                    let regy = get_low_reg(ins);

                    match (ins & 0x08) == 0 {
                        false => Ok(Instruction::ABCD(Target::DirectDReg(regy), Target::DirectDReg(regx))),
                        true => Ok(Instruction::ABCD(Target::IndirectARegDec(regy), Target::IndirectARegDec(regx))),
                    }
                } else if (ins & 0b000100110000) == 0b000100000000 {
                    let regx = get_high_reg(ins);
                    let regy = get_low_reg(ins);
                    match (ins & 0x00F8) >> 3 {
                        0b01000 => Ok(Instruction::EXG(Target::DirectDReg(regx), Target::DirectDReg(regy))),
                        0b01001 => Ok(Instruction::EXG(Target::DirectAReg(regx), Target::DirectAReg(regy))),
                        0b10001 => Ok(Instruction::EXG(Target::DirectDReg(regx), Target::DirectAReg(regy))),
                        _ => return Err(Error::processor(Exceptions::IllegalInstruction as u32)),
                    }
                } else if size.is_none() {
                    let sign = if (ins & 0x0100) == 0 { Sign::Unsigned } else { Sign::Signed };
                    let data_reg = Target::DirectDReg(get_high_reg(ins));
                    let effective_addr = self.decode_lower_effective_address(memory, ins, Some(Size::Word))?;
                    Ok(Instruction::MUL(effective_addr, data_reg, Size::Word, sign))
                } else {
                    let data_reg = Target::DirectDReg(get_high_reg(ins));
                    let effective_addr = self.decode_lower_effective_address(memory, ins, size)?;
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
                            let mode = (ins & 0x08) == 0;

                            // TODO implement ADDX
                            panic!("Not Implemented");
                        } else {
                            let target = self.decode_lower_effective_address(memory, ins, Some(size))?;
                            if dir == 0 {
                                Ok(Instruction::ADD(target, Target::DirectDReg(reg), size))
                            } else {
                                Ok(Instruction::ADD(Target::DirectDReg(reg), target, size))
                            }
                        }
                    },
                    None => {
                        let size = if dir == 0 { Size::Word } else { Size::Long };
                        let target = self.decode_lower_effective_address(memory, ins, Some(size))?;
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
                            _ => return Err(Error::processor(Exceptions::IllegalInstruction as u32)),
                        }
                    },
                    None => {
                        let target = self.decode_lower_effective_address(memory, ins, Some(Size::Word))?;

                        let count = Target::Immediate(1);
                        if (ins & 0x800) == 0 {
                            match (ins & 0x0600) >> 9 {
                                0b00 => Ok(Instruction::ASd(count, target, Size::Word, dir)),
                                0b01 => Ok(Instruction::LSd(count, target, Size::Word, dir)),
                                0b10 => Ok(Instruction::ROXd(count, target, Size::Word, dir)),
                                0b11 => Ok(Instruction::ROd(count, target, Size::Word, dir)),
                                _ => return Err(Error::processor(Exceptions::IllegalInstruction as u32)),
                            }
                        } else if self.cputype > M68kType::MC68020 {
                            // Bitfield instructions (MC68020+)
                            let ext = self.read_instruction_word(memory)?;
                            let reg = ((ext & 0x7000) >> 12) as u8;

                            let offset = match (ext & 0x0800) == 0 {
                                true => RegOrImmediate::Immediate(((ext & 0x07C0) >> 6) as u8),
                                false => RegOrImmediate::DReg(((ext & 0x01C0) >> 6) as u8),
                            };

                            let width = match (ext & 0x0020) == 0 {
                                true => RegOrImmediate::Immediate((ext & 0x001F) as u8),
                                false => RegOrImmediate::DReg((ext & 0x0007) as u8),
                            };

                            match (ins & 0x0700) >> 8 {
                                0b010 => Ok(Instruction::BFCHG(target, offset, width)),
                                0b100 => Ok(Instruction::BFCLR(target, offset, width)),
                                0b011 => Ok(Instruction::BFEXTS(target, offset, width, reg)),
                                0b001 => Ok(Instruction::BFEXTU(target, offset, width, reg)),
                                0b101 => Ok(Instruction::BFFFO(target, offset, width, reg)),
                                0b111 => Ok(Instruction::BFINS(reg, target, offset, width)),
                                0b110 => Ok(Instruction::BFSET(target, offset, width)),
                                0b000 => Ok(Instruction::BFTST(target, offset, width)),
                                _ => return Err(Error::processor(Exceptions::IllegalInstruction as u32)),
                            }
                        } else {
                            return Err(Error::processor(Exceptions::IllegalInstruction as u32));
                        }
                    },
                }
            },
            _ => return Err(Error::processor(Exceptions::IllegalInstruction as u32)),
        }
    }

    fn read_instruction_word(&mut self, device: &mut dyn Addressable) -> Result<u16, Error> {
        let word = device.read_beu16((self.end - self.base) as Address)?;
        self.end += 2;
        Ok(word)
    }

    fn read_instruction_long(&mut self, device: &mut dyn Addressable) -> Result<u32, Error> {
        let word = device.read_beu32((self.end - self.base) as Address)?;
        self.end += 4;
        Ok(word)
    }

    fn decode_lower_effective_address(&mut self, memory: &mut dyn Addressable, ins: u16, size: Option<Size>) -> Result<Target, Error> {
        let reg = get_low_reg(ins);
        let mode = get_low_mode(ins);
        self.get_mode_as_target(memory, mode, reg, size)
    }

    fn decode_upper_effective_address(&mut self, memory: &mut dyn Addressable, ins: u16, size: Option<Size>) -> Result<Target, Error> {
        let reg = get_high_reg(ins);
        let mode = get_high_mode(ins);
        self.get_mode_as_target(memory, mode, reg, size)
    }

    fn decode_extension_word(&mut self, memory: &mut dyn Addressable, areg: Option<u8>) -> Result<Target, Error> {
        let brief_extension = self.read_instruction_word(memory)?;
        let xreg_num = ((brief_extension & 0x7000) >> 12) as u8;
        let xreg = if (brief_extension & 0x8000) == 0 { XRegister::Data(xreg_num) } else { XRegister::Address(xreg_num) };
        let size = if (brief_extension & 0x0800) == 0 { Size::Word } else { Size::Long };
        let scale = ((brief_extension & 0x0600) >> 9) as u8;
        let use_full = (brief_extension & 0x0100) != 0;

        if !use_full {
            let displacement = sign_extend_to_long((brief_extension & 0x00FF) as u32, Size::Byte);
            match areg {
                Some(areg) => Ok(Target::IndirectARegXRegOffset(areg, xreg, displacement, scale, size)),
                None => Ok(Target::IndirectPCXRegOffset(xreg, displacement, scale, size)),
            }
        } else if self.cputype >= M68kType::MC68020 {
            let use_base = (brief_extension & 0x0080) == 0;
            let use_index = (brief_extension & 0x0040) == 0;

            panic!("Not Implemented");
        } else {
            Err(Error::processor(Exceptions::IllegalInstruction as u32))
        }
    }

    pub fn get_mode_as_target(&mut self, memory: &mut dyn Addressable, mode: u8, reg: u8, size: Option<Size>) -> Result<Target, Error> {
        let value = match mode {
            0b000 => Target::DirectDReg(reg),
            0b001 => Target::DirectAReg(reg),
            0b010 => Target::IndirectAReg(reg),
            0b011 => Target::IndirectARegInc(reg),
            0b100 => Target::IndirectARegDec(reg),
            0b101 => {
                let data = sign_extend_to_long(self.read_instruction_word(memory)? as u32, Size::Word);
                Target::IndirectARegOffset(reg, data)
            },
            0b110 => {
                self.decode_extension_word(memory, Some(reg))?
            },
            0b111 => {
                match reg {
                    0b000 => {
                        let value = sign_extend_to_long(self.read_instruction_word(memory)? as u32, Size::Word) as u32;
                        Target::IndirectMemory(value)
                    },
                    0b001 => {
                        let value = self.read_instruction_long(memory)?;
                        Target::IndirectMemory(value)
                    },
                    0b010 => {
                        let data = sign_extend_to_long(self.read_instruction_word(memory)? as u32, Size::Word);
                        Target::IndirectPCOffset(data)
                    },
                    0b011 => {
                        self.decode_extension_word(memory, None)?
                    },
                    0b100 => {
                        let data = match size {
                            Some(Size::Byte) | Some(Size::Word) => self.read_instruction_word(memory)? as u32,
                            Some(Size::Long) => self.read_instruction_long(memory)?,
                            None => return Err(Error::processor(Exceptions::IllegalInstruction as u32)),
                        };
                        Target::Immediate(data)
                    },
                    _ => return Err(Error::processor(Exceptions::IllegalInstruction as u32)),
                }
            },
            _ => return Err(Error::processor(Exceptions::IllegalInstruction as u32)),
        };
        Ok(value)
    }

    pub fn dump_disassembly(&mut self, system: &System, start: u32, length: u32) {
        let mut next = start;
        while next < (start + length) {
            match self.decode_at(&system, next) {
                Ok(()) => {
                    self.dump_decoded(system);
                    next = self.end;
                },
                Err(err) => {
                    println!("{:?}", err);
                    match err {
                        Error { native, .. } if native == Exceptions::IllegalInstruction as u32 => {
                            println!("    at {:08x}: {:04x}", self.start, system.get_bus().read_beu16(self.start as Address).unwrap());
                        },
                        _ => { },
                    }
                    return;
                },
            }
        }
    }

    pub fn dump_decoded(&mut self, system: &System) {
        let ins_data: Result<String, Error> =
            (0..((self.end - self.start) / 2)).map(|offset|
                Ok(format!("{:04x} ", system.get_bus().read_beu16((self.start + (offset * 2)) as Address).unwrap()))
            ).collect();
        println!("{:#010x}: {}\n\t{:?}\n", self.start, ins_data.unwrap(), self.instruction);
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


impl fmt::Display for Sign {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Sign::Signed => write!(f, "s"),
            Sign::Unsigned => write!(f, "u"),
        }
    }
}

impl fmt::Display for ShiftDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShiftDirection::Right => write!(f, "r"),
            ShiftDirection::Left => write!(f, "l"),
        }
    }
}

impl fmt::Display for Size {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Size::Byte => write!(f, "b"),
            Size::Word => write!(f, "w"),
            Size::Long => write!(f, "l"),
        }
    }
}

impl fmt::Display for Condition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Condition::True => write!(f, "t"),
            Condition::False => write!(f, "f"),
            Condition::High => write!(f, "hi"),
            Condition::LowOrSame => write!(f, "ls"),
            Condition::CarryClear => write!(f, "cc"),
            Condition::CarrySet => write!(f, "cs"),
            Condition::NotEqual => write!(f, "ne"),
            Condition::Equal => write!(f, "eq"),
            Condition::OverflowClear => write!(f, "oc"),
            Condition::OverflowSet => write!(f, "os"),
            Condition::Plus => write!(f, "p"),
            Condition::Minus => write!(f, "m"),
            Condition::GreaterThanOrEqual => write!(f, "ge"),
            Condition::LessThan => write!(f, "lt"),
            Condition::GreaterThan => write!(f, "gt"),
            Condition::LessThanOrEqual => write!(f, "le"),
        }
    }
}

impl fmt::Display for ControlRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ControlRegister::VBR => write!(f, "%vbr"),
        }
    }
}

impl fmt::Display for XRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            XRegister::Data(reg) => write!(f, "d{}", reg),
            XRegister::Address(reg) => write!(f, "a{}", reg),
        }
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
            Target::IndirectARegOffset(reg, offset) => write!(f, "(%a{} + #{:04x})", reg, offset),
            Target::IndirectARegXRegOffset(reg, xreg, offset, scale, _) => {
                let scale_str = if *scale != 0 { format!("<< {}", scale) } else { "".to_string() };
                write!(f, "(%a{} + %{} + #{:04x}{})", reg, xreg, offset, scale_str)
            },
            Target::IndirectMemory(value) => write!(f, "(#{:08x})", value),
            Target::IndirectPCOffset(offset) => write!(f, "(%pc + #{:04x})", offset),
            Target::IndirectPCXRegOffset(xreg, offset, scale, _) => {
                let scale_str = if *scale != 0 { format!("<< {}", scale) } else { "".to_string() };
                write!(f, "(%pc + %{} + #{:04x}{})", xreg, offset, scale_str)
            },
        }
    }
}

fn fmt_movem_mask(mask: u16) -> String {
    format!("something")
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Instruction::ABCD(src, dest) => write!(f, "abcd\t{}, {}", src, dest),
            Instruction::ADD(src, dest, size) => write!(f, "add{}\t{}, {}", size, src, dest),
            Instruction::AND(src, dest, size) => write!(f, "and{}\t{}, {}", size, src, dest),
            Instruction::ANDtoCCR(value) => write!(f, "andb\t{:02x}, %ccr", value),
            Instruction::ANDtoSR(value) => write!(f, "andw\t{:04x}, %sr", value),
            Instruction::ASd(src, dest, size, dir) => write!(f, "as{}{}\t{}, {}", dir, size, src, dest),

            Instruction::Bcc(cond, offset) => write!(f, "b{}\t{}", cond, offset),
            Instruction::BRA(offset) => write!(f, "bra\t{}", offset),
            Instruction::BSR(offset) => write!(f, "bra\t{}", offset),
            Instruction::BCHG(src, dest, size) => write!(f, "bchg{}\t{}, {}", size, src, dest),
            Instruction::BCLR(src, dest, size) => write!(f, "bclr{}\t{}, {}", size, src, dest),
            Instruction::BSET(src, dest, size) => write!(f, "bset{}\t{}, {}", size, src, dest),
            Instruction::BTST(src, dest, size) => write!(f, "btst{}\t{}, {}", size, src, dest),
            //Instruction::BKPT(value),

            Instruction::CHK(target, reg, size) => write!(f, "chk{}\t{}, %d{}", size, target, reg),
            Instruction::CLR(target, size) => write!(f, "clr{}\t{}", size, target),
            Instruction::CMP(src, dest, size) => write!(f, "cmp{}\t{}, {}", size, src, dest),
            Instruction::CMPA(target, reg, size) => write!(f, "cmpa{}\t{}, %a{}", size, target, reg),

            Instruction::DBcc(cond, reg, offset) => write!(f, "db{}\t%d{}, {}", cond, reg, offset),
            Instruction::DIV(src, dest, size, sign) => write!(f, "div{}{}\t{}, {}", sign, size, src, dest),

            Instruction::EOR(src, dest, size) => write!(f, "eor{}\t{}, {}", size, src, dest),
            Instruction::EORtoCCR(value) => write!(f, "eorb\t{:02x}, %ccr", value),
            Instruction::EORtoSR(value) => write!(f, "eorw\t{:04x}, %sr", value),
            Instruction::EXG(src, dest) => write!(f, "exg\t{}, {}", src, dest),
            Instruction::EXT(reg, from_size, to_size) => write!(f, "ext{}{}\t%d{}", from_size, to_size, reg),

            Instruction::ILLEGAL => write!(f, "illegal"),

            Instruction::JMP(target) => write!(f, "jmp\t{}", target),
            Instruction::JSR(target) => write!(f, "jsr\t{}", target),

            Instruction::LEA(target, reg) => write!(f, "lea\t{}, %a{}", target, reg),
            Instruction::LINK(reg, offset) => write!(f, "link\t%a{}, {}", reg, offset),
            Instruction::LSd(src, dest, size, dir) => write!(f, "ls{}{}\t{}, {}", dir, size, src, dest),

            Instruction::MOVE(src, dest, size) => write!(f, "move{}\t{}, {}", size, src, dest),
            Instruction::MOVEA(target, reg, size) => write!(f, "movea{}\t{}, %a{}", size, target, reg),
            Instruction::MOVEfromSR(target) => write!(f, "movew\t%sr, {}", target),
            Instruction::MOVEtoSR(target) => write!(f, "movew\t{}, %sr", target),
            Instruction::MOVEfromCCR(target) => write!(f, "moveb\t%ccr, {}", target),
            Instruction::MOVEtoCCR(target) => write!(f, "moveb\t{}, %ccr", target),
            Instruction::MOVEC(target, reg, dir) => match dir {
                Direction::ToTarget => write!(f, "movec\t{}, {}", reg, target),
                Direction::FromTarget => write!(f, "movec\t{}, {}", target, reg),
            },
            Instruction::MOVEM(target, size, dir, mask) => match dir {
                Direction::ToTarget => write!(f, "movem{}\t{}, {}", size, fmt_movem_mask(*mask), target),
                Direction::FromTarget => write!(f, "movem{}\t{}, {}", size, target, fmt_movem_mask(*mask)),
            },
            Instruction::MOVEP(reg, target, size, dir) => match dir {
                Direction::ToTarget => write!(f, "movep{}\t%d{}, {}", size, reg, target),
                Direction::FromTarget => write!(f, "movep{}\t{}, %d{}", size, target, reg),
            },
            Instruction::MOVEQ(value, reg) => write!(f, "moveq\t#{:02x}, %d{}", value, reg),
            Instruction::MOVEUSP(target, dir) => match dir {
                Direction::ToTarget => write!(f, "movel\t%usp, {}", target),
                Direction::FromTarget => write!(f, "movel\t{}, %usp", target),
            },
            Instruction::MUL(src, dest, size, sign) => write!(f, "mul{}{}\t{}, {}", sign, size, src, dest),

            Instruction::NBCD(target) => write!(f, "nbcd\t{}", target),
            Instruction::NEG(target, size) => write!(f, "neg{}\t{}", size, target),
            Instruction::NEGX(target, size) => write!(f, "negx{}\t{}", size, target),

            Instruction::NOP => write!(f, "nop"),
            Instruction::NOT(target, size) => write!(f, "not{}\t{}", size, target),

            Instruction::OR(src, dest, size) => write!(f, "or{}\t{}, {}", size, src, dest),
            Instruction::ORtoCCR(value) => write!(f, "orb\t{:02x}, %ccr", value),
            Instruction::ORtoSR(value) => write!(f, "orw\t{:04x}, %sr", value),

            Instruction::PEA(target) => write!(f, "pea\t{}", target),

            Instruction::RESET => write!(f, "reset"),
            Instruction::ROd(src, dest, size, dir) => write!(f, "ro{}{}\t{}, {}", dir, size, src, dest),
            Instruction::ROXd(src, dest, size, dir) => write!(f, "rox{}{}\t{}, {}", dir, size, src, dest),
            Instruction::RTE => write!(f, "rte"),
            Instruction::RTR => write!(f, "rtr"),
            Instruction::RTS => write!(f, "rts"),
            Instruction::RTD(offset) => write!(f, "rtd\t{}", offset),

            Instruction::SBCD(src, dest) => write!(f, "sbcd\t{}, {}", src, dest),
            Instruction::Scc(cond, target) => write!(f, "s{}\t{}", cond, target),
            Instruction::STOP(value) => write!(f, "stop\t#{:04x}", value),
            Instruction::SUB(src, dest, size) => write!(f, "sub{}\t{}, {}", size, src, dest),
            Instruction::SWAP(reg) => write!(f, "swap\t%d{}", reg),

            Instruction::TAS(target) => write!(f, "tas\t{}", target),
            Instruction::TST(target, size) => write!(f, "tst{}\t{}", size, target),
            Instruction::TRAP(num) => write!(f, "trap\t{}", num),
            Instruction::TRAPV => write!(f, "trapv"),

            Instruction::UNLK(reg) => write!(f, "unlk\t%a{}", reg),
            _ => write!(f, "UNIMPL"),
        }
    }
}

