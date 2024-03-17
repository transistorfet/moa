use moa_core::Error;

use crate::instructions::{Instruction, Target, LoadTarget, RegisterPair};

pub enum Z80InstructionCycles {
    Single(u16),
    Branch { taken: u16, not_taken: u16 },
    Repeating { repeating: u16, terminating: u16 },
}

impl Z80InstructionCycles {
    pub fn calculate_cycles(&self, took_branch: bool) -> u16 {
        match self {
            Z80InstructionCycles::Single(cycles) => *cycles,

            Z80InstructionCycles::Branch {
                taken,
                not_taken,
            } => {
                if took_branch {
                    *taken
                } else {
                    *not_taken
                }
            },

            Z80InstructionCycles::Repeating {
                repeating,
                terminating,
            } => {
                if took_branch {
                    *repeating
                } else {
                    *terminating
                }
            },
        }
    }

    pub fn from_instruction(instruction: &Instruction, extra: u16) -> Result<Z80InstructionCycles, Error> {
        let cycles = match instruction {
            Instruction::ADCa(target)
            | Instruction::ADDa(target)
            | Instruction::AND(target)
            | Instruction::CP(target)
            | Instruction::SBCa(target)
            | Instruction::SUB(target)
            | Instruction::OR(target)
            | Instruction::XOR(target) => match target {
                Target::DirectReg(_) | Target::DirectRegHalf(_) => 4,
                Target::IndirectReg(_) => 7,
                Target::Immediate(_) => 7,
                Target::IndirectOffset(_, _) => 19,
            },

            Instruction::ADC16(_, _) | Instruction::SBC16(_, _) => 15,

            Instruction::ADD16(dest_pair, _) => {
                if !dest_pair.is_index_reg() {
                    11
                } else {
                    15
                }
            },

            Instruction::BIT(_, target) => match target {
                Target::DirectReg(_) => 8,
                Target::IndirectReg(_) => 12,
                Target::IndirectOffset(_, _) => 20,
                _ => return Err(Error::new(format!("unexpected instruction: {:?}", instruction))),
            },

            Instruction::CALL(_) => 17,

            Instruction::CALLcc(_, _) => {
                return Ok(Z80InstructionCycles::Branch {
                    taken: 17 + extra,
                    not_taken: 10 + extra,
                });
            },

            Instruction::CCF => 4,

            Instruction::CPD
            | Instruction::CPI
            | Instruction::IND
            | Instruction::INI
            | Instruction::LDD
            | Instruction::LDI
            | Instruction::OUTD
            | Instruction::OUTI => 16,

            Instruction::CPDR
            | Instruction::CPIR
            | Instruction::INDR
            | Instruction::INIR
            | Instruction::LDDR
            | Instruction::LDIR
            | Instruction::OTDR
            | Instruction::OTIR => {
                return Ok(Z80InstructionCycles::Repeating {
                    repeating: 21 + extra,
                    terminating: 16 + extra,
                });
            },

            Instruction::CPL => 4,
            Instruction::DAA => 4,

            Instruction::DEC8(target) | Instruction::INC8(target) => match target {
                Target::DirectReg(_) | Target::DirectRegHalf(_) => 4,
                Target::IndirectReg(_) => 11,
                Target::IndirectOffset(_, _) => 23,
                _ => return Err(Error::new(format!("unexpected instruction: {:?}", instruction))),
            },

            Instruction::DEC16(regpair) | Instruction::INC16(regpair) => {
                if !regpair.is_index_reg() {
                    6
                } else {
                    10
                }
            },

            Instruction::DI | Instruction::EI => 4,

            Instruction::DJNZ(_) => {
                return Ok(Z80InstructionCycles::Branch {
                    taken: 13 + extra,
                    not_taken: 8 + extra,
                });
            },

            Instruction::EXX => 4,
            Instruction::EXafaf => 4,
            Instruction::EXhlde => 4,
            Instruction::EXsp(regpair) => {
                if !regpair.is_index_reg() {
                    19
                } else {
                    23
                }
            },

            Instruction::HALT => 4,
            Instruction::IM(_) => 8,

            Instruction::INic(_) | Instruction::INicz | Instruction::OUTic(_) | Instruction::OUTicz => 12,

            Instruction::INx(_) | Instruction::OUTx(_) => 11,

            Instruction::JP(_) => 10,
            Instruction::JR(_) => 12,

            Instruction::JPIndirect(regpair) => {
                if !regpair.is_index_reg() {
                    4
                } else {
                    8
                }
            },

            Instruction::JPcc(_, _) => 10,

            Instruction::JRcc(_, _) => {
                return Ok(Z80InstructionCycles::Branch {
                    taken: 12 + extra,
                    not_taken: 7 + extra,
                });
            },

            Instruction::LD(dest, src) => {
                match (dest, src) {
                    // 8-Bit Operations
                    (LoadTarget::DirectRegByte(_), LoadTarget::DirectRegByte(_)) => 4,

                    (LoadTarget::DirectRegHalfByte(_), LoadTarget::DirectRegByte(_))
                    | (LoadTarget::DirectRegByte(_), LoadTarget::DirectRegHalfByte(_))
                    | (LoadTarget::DirectRegHalfByte(_), LoadTarget::DirectRegHalfByte(_)) => 8,

                    (LoadTarget::DirectRegByte(_) | LoadTarget::DirectRegHalfByte(_), LoadTarget::ImmediateByte(_)) => 7,
                    (LoadTarget::IndirectRegByte(_), LoadTarget::ImmediateByte(_)) => 10,

                    (LoadTarget::IndirectOffsetByte(_, _), _) | (_, LoadTarget::IndirectOffsetByte(_, _)) => 19,

                    (_, LoadTarget::IndirectRegByte(_)) | (LoadTarget::IndirectRegByte(_), _) => 7,

                    (_, LoadTarget::IndirectByte(_)) | (LoadTarget::IndirectByte(_), _) => 13,

                    // 16-Bit Operations
                    (LoadTarget::DirectRegWord(regpair), LoadTarget::ImmediateWord(_))
                    | (LoadTarget::ImmediateWord(_), LoadTarget::DirectRegWord(regpair)) => {
                        if !regpair.is_index_reg() {
                            10
                        } else {
                            14
                        }
                    },

                    (LoadTarget::DirectRegWord(_), LoadTarget::DirectRegWord(regpair)) => {
                        if !regpair.is_index_reg() {
                            6
                        } else {
                            10
                        }
                    },

                    (LoadTarget::IndirectWord(_), LoadTarget::DirectRegWord(RegisterPair::HL))
                    | (LoadTarget::DirectRegWord(RegisterPair::HL), LoadTarget::IndirectWord(_)) => 16,

                    (LoadTarget::IndirectWord(_), _) | (_, LoadTarget::IndirectWord(_)) => 20,

                    _ => return Err(Error::new(format!("unexpected instruction: {:?}", instruction))),
                }
            },

            Instruction::LDsr(_, _) => 9,

            Instruction::NEG => 8,
            Instruction::NOP => 4,

            Instruction::POP(regpair) => {
                if !regpair.is_index_reg() {
                    10
                } else {
                    14
                }
            },
            Instruction::PUSH(regpair) => {
                if !regpair.is_index_reg() {
                    11
                } else {
                    15
                }
            },

            Instruction::RES(_, target, _) | Instruction::SET(_, target, _) => match target {
                Target::DirectReg(_) => 8,
                Target::IndirectReg(_) => 15,
                Target::IndirectOffset(_, _) => 23,
                _ => return Err(Error::new(format!("unexpected instruction: {:?}", instruction))),
            },

            Instruction::RET => 10,
            Instruction::RETI => 14,
            Instruction::RETN => 14,

            Instruction::RETcc(_) => {
                return Ok(Z80InstructionCycles::Branch {
                    taken: 11 + extra,
                    not_taken: 5 + extra,
                });
            },

            Instruction::RL(target, _)
            | Instruction::RLC(target, _)
            | Instruction::RR(target, _)
            | Instruction::RRC(target, _)
            | Instruction::SLA(target, _)
            | Instruction::SLL(target, _)
            | Instruction::SRA(target, _)
            | Instruction::SRL(target, _) => match target {
                Target::DirectReg(_) => 8,
                Target::IndirectReg(_) => 15,
                Target::IndirectOffset(_, _) => 23,
                _ => return Err(Error::new(format!("unexpected instruction: {:?}", instruction))),
            },

            Instruction::RLA | Instruction::RLCA | Instruction::RRA | Instruction::RRCA => 4,

            Instruction::RLD => 18,
            Instruction::RRD => 18,

            Instruction::RST(_) => 11,

            Instruction::SCF => 4,
        };
        Ok(Z80InstructionCycles::Single(cycles + extra))
    }
}
