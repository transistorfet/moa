
use crate::error::Error;
use crate::devices::{Address, Addressable};

use super::state::{Z80, Z80Type, Register};


#[derive(Copy, Clone, Debug)]
pub enum Size {
    Byte,
    Word,
}

#[derive(Copy, Clone, Debug)]
pub enum Condition {
    NotZero,
    Zero,
    NotCarry,
    Carry,
    ParityOdd,
    ParityEven,
    Positive,
    Negative,
}

#[derive(Copy, Clone, Debug)]
pub enum RegisterPair {
    BC,
    DE,
    HL,
    AF,
    SP,
}

#[derive(Copy, Clone, Debug)]
pub enum SpecialRegister {
    I,
    R,
}

#[derive(Copy, Clone, Debug)]
pub enum Target {
    DirectReg(Register),
    IndirectReg(RegisterPair),
    Immediate(u8),
}

#[derive(Copy, Clone, Debug)]
pub enum LoadTarget {
    DirectRegByte(Register),
    DirectRegWord(RegisterPair),
    IndirectRegByte(RegisterPair),
    IndirectRegWord(RegisterPair),
    DirectAltRegByte(Register),
    DirectSpecialRegByte(SpecialRegister),
    IndirectByte(u16),
    IndirectWord(u16),
    ImmediateByte(u8),
    ImmediateWord(u16),
}

#[derive(Clone, Debug)]
pub enum Instruction {
    ADCa(Target),
    ADChl(RegisterPair),
    ADDa(Target),
    ADDhl(RegisterPair),
    AND(Target),
    CP(Target),
    NEG,
    OR(Target),
    SBCa(Target),
    SBChl(RegisterPair),
    SUB(Target),
    XOR(Target),

    BIT(u8, Target),
    RES(u8, Target),
    RL(Target),
    RLC(Target),
    RR(Target),
    RRC(Target),
    SET(u8, Target),
    SLA(Target),
    SLL(Target),
    SRA(Target),
    SRL(Target),

    DEC8(Target),
    DEC16(RegisterPair),
    INC8(Target),
    INC16(RegisterPair),

    EXX,
    EXafaf,
    EXhlsp,
    EXhlde,
    LD(LoadTarget, LoadTarget),
    POP(RegisterPair),
    PUSH(RegisterPair),

    INx(u8),
    INic(Register),
    OUTx(u8),
    OUTic(Register),

    CALL(u16),
    CALLcc(Condition, u16),
    DJNZ(i8),
    JP(u16),
    JPIndirectHL,
    JPcc(Condition, u16),
    JR(i8),
    JRcc(Condition, i8),
    RET,
    RETI,
    RETN,
    RETcc(Condition),

    DI,
    EI,
    IM(u8),
    NOP,
    HALT,
    RST(u8),

    CCF,
    CPL,
    DAA,
    RLA,
    RLCA,
    RRA,
    RRCA,
    RRD,
    RLD,
    SCF,

    CPD,
    CPDR,
    CPI,
    CPIR,
    IND,
    INDR,
    INI,
    INIR,
    LDD,
    LDDR,
    LDI,
    LDIR,
    OTDR,
    OTIR,
    OUTD,
    OUTI,
}

pub struct Z80Decoder {
    pub start: u16,
    pub end: u16,
    pub instruction: Instruction,
}

impl Z80Decoder {
    pub fn new() -> Self {
        Self {
            start: 0,
            end: 0,
            instruction: Instruction::NOP,
        }
    }
}

impl Z80Decoder {
    pub fn decode_at(&mut self, memory: &mut dyn Addressable, start: u16) -> Result<(), Error> {
        self.start = start;
        self.end = start;
        self.instruction = self.decode_one(memory)?;
        Ok(())
    }

    pub fn decode_one(&mut self, memory: &mut dyn Addressable) -> Result<Instruction, Error> {
        let ins = self.read_instruction_byte(memory)?;

        match get_ins_x(ins) {
            0 => {
                match get_ins_z(ins) {
                    0 => {
                        match get_ins_y(ins) {
                            0 => Ok(Instruction::NOP),
                            1 => Ok(Instruction::EXafaf),
                            2 => {
                                let offset = self.read_instruction_byte(memory)? as i8;
                                Ok(Instruction::DJNZ(offset))
                            },
                            3 => {
                                let offset = self.read_instruction_byte(memory)? as i8;
                                Ok(Instruction::JR(offset))
                            },
                            y => {
                                let offset = self.read_instruction_byte(memory)? as i8;
                                Ok(Instruction::JRcc(get_condition(y - 4), offset))
                            },
                        }
                    },
                    1 => {
                        if get_ins_q(ins) == 0 {
                            let data = self.read_instruction_word(memory)?;
                            Ok(Instruction::LD(LoadTarget::DirectRegWord(get_register_pair(get_ins_p(ins))), LoadTarget::ImmediateWord(data)))
                        } else {
                            Ok(Instruction::ADDhl(get_register_pair(get_ins_p(ins))))
                        }
                    },
                    2 => {
                        if (ins & 0x20) == 0 {
                            let target = match (ins & 0x10) != 0 {
                                false => LoadTarget::IndirectRegByte(RegisterPair::BC),
                                true => LoadTarget::IndirectRegByte(RegisterPair::DE),
                            };

                            match (ins & 0x08) != 0 {
                                false => Ok(Instruction::LD(target, LoadTarget::DirectRegByte(Register::A))),
                                true => Ok(Instruction::LD(LoadTarget::DirectRegByte(Register::A), target)),
                            }
                        } else {
                            let addr = self.read_instruction_word(memory)?;
                            match (ins >> 3) & 0x03 {
                                0 => Ok(Instruction::LD(LoadTarget::IndirectWord(addr), LoadTarget::DirectRegWord(RegisterPair::HL))),
                                1 => Ok(Instruction::LD(LoadTarget::IndirectByte(addr), LoadTarget::DirectRegByte(Register::A))),
                                2 => Ok(Instruction::LD(LoadTarget::DirectRegWord(RegisterPair::HL), LoadTarget::IndirectWord(addr))),
                                3 => Ok(Instruction::LD(LoadTarget::DirectRegByte(Register::A), LoadTarget::IndirectByte(addr))),
                                _ => panic!("InternalError: impossible value"),
                            }
                        }
                    },
                    3 => {
                        if get_ins_q(ins) == 0 {
                            Ok(Instruction::INC16(get_register_pair(get_ins_p(ins))))
                        } else {
                            Ok(Instruction::DEC16(get_register_pair(get_ins_p(ins))))
                        }
                    },
                    4 => {
                        Ok(Instruction::INC8(get_register(get_ins_y(ins))))
                    },
                    5 => {
                        Ok(Instruction::DEC8(get_register(get_ins_y(ins))))
                    },
                    6 => {
                        let data = self.read_instruction_byte(memory)?;
                        Ok(Instruction::LD(to_load_target(get_register(get_ins_y(ins))), LoadTarget::ImmediateByte(data)))
                    },
                    7 => {
                        match get_ins_y(ins) {
                            0 => Ok(Instruction::RLCA),
                            1 => Ok(Instruction::RRCA),
                            2 => Ok(Instruction::RLA),
                            3 => Ok(Instruction::RRA),
                            4 => Ok(Instruction::DAA),
                            5 => Ok(Instruction::CPL),
                            6 => Ok(Instruction::SCF),
                            7 => Ok(Instruction::CCF),
                            _ => panic!("InternalError: impossible value"),
                        }
                    },
                    _ => panic!("InternalError: impossible value"),
                }
            },
            1 => {
                if ins == 0x76 {
                    Ok(Instruction::HALT)
                } else {
                    Ok(Instruction::LD(to_load_target(get_register(get_ins_y(ins))), to_load_target(get_register(get_ins_z(ins)))))
                }
            },
            2 => {
                Ok(get_alu_instruction(get_ins_y(ins), get_register(get_ins_z(ins))))
            },
            3 => {
                match get_ins_z(ins) {
                    0 => {
                        Ok(Instruction::RETcc(get_condition(get_ins_y(ins))))
                    },
                    1 => {
                        if get_ins_q(ins) == 0 {
                            Ok(Instruction::POP(get_register_pair_alt(get_ins_p(ins))))
                        } else {
                            match get_ins_p(ins) {
                                0 => Ok(Instruction::RET),
                                1 => Ok(Instruction::EXX),
                                2 => Ok(Instruction::JPIndirectHL),
                                3 => Ok(Instruction::LD(LoadTarget::DirectRegWord(RegisterPair::SP), LoadTarget::DirectRegWord(RegisterPair::HL))),
                                _ => panic!("InternalError: impossible value"),
                            }
                        }
                    },
                    2 => {
                        let addr = self.read_instruction_word(memory)?;
                        Ok(Instruction::JPcc(get_condition(get_ins_y(ins)), addr))
                    },
                    3 => {
                        match get_ins_y(ins) {
                            0 => {
                                let addr = self.read_instruction_word(memory)?;
                                Ok(Instruction::JP(addr))
                            },
                            1 => {
                                self.decode_prefix_cb(memory)
                            },
                            2 => {
                                let port = self.read_instruction_byte(memory)?;
                                Ok(Instruction::OUTx(port))
                            },
                            3 => {
                                let port = self.read_instruction_byte(memory)?;
                                Ok(Instruction::INx(port))
                            },
                            4 => Ok(Instruction::EXhlsp),
                            5 => Ok(Instruction::EXhlde),
                            6 => Ok(Instruction::DI),
                            7 => Ok(Instruction::EI),
                            _ => panic!("InternalError: impossible value"),
                        }
                    },
                    4 => {
                        let addr = self.read_instruction_word(memory)?;
                        Ok(Instruction::CALLcc(get_condition(get_ins_y(ins)), addr))
                    }
                    5 => {
                        if get_ins_q(ins) == 0 {
                            Ok(Instruction::PUSH(get_register_pair_alt(get_ins_p(ins))))
                        } else {
                            match get_ins_p(ins) {
                                0 => {
                                    let addr = self.read_instruction_word(memory)?;
                                    Ok(Instruction::CALL(addr))
                                },
                                1 => self.decode_prefix_dd(memory),
                                2 => self.decode_prefix_ed(memory),
                                3 => self.decode_prefix_fd(memory),
                                _ => panic!("Undecoded Instruction"),
                            }
                        }
                    }
                    6 => {
                        let data = self.read_instruction_byte(memory)?;
                        Ok(get_alu_instruction(get_ins_y(ins), Target::Immediate(data)))
                    },
                    7 => {
                        Ok(Instruction::RST(get_ins_y(ins) * 8))
                    },
                    _ => panic!("InternalError: impossible value"),
                }
            },
            _ => panic!("Undecoded Instruction"),
        }
    }

    pub fn decode_prefix_cb(&mut self, memory: &mut dyn Addressable) -> Result<Instruction, Error> {
        let ins = self.read_instruction_byte(memory)?;
        match get_ins_x(ins) {
            0 => Ok(get_rot_instruction(get_ins_y(ins), get_register(get_ins_z(ins)))),
            1 => Ok(Instruction::BIT(get_ins_y(ins), get_register(get_ins_z(ins)))),
            2 => Ok(Instruction::RES(get_ins_y(ins), get_register(get_ins_z(ins)))),
            3 => Ok(Instruction::SET(get_ins_y(ins), get_register(get_ins_z(ins)))),
            _ => panic!("InternalError: impossible value"),
        }
    }

    pub fn decode_prefix_dd(&mut self, memory: &mut dyn Addressable) -> Result<Instruction, Error> {
        panic!("DD instructions unimplemented")
    }

    pub fn decode_prefix_ed(&mut self, memory: &mut dyn Addressable) -> Result<Instruction, Error> {
        let ins = self.read_instruction_byte(memory)?;

        match get_ins_x(ins) {
            0 => Ok(Instruction::NOP),
            1 => {
                match get_ins_z(ins) {
                    0 => {
                        let target = get_register(get_ins_y(ins));
                        if let Target::DirectReg(reg) = target {
                            Ok(Instruction::INic(reg))
                        } else {
                            //Ok(Instruction::INic())
                            panic!("Unimplemented");
                        }
                    },
                    1 => {
                        let target = get_register(get_ins_y(ins));
                        if let Target::DirectReg(reg) = target {
                            Ok(Instruction::OUTic(reg))
                        } else {
                            //Ok(Instruction::OUTic())
                            panic!("Unimplemented");
                        }
                    },
                    2 => {
                        if get_ins_q(ins) == 0 {
                            Ok(Instruction::SBChl(get_register_pair(get_ins_p(ins))))
                        } else {
                            Ok(Instruction::ADChl(get_register_pair(get_ins_p(ins))))
                        }
                    },
                    3 => {
                        let addr = self.read_instruction_word(memory)?;
                        if get_ins_q(ins) == 0 {
                            Ok(Instruction::LD(LoadTarget::IndirectWord(addr), LoadTarget::DirectRegWord(get_register_pair(get_ins_p(ins)))))
                        } else {
                            Ok(Instruction::LD(LoadTarget::DirectRegWord(get_register_pair(get_ins_p(ins))), LoadTarget::IndirectWord(addr)))
                        }
                    },
                    4 => {
                        Ok(Instruction::NEG)
                    },
                    5 => {
                        if get_ins_y(ins) == 1 {
                            Ok(Instruction::RETI)
                        } else {
                            Ok(Instruction::RETN)
                        }
                    },
                    6 => {
                        Ok(Instruction::IM(get_ins_y(ins)))
                    },
                    7 => {
                        match get_ins_y(ins) {
                            0 => Ok(Instruction::LD(LoadTarget::DirectSpecialRegByte(SpecialRegister::I), LoadTarget::DirectRegByte(Register::A))),
                            1 => Ok(Instruction::LD(LoadTarget::DirectSpecialRegByte(SpecialRegister::R), LoadTarget::DirectRegByte(Register::A))),
                            2 => Ok(Instruction::LD(LoadTarget::DirectRegByte(Register::A), LoadTarget::DirectSpecialRegByte(SpecialRegister::I))),
                            3 => Ok(Instruction::LD(LoadTarget::DirectRegByte(Register::A), LoadTarget::DirectSpecialRegByte(SpecialRegister::R))),
                            4 => Ok(Instruction::RRD),
                            5 => Ok(Instruction::RLD),
                            _ => Ok(Instruction::NOP),
                        }
                    },
                    _ => panic!("InternalError: impossible value"),
                }
            },
            2 => {
                match ins {
                    0xA0 => Ok(Instruction::LDI),
                    0xA1 => Ok(Instruction::CPI),
                    0xA2 => Ok(Instruction::INI),
                    0xA3 => Ok(Instruction::OUTI),
                    0xA8 => Ok(Instruction::LDD),
                    0xA9 => Ok(Instruction::CPD),
                    0xAA => Ok(Instruction::IND),
                    0xAB => Ok(Instruction::OUTD),
                    0xB0 => Ok(Instruction::LDIR),
                    0xB1 => Ok(Instruction::CPIR),
                    0xB2 => Ok(Instruction::INIR),
                    0xB3 => Ok(Instruction::OTIR),
                    0xB8 => Ok(Instruction::LDDR),
                    0xB9 => Ok(Instruction::CPDR),
                    0xBA => Ok(Instruction::INDR),
                    0xBB => Ok(Instruction::OTDR),
                    _ => Ok(Instruction::NOP),
                }
            },
            3 => Ok(Instruction::NOP),
            _ => panic!("InternalError: impossible value"),
        }
    }

    pub fn decode_prefix_fd(&mut self, memory: &mut dyn Addressable) -> Result<Instruction, Error> {
        panic!("FD instructions unimplemented")
    }


    fn read_instruction_byte(&mut self, device: &mut dyn Addressable) -> Result<u8, Error> {
        let byte = device.read_u8(self.end as Address)?;
        self.end += 1;
        Ok(byte)
    }

    fn read_instruction_word(&mut self, device: &mut dyn Addressable) -> Result<u16, Error> {
        let word = device.read_leu16(self.end as Address)?;
        self.end += 2;
        Ok(word)
    }

    pub fn dump_decoded(&mut self, memory: &mut dyn Addressable) {
        let ins_data: Result<String, Error> =
            (0..(self.end - self.start)).map(|offset|
                Ok(format!("{:02x} ", memory.read_u8((self.start + offset) as Address).unwrap()))
            ).collect();
        println!("{:#06x}: {}\n\t{:?}\n", self.start, ins_data.unwrap(), self.instruction);
    }
}

fn get_alu_instruction(alu: u8, target: Target) -> Instruction {
    match alu {
        0 => Instruction::ADDa(target),
        1 => Instruction::ADCa(target),
        2 => Instruction::SUB(target),
        3 => Instruction::SBCa(target),
        4 => Instruction::AND(target),
        5 => Instruction::XOR(target),
        6 => Instruction::OR(target),
        7 => Instruction::CP(target),
        _ => panic!("InternalError: impossible value"),
    }
}

fn get_rot_instruction(rot: u8, target: Target) -> Instruction {
    match rot {
        0 => Instruction::RLC(target),
        1 => Instruction::RRC(target),
        2 => Instruction::RL(target),
        3 => Instruction::RR(target),
        4 => Instruction::SLA(target),
        5 => Instruction::SRA(target),
        6 => Instruction::SLL(target),
        7 => Instruction::SRL(target),
        _ => panic!("InternalError: impossible value"),
    }
}

fn get_register(reg: u8) -> Target {
    match reg {
        0 => Target::DirectReg(Register::B),
        1 => Target::DirectReg(Register::C),
        2 => Target::DirectReg(Register::D),
        3 => Target::DirectReg(Register::E),
        4 => Target::DirectReg(Register::H),
        5 => Target::DirectReg(Register::L),
        6 => Target::IndirectReg(RegisterPair::HL),
        7 => Target::DirectReg(Register::A),
        _ => panic!("InternalError: impossible value"),
    }
}

fn to_load_target(target: Target) -> LoadTarget {
    match target {
        Target::DirectReg(reg) => LoadTarget::DirectRegByte(reg),
        Target::IndirectReg(reg) => LoadTarget::IndirectRegByte(reg),
        Target::Immediate(data) => LoadTarget::ImmediateByte(data),
    }
}

fn get_register_pair(reg: u8) -> RegisterPair {
    match reg {
        0 => RegisterPair::BC,
        1 => RegisterPair::DE,
        2 => RegisterPair::HL,
        3 => RegisterPair::SP,
        _ => panic!("InternalError: impossible value"),
    }
}

fn get_register_pair_alt(reg: u8) -> RegisterPair {
    match reg {
        0 => RegisterPair::BC,
        1 => RegisterPair::DE,
        2 => RegisterPair::HL,
        3 => RegisterPair::AF,
        _ => panic!("InternalError: impossible value"),
    }
}

fn get_condition(cond: u8) -> Condition {
    match cond {
        0 => Condition::NotZero,
        1 => Condition::Zero,
        2 => Condition::NotCarry,
        3 => Condition::Carry,
        4 => Condition::ParityOdd,
        5 => Condition::ParityEven,
        6 => Condition::Positive,
        7 => Condition::Negative,
        _ => panic!("InternalError: impossible value"),
    }
}

fn get_ins_x(ins: u8) -> u8 {
    (ins >> 6) & 0x03
}

fn get_ins_y(ins: u8) -> u8 {
    (ins >> 3) & 0x07
}

fn get_ins_z(ins: u8) -> u8 {
    ins & 0x07
}

fn get_ins_p(ins: u8) -> u8 {
    (ins >> 4) & 0x03
}

fn get_ins_q(ins: u8) -> u8 {
    (ins >> 3) & 0x01
}

