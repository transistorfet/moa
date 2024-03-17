use core::fmt::Write;
use femtos::Instant;

use moa_core::{Address, Addressable};

use crate::state::Z80Error;
use crate::instructions::{
    Direction, Condition, Register, RegisterPair, IndexRegister, IndexRegisterHalf, SpecialRegister, InterruptMode, Target,
    LoadTarget, UndocumentedCopy, Instruction,
};

//use emulator_hal::bus::BusAccess;
//
//type Z80Address = (bool, u16);

#[derive(Clone)]
pub struct Z80Decoder {
    pub clock: Instant,
    pub start: u16,
    pub end: u16,
    pub extra_instruction_bytes: u16,
    pub instruction: Instruction,
}

impl Default for Z80Decoder {
    fn default() -> Self {
        Self {
            clock: Instant::START,
            start: 0,
            end: 0,
            extra_instruction_bytes: 0,
            instruction: Instruction::NOP,
        }
    }
}

/*
    fn read_test<B>(&mut self, device: &mut B) -> Result<u8, Z80Error>
    where
        B: BusAccess<Z80Address, Instant = Instant>,
    {
        device.read_u8(self.clock, (false, self.end as u16))
            .map_err(|err| Z80Error::BusError(format!("butts")))
    }
*/

impl Z80Decoder {
    pub fn decode_at(&mut self, memory: &mut dyn Addressable, clock: Instant, start: u16) -> Result<(), Z80Error> {
        self.clock = clock;
        self.start = start;
        self.end = start;
        self.extra_instruction_bytes = 0;
        self.instruction = self.decode_one(memory)?;
        Ok(())
    }

    pub fn decode_one(&mut self, memory: &mut dyn Addressable) -> Result<Instruction, Z80Error> {
        let ins = self.read_instruction_byte(memory)?;
        self.decode_bare(memory, ins, 0)
    }

    pub fn decode_bare(
        &mut self,
        memory: &mut dyn Addressable,
        ins: u8,
        extra_instruction_bytes: u16,
    ) -> Result<Instruction, Z80Error> {
        self.extra_instruction_bytes = extra_instruction_bytes;
        match get_ins_x(ins) {
            0 => match get_ins_z(ins) {
                0 => match get_ins_y(ins) {
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
                },
                1 => {
                    if get_ins_q(ins) == 0 {
                        let data = self.read_instruction_word(memory)?;
                        Ok(Instruction::LD(
                            LoadTarget::DirectRegWord(get_register_pair(get_ins_p(ins))),
                            LoadTarget::ImmediateWord(data),
                        ))
                    } else {
                        Ok(Instruction::ADD16(RegisterPair::HL, get_register_pair(get_ins_p(ins))))
                    }
                },
                2 => {
                    if (ins & 0x20) == 0 {
                        let target = match (ins & 0x10) != 0 {
                            false => LoadTarget::IndirectRegByte(RegisterPair::BC),
                            true => LoadTarget::IndirectRegByte(RegisterPair::DE),
                        };

                        match get_ins_q(ins) != 0 {
                            false => Ok(Instruction::LD(target, LoadTarget::DirectRegByte(Register::A))),
                            true => Ok(Instruction::LD(LoadTarget::DirectRegByte(Register::A), target)),
                        }
                    } else {
                        let addr = self.read_instruction_word(memory)?;
                        match (ins >> 3) & 0x03 {
                            0 => Ok(Instruction::LD(LoadTarget::IndirectWord(addr), LoadTarget::DirectRegWord(RegisterPair::HL))),
                            1 => Ok(Instruction::LD(LoadTarget::DirectRegWord(RegisterPair::HL), LoadTarget::IndirectWord(addr))),
                            2 => Ok(Instruction::LD(LoadTarget::IndirectByte(addr), LoadTarget::DirectRegByte(Register::A))),
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
                4 => Ok(Instruction::INC8(get_register(get_ins_y(ins)))),
                5 => Ok(Instruction::DEC8(get_register(get_ins_y(ins)))),
                6 => {
                    let data = self.read_instruction_byte(memory)?;
                    Ok(Instruction::LD(to_load_target(get_register(get_ins_y(ins))), LoadTarget::ImmediateByte(data)))
                },
                7 => match get_ins_y(ins) {
                    0 => Ok(Instruction::RLCA),
                    1 => Ok(Instruction::RRCA),
                    2 => Ok(Instruction::RLA),
                    3 => Ok(Instruction::RRA),
                    4 => Ok(Instruction::DAA),
                    5 => Ok(Instruction::CPL),
                    6 => Ok(Instruction::SCF),
                    7 => Ok(Instruction::CCF),
                    _ => panic!("InternalError: impossible value"),
                },
                _ => panic!("InternalError: impossible value"),
            },
            1 => {
                if ins == 0x76 {
                    Ok(Instruction::HALT)
                } else {
                    Ok(Instruction::LD(
                        to_load_target(get_register(get_ins_y(ins))),
                        to_load_target(get_register(get_ins_z(ins))),
                    ))
                }
            },
            2 => Ok(get_alu_instruction(get_ins_y(ins), get_register(get_ins_z(ins)))),
            3 => match get_ins_z(ins) {
                0 => Ok(Instruction::RETcc(get_condition(get_ins_y(ins)))),
                1 => {
                    if get_ins_q(ins) == 0 {
                        Ok(Instruction::POP(get_register_pair_alt(get_ins_p(ins))))
                    } else {
                        match get_ins_p(ins) {
                            0 => Ok(Instruction::RET),
                            1 => Ok(Instruction::EXX),
                            2 => Ok(Instruction::JPIndirect(RegisterPair::HL)),
                            3 => Ok(Instruction::LD(
                                LoadTarget::DirectRegWord(RegisterPair::SP),
                                LoadTarget::DirectRegWord(RegisterPair::HL),
                            )),
                            _ => panic!("InternalError: impossible value"),
                        }
                    }
                },
                2 => {
                    let addr = self.read_instruction_word(memory)?;
                    Ok(Instruction::JPcc(get_condition(get_ins_y(ins)), addr))
                },
                3 => match get_ins_y(ins) {
                    0 => {
                        let addr = self.read_instruction_word(memory)?;
                        Ok(Instruction::JP(addr))
                    },
                    1 => self.decode_prefix_cb(memory),
                    2 => {
                        let port = self.read_instruction_byte(memory)?;
                        Ok(Instruction::OUTx(port))
                    },
                    3 => {
                        let port = self.read_instruction_byte(memory)?;
                        Ok(Instruction::INx(port))
                    },
                    4 => Ok(Instruction::EXsp(RegisterPair::HL)),
                    5 => Ok(Instruction::EXhlde),
                    6 => Ok(Instruction::DI),
                    7 => Ok(Instruction::EI),
                    _ => panic!("InternalError: impossible value"),
                },
                4 => {
                    let addr = self.read_instruction_word(memory)?;
                    Ok(Instruction::CALLcc(get_condition(get_ins_y(ins)), addr))
                },
                5 => {
                    if get_ins_q(ins) == 0 {
                        Ok(Instruction::PUSH(get_register_pair_alt(get_ins_p(ins))))
                    } else {
                        match get_ins_p(ins) {
                            0 => {
                                let addr = self.read_instruction_word(memory)?;
                                Ok(Instruction::CALL(addr))
                            },
                            1 => self.decode_prefix_dd_fd(memory, IndexRegister::IX),
                            2 => self.decode_prefix_ed(memory),
                            3 => self.decode_prefix_dd_fd(memory, IndexRegister::IY),
                            _ => panic!("InternalError: impossible value"),
                        }
                    }
                },
                6 => {
                    let data = self.read_instruction_byte(memory)?;
                    Ok(get_alu_instruction(get_ins_y(ins), Target::Immediate(data)))
                },
                7 => Ok(Instruction::RST(get_ins_y(ins) * 8)),
                _ => panic!("InternalError: impossible value"),
            },
            _ => panic!("InternalError: impossible value"),
        }
    }

    pub fn decode_prefix_cb(&mut self, memory: &mut dyn Addressable) -> Result<Instruction, Z80Error> {
        let ins = self.read_instruction_byte(memory)?;
        match get_ins_x(ins) {
            0 => Ok(get_rot_instruction(get_ins_y(ins), get_register(get_ins_z(ins)), None)),
            1 => Ok(Instruction::BIT(get_ins_y(ins), get_register(get_ins_z(ins)))),
            2 => Ok(Instruction::RES(get_ins_y(ins), get_register(get_ins_z(ins)), None)),
            3 => Ok(Instruction::SET(get_ins_y(ins), get_register(get_ins_z(ins)), None)),
            _ => panic!("InternalError: impossible value"),
        }
    }

    pub fn decode_sub_prefix_cb(&mut self, memory: &mut dyn Addressable, reg: IndexRegister) -> Result<Instruction, Z80Error> {
        let offset = self.read_instruction_byte(memory)? as i8;
        let ins = self.read_instruction_byte(memory)?;
        let opt_copy = match get_ins_z(ins) {
            6 => None, //Some(Target::DirectReg(Register::F)),
            z => Some(get_register(z)),
        };

        match get_ins_x(ins) {
            0 => Ok(get_rot_instruction(get_ins_y(ins), Target::IndirectOffset(reg, offset), opt_copy)),
            1 => Ok(Instruction::BIT(get_ins_y(ins), Target::IndirectOffset(reg, offset))),
            2 => Ok(Instruction::RES(get_ins_y(ins), Target::IndirectOffset(reg, offset), opt_copy)),
            3 => Ok(Instruction::SET(get_ins_y(ins), Target::IndirectOffset(reg, offset), opt_copy)),
            _ => panic!("InternalError: impossible value"),
        }
    }

    pub fn decode_prefix_ed(&mut self, memory: &mut dyn Addressable) -> Result<Instruction, Z80Error> {
        let ins = self.read_instruction_byte(memory)?;

        match get_ins_x(ins) {
            0 => Ok(Instruction::NOP),
            1 => match get_ins_z(ins) {
                0 => {
                    let target = get_register(get_ins_y(ins));
                    if let Target::DirectReg(reg) = target {
                        Ok(Instruction::INic(reg))
                    } else {
                        Ok(Instruction::INicz)
                    }
                },
                1 => {
                    let target = get_register(get_ins_y(ins));
                    if let Target::DirectReg(reg) = target {
                        Ok(Instruction::OUTic(reg))
                    } else {
                        Ok(Instruction::OUTicz)
                    }
                },
                2 => {
                    if get_ins_q(ins) == 0 {
                        Ok(Instruction::SBC16(RegisterPair::HL, get_register_pair(get_ins_p(ins))))
                    } else {
                        Ok(Instruction::ADC16(RegisterPair::HL, get_register_pair(get_ins_p(ins))))
                    }
                },
                3 => {
                    let addr = self.read_instruction_word(memory)?;
                    if get_ins_q(ins) == 0 {
                        Ok(Instruction::LD(
                            LoadTarget::IndirectWord(addr),
                            LoadTarget::DirectRegWord(get_register_pair(get_ins_p(ins))),
                        ))
                    } else {
                        Ok(Instruction::LD(
                            LoadTarget::DirectRegWord(get_register_pair(get_ins_p(ins))),
                            LoadTarget::IndirectWord(addr),
                        ))
                    }
                },
                4 => Ok(Instruction::NEG),
                5 => {
                    if get_ins_y(ins) == 1 {
                        Ok(Instruction::RETI)
                    } else {
                        Ok(Instruction::RETN)
                    }
                },
                6 => match get_ins_y(ins) & 0x03 {
                    0 => Ok(Instruction::IM(InterruptMode::Mode0)),
                    1 => Ok(Instruction::IM(InterruptMode::Mode0)),
                    2 => Ok(Instruction::IM(InterruptMode::Mode1)),
                    3 => Ok(Instruction::IM(InterruptMode::Mode2)),
                    _ => panic!("InternalError: impossible value"),
                },
                7 => match get_ins_y(ins) {
                    0 => Ok(Instruction::LDsr(SpecialRegister::I, Direction::FromAcc)),
                    1 => Ok(Instruction::LDsr(SpecialRegister::R, Direction::FromAcc)),
                    2 => Ok(Instruction::LDsr(SpecialRegister::I, Direction::ToAcc)),
                    3 => Ok(Instruction::LDsr(SpecialRegister::R, Direction::ToAcc)),
                    4 => Ok(Instruction::RRD),
                    5 => Ok(Instruction::RLD),
                    _ => Ok(Instruction::NOP),
                },
                _ => panic!("InternalError: impossible value"),
            },
            2 => match ins {
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
            },
            3 => Ok(Instruction::NOP),
            _ => panic!("InternalError: impossible value"),
        }
    }

    pub fn decode_prefix_dd_fd(&mut self, memory: &mut dyn Addressable, index_reg: IndexRegister) -> Result<Instruction, Z80Error> {
        let ins = self.read_instruction_byte(memory)?;

        if ins == 0xCB {
            return self.decode_sub_prefix_cb(memory, index_reg);
        }

        match get_ins_x(ins) {
            0 => {
                if (ins & 0x0F) == 9 {
                    return Ok(Instruction::ADD16(index_reg.into(), get_register_pair_index(get_ins_p(ins), index_reg)));
                }

                match get_ins_p(ins) {
                    2 => match get_ins_z(ins) {
                        1 => {
                            let data = self.read_instruction_word(memory)?;
                            Ok(Instruction::LD(LoadTarget::DirectRegWord(index_reg.into()), LoadTarget::ImmediateWord(data)))
                        },
                        2 => {
                            let addr = self.read_instruction_word(memory)?;
                            let regpair = index_reg.into();
                            match get_ins_q(ins) != 0 {
                                false => Ok(Instruction::LD(LoadTarget::IndirectWord(addr), LoadTarget::DirectRegWord(regpair))),
                                true => Ok(Instruction::LD(LoadTarget::DirectRegWord(regpair), LoadTarget::IndirectWord(addr))),
                            }
                        },
                        3 => match get_ins_q(ins) != 0 {
                            false => Ok(Instruction::INC16(index_reg.into())),
                            true => Ok(Instruction::DEC16(index_reg.into())),
                        },
                        4 => {
                            self.extra_instruction_bytes = 4;
                            let half_target = Target::DirectRegHalf(get_index_register_half(index_reg, get_ins_q(ins)));
                            Ok(Instruction::INC8(half_target))
                        },
                        5 => {
                            self.extra_instruction_bytes = 4;
                            let half_target = Target::DirectRegHalf(get_index_register_half(index_reg, get_ins_q(ins)));
                            Ok(Instruction::DEC8(half_target))
                        },
                        6 => {
                            self.extra_instruction_bytes = 4;
                            let half_target = Target::DirectRegHalf(get_index_register_half(index_reg, get_ins_q(ins)));
                            let data = self.read_instruction_byte(memory)?;
                            Ok(Instruction::LD(to_load_target(half_target), LoadTarget::ImmediateByte(data)))
                        },
                        _ => self.decode_bare(memory, ins, 4),
                    },
                    3 => match ins {
                        0x34 => {
                            let offset = self.read_instruction_byte(memory)? as i8;
                            Ok(Instruction::INC8(Target::IndirectOffset(index_reg, offset)))
                        },
                        0x35 => {
                            let offset = self.read_instruction_byte(memory)? as i8;
                            Ok(Instruction::DEC8(Target::IndirectOffset(index_reg, offset)))
                        },
                        0x36 => {
                            let offset = self.read_instruction_byte(memory)? as i8;
                            let immediate = self.read_instruction_byte(memory)?;
                            Ok(Instruction::LD(
                                LoadTarget::IndirectOffsetByte(index_reg, offset),
                                LoadTarget::ImmediateByte(immediate),
                            ))
                        },
                        _ => self.decode_bare(memory, ins, 4),
                    },
                    _ => self.decode_bare(memory, ins, 4),
                }
            },
            1 => match get_ins_p(ins) {
                0 | 1 => {
                    let target = match self.decode_index_target(memory, index_reg, get_ins_z(ins))? {
                        Some(target) => target,
                        None => return self.decode_bare(memory, ins, 4),
                    };

                    match (ins & 0x18) >> 3 {
                        0 => Ok(Instruction::LD(LoadTarget::DirectRegByte(Register::B), to_load_target(target))),
                        1 => Ok(Instruction::LD(LoadTarget::DirectRegByte(Register::C), to_load_target(target))),
                        2 => Ok(Instruction::LD(LoadTarget::DirectRegByte(Register::D), to_load_target(target))),
                        3 => Ok(Instruction::LD(LoadTarget::DirectRegByte(Register::E), to_load_target(target))),
                        _ => panic!("InternalError: impossible value"),
                    }
                },
                2 => {
                    let src = match get_ins_z(ins) {
                        0 => Target::DirectReg(Register::B),
                        1 => Target::DirectReg(Register::C),
                        2 => Target::DirectReg(Register::D),
                        3 => Target::DirectReg(Register::E),
                        4 => Target::DirectRegHalf(get_index_register_half(index_reg, 0)),
                        5 => Target::DirectRegHalf(get_index_register_half(index_reg, 1)),
                        6 => {
                            let offset = self.read_instruction_byte(memory)? as i8;
                            let src = to_load_target(Target::IndirectOffset(index_reg, offset));
                            if get_ins_q(ins) == 0 {
                                return Ok(Instruction::LD(LoadTarget::DirectRegByte(Register::H), src));
                            } else {
                                return Ok(Instruction::LD(LoadTarget::DirectRegByte(Register::L), src));
                            }
                        },
                        7 => Target::DirectReg(Register::A),
                        _ => panic!("InternalError: impossible value"),
                    };

                    let dest = get_index_register_half(index_reg, get_ins_q(ins));
                    Ok(Instruction::LD(LoadTarget::DirectRegHalfByte(dest), to_load_target(src)))
                },
                3 => {
                    if get_ins_q(ins) == 0 {
                        if get_ins_z(ins) == 6 {
                            return self.decode_bare(memory, ins, 4);
                        }
                        let src = get_register(get_ins_z(ins));
                        let offset = self.read_instruction_byte(memory)? as i8;
                        Ok(Instruction::LD(LoadTarget::IndirectOffsetByte(index_reg, offset), to_load_target(src)))
                    } else {
                        let target = match self.decode_index_target(memory, index_reg, get_ins_z(ins))? {
                            Some(target) => target,
                            None => return self.decode_bare(memory, ins, 4),
                        };

                        Ok(Instruction::LD(LoadTarget::DirectRegByte(Register::A), to_load_target(target)))
                    }
                },
                _ => panic!("InternalError: impossible value"),
            },
            2 => {
                self.extra_instruction_bytes = 4;

                let target = match self.decode_index_target(memory, index_reg, get_ins_z(ins))? {
                    Some(target) => target,
                    None => return self.decode_bare(memory, ins, 4),
                };

                match get_ins_y(ins) {
                    0 => Ok(Instruction::ADDa(target)),
                    1 => Ok(Instruction::ADCa(target)),
                    2 => Ok(Instruction::SUB(target)),
                    3 => Ok(Instruction::SBCa(target)),
                    4 => Ok(Instruction::AND(target)),
                    5 => Ok(Instruction::XOR(target)),
                    6 => Ok(Instruction::OR(target)),
                    7 => Ok(Instruction::CP(target)),
                    _ => panic!("InternalError: impossible value"),
                }
            },
            3 => match ins {
                0xE1 => Ok(Instruction::POP(index_reg.into())),
                0xE3 => Ok(Instruction::EXsp(index_reg.into())),
                0xE5 => Ok(Instruction::PUSH(index_reg.into())),
                0xE9 => Ok(Instruction::JPIndirect(index_reg.into())),
                0xF9 => Ok(Instruction::LD(
                    LoadTarget::DirectRegWord(RegisterPair::SP),
                    LoadTarget::DirectRegWord(index_reg.into()),
                )),
                _ => self.decode_bare(memory, ins, 4),
            },
            _ => panic!("InternalError: impossible value"),
        }
    }

    fn decode_index_target(
        &mut self,
        memory: &mut dyn Addressable,
        index_reg: IndexRegister,
        z: u8,
    ) -> Result<Option<Target>, Z80Error> {
        let result = match z {
            4 => Some(Target::DirectRegHalf(get_index_register_half(index_reg, 0))),
            5 => Some(Target::DirectRegHalf(get_index_register_half(index_reg, 1))),
            6 => {
                let offset = self.read_instruction_byte(memory)? as i8;
                Some(Target::IndirectOffset(index_reg, offset))
            },
            _ => None,
        };
        Ok(result)
    }


    fn read_instruction_byte(&mut self, device: &mut dyn Addressable) -> Result<u8, Z80Error> {
        let byte = device.read_u8(self.clock, self.end as Address)?;
        self.end = self.end.wrapping_add(1);
        Ok(byte)
    }

    fn read_instruction_word(&mut self, device: &mut dyn Addressable) -> Result<u16, Z80Error> {
        let word = device.read_leu16(self.clock, self.end as Address)?;
        self.end = self.end.wrapping_add(2);
        Ok(word)
    }

    pub fn format_instruction_bytes(&mut self, memory: &mut dyn Addressable) -> String {
        let mut ins_data = String::new();
        for offset in 0..self.end.saturating_sub(self.start) {
            write!(ins_data, "{:02x} ", memory.read_u8(self.clock, (self.start + offset) as Address).unwrap()).unwrap()
        }
        ins_data
    }

    pub fn dump_decoded(&mut self, memory: &mut dyn Addressable) {
        let ins_data = self.format_instruction_bytes(memory);
        println!("{:#06x}: {}\n\t{:?}\n", self.start, ins_data, self.instruction);
    }

    pub fn dump_disassembly(&mut self, memory: &mut dyn Addressable, start: u16, length: u16) {
        let mut next = start;
        while next < (start + length) {
            match self.decode_at(memory, self.clock, next) {
                Ok(()) => {
                    self.dump_decoded(memory);
                    next = self.end;
                },
                Err(err) => {
                    println!("{:?}", err);
                    return;
                },
            }
        }
    }
}

impl From<IndexRegister> for RegisterPair {
    fn from(value: IndexRegister) -> Self {
        match value {
            IndexRegister::IX => RegisterPair::IX,
            IndexRegister::IY => RegisterPair::IY,
        }
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

fn get_rot_instruction(rot: u8, target: Target, opt_copy: UndocumentedCopy) -> Instruction {
    match rot {
        0 => Instruction::RLC(target, opt_copy),
        1 => Instruction::RRC(target, opt_copy),
        2 => Instruction::RL(target, opt_copy),
        3 => Instruction::RR(target, opt_copy),
        4 => Instruction::SLA(target, opt_copy),
        5 => Instruction::SRA(target, opt_copy),
        6 => Instruction::SLL(target, opt_copy),
        7 => Instruction::SRL(target, opt_copy),
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
        Target::DirectRegHalf(reg) => LoadTarget::DirectRegHalfByte(reg),
        Target::IndirectReg(reg) => LoadTarget::IndirectRegByte(reg),
        Target::IndirectOffset(reg, offset) => LoadTarget::IndirectOffsetByte(reg, offset),
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

fn get_register_pair_index(reg: u8, index_reg: IndexRegister) -> RegisterPair {
    match reg {
        0 => RegisterPair::BC,
        1 => RegisterPair::DE,
        2 => index_reg.into(),
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

fn get_index_register_half(reg: IndexRegister, q: u8) -> IndexRegisterHalf {
    match reg {
        IndexRegister::IX => {
            if q == 0 {
                IndexRegisterHalf::IXH
            } else {
                IndexRegisterHalf::IXL
            }
        },
        IndexRegister::IY => {
            if q == 0 {
                IndexRegisterHalf::IYH
            } else {
                IndexRegisterHalf::IYL
            }
        },
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


/// Z80 Decode
///
/// Based on an algorithm described in a Romanian book called "Ghidul Programatorului ZX Spectrum"
/// ("The ZX Spectrum Programmer's Guide") via <http://www.z80.info/decoding.htm>
///
/// Instructions are broken up into x, y, and z parts, or alternatively into x, p, q, and z parts
/// +----------------------+
/// Bits : 7 6 5 4 3 2 1 0
///       | X |  Y  |  X  |
///             P  Q
/// +----------------------+

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
