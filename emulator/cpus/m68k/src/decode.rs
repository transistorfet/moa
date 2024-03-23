// Instruction Decoding

use core::marker::PhantomData;
use emulator_hal::bus::BusAccess;

use crate::{M68kType, M68kError, M68kBusPort, M68kAddress, Exceptions};
use crate::instructions::{
    Size, Sign, Direction, XRegister, BaseRegister, IndexRegister, RegOrImmediate, ControlRegister, Condition, Target, Instruction,
    sign_extend_to_long,
};


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
const OPCG_ALINE: u8 = 0xA;
const OPCG_CMP_EOR: u8 = 0xB;
const OPCG_MUL_AND: u8 = 0xC;
const OPCG_ADD: u8 = 0xD;
const OPCG_SHIFT: u8 = 0xE;
const OPCG_FLINE: u8 = 0xF;


#[derive(Clone, Debug)]
pub struct M68kDecoder<Instant> {
    pub cputype: M68kType,
    pub is_supervisor: bool,
    pub start: u32,
    pub end: u32,
    pub instruction_word: u16,
    pub instruction: Instruction,
    pub instant: PhantomData<Instant>,
}

pub struct InstructionDecoding<'a, Bus, Instant>
where
    Bus: BusAccess<M68kAddress, Instant = Instant>,
{
    pub(crate) bus: &'a mut Bus,
    pub(crate) memory: &'a mut M68kBusPort<Instant>,
    pub(crate) decoder: &'a mut M68kDecoder<Instant>,
}

impl<Instant> M68kDecoder<Instant>
where
    Instant: Copy,
{
    #[inline]
    pub fn new(cputype: M68kType, is_supervisor: bool, start: u32) -> M68kDecoder<Instant> {
        M68kDecoder {
            cputype,
            is_supervisor,
            start,
            end: start,
            instruction_word: 0,
            instruction: Instruction::NOP,
            instant: PhantomData,
        }
    }

    #[inline]
    pub fn init(&mut self, is_supervisor: bool, start: u32) {
        self.is_supervisor = is_supervisor;
        self.start = start;
        self.end = start;
    }

    #[inline]
    pub fn decode_at<Bus>(
        &mut self,
        bus: &mut Bus,
        memory: &mut M68kBusPort<Instant>,
        is_supervisor: bool,
        start: u32,
    ) -> Result<(), M68kError<Bus::Error>>
    where
        Bus: BusAccess<M68kAddress, Instant = Instant>,
    {
        self.init(is_supervisor, start);
        let mut decoding = InstructionDecoding {
            bus,
            memory,
            decoder: self,
        };
        self.instruction = decoding.decode_next()?;
        Ok(())
    }

    pub fn dump_disassembly<Bus>(&mut self, bus: &mut Bus, memory: &mut M68kBusPort<Instant>, start: u32, length: u32)
    where
        Bus: BusAccess<M68kAddress, Instant = Instant>,
    {
        let mut next = start;
        while next < (start + length) {
            match self.decode_at(bus, memory, self.is_supervisor, next) {
                Ok(()) => {
                    self.dump_decoded(memory.current_clock, bus);
                    next = self.end;
                },
                Err(err) => {
                    println!("{:?}", err);
                    if let M68kError::Exception(Exceptions::IllegalInstruction) = err {
                        println!("    at {:08x}: {:04x}", self.start, bus.read_beu16(memory.current_clock, self.start).unwrap());
                    }
                    return;
                },
            }
        }
    }

    pub fn dump_decoded<Bus>(&mut self, clock: Instant, bus: &mut Bus)
    where
        Bus: BusAccess<M68kAddress, Instant = Instant>,
    {
        let ins_data: Result<String, M68kError<Bus::Error>> = (0..((self.end - self.start) / 2))
            .map(|offset| Ok(format!("{:04x} ", bus.read_beu16(clock, self.start + (offset * 2)).unwrap())))
            .collect();
        println!("{:#010x}: {}\n\t{}\n", self.start, ins_data.unwrap(), self.instruction);
    }
}

impl<'a, Bus, Instant> InstructionDecoding<'a, Bus, Instant>
where
    Bus: BusAccess<M68kAddress, Instant = Instant>,
    Instant: Copy,
{
    #[inline]
    pub fn decode_next(&mut self) -> Result<Instruction, M68kError<Bus::Error>> {
        let ins = self.read_instruction_word()?;
        self.decoder.instruction_word = ins;

        match ((ins & 0xF000) >> 12) as u8 {
            OPCG_BIT_OPS => self.decode_group_bit_ops(ins),
            OPCG_MOVE_BYTE => self.decode_group_move_byte(ins),
            OPCG_MOVE_LONG => self.decode_group_move_long(ins),
            OPCG_MOVE_WORD => self.decode_group_move_word(ins),
            OPCG_MISC => self.decode_group_misc(ins),
            OPCG_ADDQ_SUBQ => self.decode_group_addq_subq(ins),
            OPCG_BRANCH => self.decode_group_branch(ins),
            OPCG_MOVEQ => self.decode_group_moveq(ins),
            OPCG_DIV_OR => self.decode_group_div_or(ins),
            OPCG_SUB => self.decode_group_sub(ins),
            OPCG_ALINE => Ok(Instruction::UnimplementedA(ins)),
            OPCG_CMP_EOR => self.decode_group_cmp_eor(ins),
            OPCG_MUL_AND => self.decode_group_mul_and(ins),
            OPCG_ADD => self.decode_group_add(ins),
            OPCG_SHIFT => self.decode_group_shift(ins),
            OPCG_FLINE => Ok(Instruction::UnimplementedF(ins)),
            _ => Err(M68kError::Exception(Exceptions::IllegalInstruction)),
        }
    }

    #[inline]
    fn decode_group_bit_ops(&mut self, ins: u16) -> Result<Instruction, M68kError<Bus::Error>> {
        let optype = (ins & 0x0F00) >> 8;

        if (ins & 0x13F) == 0x03C {
            match (ins & 0x00C0) >> 6 {
                0b00 => {
                    let data = self.read_instruction_word()?;
                    match optype {
                        0b0000 => Ok(Instruction::ORtoCCR(data as u8)),
                        0b0010 => Ok(Instruction::ANDtoCCR(data as u8)),
                        0b1010 => Ok(Instruction::EORtoCCR(data as u8)),
                        _ => Err(M68kError::Exception(Exceptions::IllegalInstruction)),
                    }
                },
                0b01 => {
                    let data = self.read_instruction_word()?;
                    match optype {
                        0b0000 => Ok(Instruction::ORtoSR(data)),
                        0b0010 => Ok(Instruction::ANDtoSR(data)),
                        0b1010 => Ok(Instruction::EORtoSR(data)),
                        _ => Err(M68kError::Exception(Exceptions::IllegalInstruction)),
                    }
                },
                _ => Err(M68kError::Exception(Exceptions::IllegalInstruction)),
            }
        } else if (ins & 0x138) == 0x108 {
            let dreg = get_high_reg(ins);
            let areg = get_low_reg(ins);
            let dir = if (ins & 0x0080) == 0 {
                Direction::FromTarget
            } else {
                Direction::ToTarget
            };
            let size = if (ins & 0x0040) == 0 { Size::Word } else { Size::Long };
            let offset = self.read_instruction_word()? as i16;
            Ok(Instruction::MOVEP(dreg, areg, offset, size, dir))
        } else if (ins & 0x0100) == 0x0100 || (ins & 0x0F00) == 0x0800 {
            let bitnum = if (ins & 0x0100) == 0x0100 {
                Target::DirectDReg(get_high_reg(ins))
            } else {
                Target::Immediate(self.read_instruction_word()? as u32)
            };

            let target = self.decode_lower_effective_address(ins, Some(Size::Byte))?;
            let size = match target {
                Target::DirectAReg(_) | Target::DirectDReg(_) => Size::Long,
                _ => Size::Byte,
            };

            match (ins & 0x00C0) >> 6 {
                0b00 => Ok(Instruction::BTST(bitnum, target, size)),
                0b01 => Ok(Instruction::BCHG(bitnum, target, size)),
                0b10 => Ok(Instruction::BCLR(bitnum, target, size)),
                0b11 => Ok(Instruction::BSET(bitnum, target, size)),
                _ => Err(M68kError::Exception(Exceptions::IllegalInstruction)),
            }
        } else {
            let size = get_size(ins);
            let data = match size {
                Some(Size::Byte) => self.read_instruction_word()? as u32 & 0xFF,
                Some(Size::Word) => self.read_instruction_word()? as u32,
                Some(Size::Long) => self.read_instruction_long()?,
                None => return Err(M68kError::Exception(Exceptions::IllegalInstruction)),
            };
            let target = self.decode_lower_effective_address(ins, size)?;

            match optype {
                0b0000 => Ok(Instruction::OR(Target::Immediate(data), target, size.unwrap())),
                0b0010 => Ok(Instruction::AND(Target::Immediate(data), target, size.unwrap())),
                0b0100 => Ok(Instruction::SUB(Target::Immediate(data), target, size.unwrap())),
                0b0110 => Ok(Instruction::ADD(Target::Immediate(data), target, size.unwrap())),
                0b1010 => Ok(Instruction::EOR(Target::Immediate(data), target, size.unwrap())),
                0b1100 => Ok(Instruction::CMP(Target::Immediate(data), target, size.unwrap())),
                _ => Err(M68kError::Exception(Exceptions::IllegalInstruction)),
            }
        }
    }

    #[inline]
    fn decode_group_move_byte(&mut self, ins: u16) -> Result<Instruction, M68kError<Bus::Error>> {
        let src = self.decode_lower_effective_address(ins, Some(Size::Byte))?;
        let dest = self.decode_upper_effective_address(ins, Some(Size::Byte))?;
        Ok(Instruction::MOVE(src, dest, Size::Byte))
    }

    #[inline]
    fn decode_group_move_long(&mut self, ins: u16) -> Result<Instruction, M68kError<Bus::Error>> {
        let src = self.decode_lower_effective_address(ins, Some(Size::Long))?;
        let dest = self.decode_upper_effective_address(ins, Some(Size::Long))?;
        if let Target::DirectAReg(reg) = dest {
            Ok(Instruction::MOVEA(src, reg, Size::Long))
        } else {
            Ok(Instruction::MOVE(src, dest, Size::Long))
        }
    }

    #[inline]
    fn decode_group_move_word(&mut self, ins: u16) -> Result<Instruction, M68kError<Bus::Error>> {
        let src = self.decode_lower_effective_address(ins, Some(Size::Word))?;
        let dest = self.decode_upper_effective_address(ins, Some(Size::Word))?;
        if let Target::DirectAReg(reg) = dest {
            Ok(Instruction::MOVEA(src, reg, Size::Word))
        } else {
            Ok(Instruction::MOVE(src, dest, Size::Word))
        }
    }

    #[inline]
    fn decode_group_misc(&mut self, ins: u16) -> Result<Instruction, M68kError<Bus::Error>> {
        let ins_0f00 = ins & 0xF00;
        let ins_00f0 = ins & 0x0F0;

        if (ins & 0x180) == 0x180 {
            if (ins & 0x040) == 0 {
                let size = match get_size(ins) {
                    Some(Size::Word) => Size::Word,
                    Some(Size::Long) if self.decoder.cputype >= M68kType::MC68020 => Size::Long,
                    // On the 68000, long words in CHK are not supported, but the opcode maps to the word size instruction
                    Some(Size::Long) => Size::Word,
                    _ => return Err(M68kError::Exception(Exceptions::IllegalInstruction)),
                };

                let reg = get_high_reg(ins);
                let target = self.decode_lower_effective_address(ins, Some(size))?;
                Ok(Instruction::CHK(target, reg, size))
            } else {
                let src = self.decode_lower_effective_address(ins, None)?;
                let dest = get_high_reg(ins);
                Ok(Instruction::LEA(src, dest))
            }
        } else if (ins & 0xB80) == 0x880 && (ins & 0x038) != 0 {
            let size = if (ins & 0x0040) == 0 { Size::Word } else { Size::Long };
            let data = self.read_instruction_word()?;
            let target = self.decode_lower_effective_address(ins, None)?;
            let dir = if (ins & 0x0400) == 0 {
                Direction::ToTarget
            } else {
                Direction::FromTarget
            };
            Ok(Instruction::MOVEM(target, size, dir, data))
        } else if (ins & 0xF80) == 0xC00 && self.decoder.cputype >= M68kType::MC68020 {
            let extension = self.read_instruction_word()?;
            let reg_h = if (extension & 0x0400) != 0 {
                Some(get_low_reg(ins))
            } else {
                None
            };
            let reg_l = ((extension & 0x7000) >> 12) as u8;
            let target = self.decode_lower_effective_address(ins, Some(Size::Long))?;
            let sign = if (ins & 0x0800) == 0 { Sign::Unsigned } else { Sign::Signed };
            match (ins & 0x040) == 0 {
                true => Ok(Instruction::MULL(target, reg_h, reg_l, sign)),
                false => Ok(Instruction::DIVL(target, reg_h, reg_l, sign)),
            }
        } else if (ins & 0x800) == 0 {
            let target = self.decode_lower_effective_address(ins, Some(Size::Word))?;
            match (ins & 0x0700) >> 8 {
                0b000 => match get_size(ins) {
                    Some(size) => Ok(Instruction::NEGX(target, size)),
                    None => Ok(Instruction::MOVEfromSR(target)),
                },
                0b010 => match get_size(ins) {
                    Some(size) => Ok(Instruction::CLR(target, size)),
                    None if self.decoder.cputype >= M68kType::MC68010 => Ok(Instruction::MOVEfromCCR(target)),
                    None => Err(M68kError::Exception(Exceptions::IllegalInstruction)),
                },
                0b100 => match get_size(ins) {
                    Some(size) => Ok(Instruction::NEG(target, size)),
                    None => Ok(Instruction::MOVEtoCCR(target)),
                },
                0b110 => match get_size(ins) {
                    Some(size) => Ok(Instruction::NOT(target, size)),
                    None => Ok(Instruction::MOVEtoSR(target)),
                },
                _ => Err(M68kError::Exception(Exceptions::IllegalInstruction)),
            }
        } else if ins_0f00 == 0x800 || ins_0f00 == 0x900 {
            let opmode = (ins & 0x01C0) >> 6;
            let mode = get_low_mode(ins);
            match (opmode, mode) {
                (0b000, 0b001) if self.decoder.cputype >= M68kType::MC68020 => {
                    let data = self.read_instruction_long()? as i32;
                    Ok(Instruction::LINK(get_low_reg(ins), data))
                },
                (0b000, _) => {
                    let target = self.decode_lower_effective_address(ins, Some(Size::Byte))?;
                    Ok(Instruction::NBCD(target))
                },
                (0b001, 0b000) => Ok(Instruction::SWAP(get_low_reg(ins))),
                (0b001, 0b001) => Ok(Instruction::BKPT(get_low_reg(ins))),
                (0b001, _) => {
                    let target = self.decode_lower_effective_address(ins, None)?;
                    Ok(Instruction::PEA(target))
                },
                (0b010, 0b000) => Ok(Instruction::EXT(get_low_reg(ins), Size::Byte, Size::Word)),
                (0b011, 0b000) => Ok(Instruction::EXT(get_low_reg(ins), Size::Word, Size::Long)),
                (0b111, 0b000) => Ok(Instruction::EXT(get_low_reg(ins), Size::Byte, Size::Long)),
                _ => Err(M68kError::Exception(Exceptions::IllegalInstruction)),
            }
        } else if ins_0f00 == 0xA00 {
            if (ins & 0x0FF) == 0xFC {
                Ok(Instruction::ILLEGAL)
            } else {
                let target = self.decode_lower_effective_address(ins, Some(Size::Word))?;
                match get_size(ins) {
                    Some(size) => Ok(Instruction::TST(target, size)),
                    None => Ok(Instruction::TAS(target)),
                }
            }
        } else if ins_0f00 == 0xE00 {
            if (ins & 0x80) == 0x80 {
                let target = self.decode_lower_effective_address(ins, None)?;
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
                    let data = (self.read_instruction_word()? as i16) as i32;
                    Ok(Instruction::LINK(reg, data))
                } else {
                    Ok(Instruction::UNLK(reg))
                }
            } else if ins_00f0 == 0x60 {
                let reg = get_low_reg(ins);
                let dir = if (ins & 0b1000) == 0 {
                    Direction::FromTarget
                } else {
                    Direction::ToTarget
                };
                Ok(Instruction::MOVEUSP(Target::DirectAReg(reg), dir))
            } else {
                match ins & 0x00FF {
                    0x70 => Ok(Instruction::RESET),
                    0x71 => Ok(Instruction::NOP),
                    0x72 => {
                        let data = self.read_instruction_word()?;
                        Ok(Instruction::STOP(data))
                    },
                    0x73 => Ok(Instruction::RTE),
                    0x74 if self.decoder.cputype >= M68kType::MC68010 => {
                        let offset = self.read_instruction_word()? as i16;
                        Ok(Instruction::RTD(offset))
                    },
                    0x75 => Ok(Instruction::RTS),
                    0x76 => Ok(Instruction::TRAPV),
                    0x77 => Ok(Instruction::RTR),
                    0x7A | 0x7B if self.decoder.cputype >= M68kType::MC68010 => {
                        let dir = if ins & 0x01 == 0 {
                            Direction::ToTarget
                        } else {
                            Direction::FromTarget
                        };
                        let ins2 = self.read_instruction_word()?;
                        let target = match ins2 & 0x8000 {
                            0 => Target::DirectDReg(((ins2 & 0x7000) >> 12) as u8),
                            _ => Target::DirectAReg(((ins2 & 0x7000) >> 12) as u8),
                        };
                        let creg = match ins2 & 0xFFF {
                            0x801 => ControlRegister::VBR,
                            _ => return Err(M68kError::Exception(Exceptions::IllegalInstruction)),
                        };
                        Ok(Instruction::MOVEC(target, creg, dir))
                    },
                    _ => Err(M68kError::Exception(Exceptions::IllegalInstruction)),
                }
            }
        } else {
            Err(M68kError::Exception(Exceptions::IllegalInstruction))
        }
    }

    #[inline]
    fn decode_group_addq_subq(&mut self, ins: u16) -> Result<Instruction, M68kError<Bus::Error>> {
        match get_size(ins) {
            Some(size) => {
                let target = self.decode_lower_effective_address(ins, Some(size))?;
                let mut data = ((ins & 0x0E00) >> 9) as u32;
                if data == 0 {
                    data = 8;
                }

                if let Target::DirectAReg(reg) = target {
                    if (ins & 0x0100) == 0 {
                        Ok(Instruction::ADDA(Target::Immediate(data), reg, size))
                    } else {
                        Ok(Instruction::SUBA(Target::Immediate(data), reg, size))
                    }
                } else if (ins & 0x0100) == 0 {
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
                    let disp = self.read_instruction_word()? as i16;
                    Ok(Instruction::DBcc(condition, reg, disp))
                } else {
                    let target = self.decode_lower_effective_address(ins, Some(Size::Byte))?;
                    Ok(Instruction::Scc(condition, target))
                }
            },
        }
    }

    #[inline]
    fn decode_group_branch(&mut self, ins: u16) -> Result<Instruction, M68kError<Bus::Error>> {
        let mut disp = ((ins & 0xFF) as i8) as i32;
        if disp == 0 {
            disp = (self.read_instruction_word()? as i16) as i32;
        } else if disp == -1 && self.decoder.cputype >= M68kType::MC68020 {
            disp = self.read_instruction_long()? as i32;
        }
        let condition = get_condition(ins);
        match condition {
            Condition::True => Ok(Instruction::BRA(disp)),
            Condition::False => Ok(Instruction::BSR(disp)),
            _ => Ok(Instruction::Bcc(condition, disp)),
        }
    }

    #[inline]
    fn decode_group_moveq(&mut self, ins: u16) -> Result<Instruction, M68kError<Bus::Error>> {
        if (ins & 0x0100) != 0 {
            return Err(M68kError::Exception(Exceptions::IllegalInstruction));
        }
        let reg = get_high_reg(ins);
        let data = (ins & 0xFF) as u8;
        Ok(Instruction::MOVEQ(data, reg))
    }

    #[inline]
    fn decode_group_div_or(&mut self, ins: u16) -> Result<Instruction, M68kError<Bus::Error>> {
        let size = get_size(ins);

        if (ins & 0x1F0) == 0x100 {
            let regx = get_high_reg(ins);
            let regy = get_low_reg(ins);

            match (ins & 0x08) != 0 {
                false => Ok(Instruction::SBCD(Target::DirectDReg(regy), Target::DirectDReg(regx))),
                true => Ok(Instruction::SBCD(Target::IndirectARegDec(regy), Target::IndirectARegDec(regx))),
            }
        } else if let Some(size) = size {
            let data_reg = Target::DirectDReg(get_high_reg(ins));
            let effective_addr = self.decode_lower_effective_address(ins, Some(size))?;
            let (from, to) = if (ins & 0x0100) == 0 {
                (effective_addr, data_reg)
            } else {
                (data_reg, effective_addr)
            };
            Ok(Instruction::OR(from, to, size))
        } else {
            let sign = if (ins & 0x0100) == 0 { Sign::Unsigned } else { Sign::Signed };
            let effective_addr = self.decode_lower_effective_address(ins, Some(Size::Word))?;
            Ok(Instruction::DIVW(effective_addr, get_high_reg(ins), sign))
        }
    }

    #[inline]
    fn decode_group_sub(&mut self, ins: u16) -> Result<Instruction, M68kError<Bus::Error>> {
        let reg = get_high_reg(ins);
        let dir = (ins & 0x0100) >> 8;
        let size = get_size(ins);
        match size {
            Some(size) => {
                if (ins & 0b100110000) == 0b100000000 {
                    let src = get_low_reg(ins);
                    let dest = get_high_reg(ins);
                    match (ins & 0x08) == 0 {
                        true => Ok(Instruction::SUBX(Target::DirectDReg(src), Target::DirectDReg(dest), size)),
                        false => Ok(Instruction::SUBX(Target::IndirectARegDec(src), Target::IndirectARegDec(dest), size)),
                    }
                } else {
                    let target = self.decode_lower_effective_address(ins, Some(size))?;
                    if dir == 0 {
                        Ok(Instruction::SUB(target, Target::DirectDReg(reg), size))
                    } else {
                        Ok(Instruction::SUB(Target::DirectDReg(reg), target, size))
                    }
                }
            },
            None => {
                let size = if dir == 0 { Size::Word } else { Size::Long };
                let target = self.decode_lower_effective_address(ins, Some(size))?;
                Ok(Instruction::SUBA(target, reg, size))
            },
        }
    }

    #[inline]
    fn decode_group_cmp_eor(&mut self, ins: u16) -> Result<Instruction, M68kError<Bus::Error>> {
        let reg = get_high_reg(ins);
        let optype = (ins & 0x0100) >> 8;
        let size = get_size(ins);
        match (optype, size) {
            (0b1, Some(size)) => {
                if get_low_mode(ins) == 0b001 {
                    Ok(Instruction::CMP(Target::IndirectARegInc(get_low_reg(ins)), Target::IndirectARegInc(reg), size))
                } else {
                    let target = self.decode_lower_effective_address(ins, Some(size))?;
                    Ok(Instruction::EOR(Target::DirectDReg(reg), target, size))
                }
            },
            (0b0, Some(size)) => {
                let target = self.decode_lower_effective_address(ins, Some(size))?;
                Ok(Instruction::CMP(target, Target::DirectDReg(reg), size))
            },
            (_, None) => {
                let size = if optype == 0 { Size::Word } else { Size::Long };
                let target = self.decode_lower_effective_address(ins, Some(size))?;
                Ok(Instruction::CMPA(target, reg, size))
            },
            _ => Err(M68kError::Exception(Exceptions::IllegalInstruction)),
        }
    }

    #[inline]
    fn decode_group_mul_and(&mut self, ins: u16) -> Result<Instruction, M68kError<Bus::Error>> {
        let size = get_size(ins);

        if (ins & 0b0001_1111_0000) == 0b0001_0000_0000 {
            let regx = get_high_reg(ins);
            let regy = get_low_reg(ins);

            match (ins & 0x08) != 0 {
                false => Ok(Instruction::ABCD(Target::DirectDReg(regy), Target::DirectDReg(regx))),
                true => Ok(Instruction::ABCD(Target::IndirectARegDec(regy), Target::IndirectARegDec(regx))),
            }
        } else if (ins & 0b0001_0011_0000) == 0b0001_0000_0000 && size.is_some() {
            let regx = get_high_reg(ins);
            let regy = get_low_reg(ins);
            match (ins & 0x00F8) >> 3 {
                0b01000 => Ok(Instruction::EXG(Target::DirectDReg(regx), Target::DirectDReg(regy))),
                0b01001 => Ok(Instruction::EXG(Target::DirectAReg(regx), Target::DirectAReg(regy))),
                0b10001 => Ok(Instruction::EXG(Target::DirectDReg(regx), Target::DirectAReg(regy))),
                _ => Err(M68kError::Exception(Exceptions::IllegalInstruction)),
            }
        } else if let Some(size) = size {
            let data_reg = Target::DirectDReg(get_high_reg(ins));
            let effective_addr = self.decode_lower_effective_address(ins, Some(size))?;
            let (from, to) = if (ins & 0x0100) == 0 {
                (effective_addr, data_reg)
            } else {
                (data_reg, effective_addr)
            };
            Ok(Instruction::AND(from, to, size))
        } else {
            let sign = if (ins & 0x0100) == 0 { Sign::Unsigned } else { Sign::Signed };
            let effective_addr = self.decode_lower_effective_address(ins, Some(Size::Word))?;
            Ok(Instruction::MULW(effective_addr, get_high_reg(ins), sign))
        }
    }

    #[inline]
    fn decode_group_add(&mut self, ins: u16) -> Result<Instruction, M68kError<Bus::Error>> {
        let reg = get_high_reg(ins);
        let dir = (ins & 0x0100) >> 8;
        let size = get_size(ins);
        match size {
            Some(size) => {
                if (ins & 0b100110000) == 0b100000000 {
                    let src = get_low_reg(ins);
                    let dest = get_high_reg(ins);
                    match (ins & 0x08) == 0 {
                        true => Ok(Instruction::ADDX(Target::DirectDReg(src), Target::DirectDReg(dest), size)),
                        false => Ok(Instruction::ADDX(Target::IndirectARegDec(src), Target::IndirectARegDec(dest), size)),
                    }
                } else {
                    let target = self.decode_lower_effective_address(ins, Some(size))?;
                    if dir == 0 {
                        Ok(Instruction::ADD(target, Target::DirectDReg(reg), size))
                    } else {
                        Ok(Instruction::ADD(Target::DirectDReg(reg), target, size))
                    }
                }
            },
            None => {
                let size = if dir == 0 { Size::Word } else { Size::Long };
                let target = self.decode_lower_effective_address(ins, Some(size))?;
                Ok(Instruction::ADDA(target, reg, size))
            },
        }
    }

    fn decode_group_shift(&mut self, ins: u16) -> Result<Instruction, M68kError<Bus::Error>> {
        match get_size(ins) {
            Some(size) => {
                let target = Target::DirectDReg(get_low_reg(ins));
                let rotation = get_high_reg(ins);
                let count = if (ins & 0x0020) == 0 {
                    Target::Immediate(if rotation != 0 { rotation as u32 } else { 8 })
                } else {
                    Target::DirectDReg(rotation)
                };

                if (ins & 0x0100) == 0 {
                    match (ins & 0x0018) >> 3 {
                        0b00 => Ok(Instruction::ASR(count, target, size)),
                        0b01 => Ok(Instruction::LSR(count, target, size)),
                        0b10 => Ok(Instruction::ROXR(count, target, size)),
                        0b11 => Ok(Instruction::ROR(count, target, size)),
                        _ => Err(M68kError::Exception(Exceptions::IllegalInstruction)),
                    }
                } else {
                    match (ins & 0x0018) >> 3 {
                        0b00 => Ok(Instruction::ASL(count, target, size)),
                        0b01 => Ok(Instruction::LSL(count, target, size)),
                        0b10 => Ok(Instruction::ROXL(count, target, size)),
                        0b11 => Ok(Instruction::ROL(count, target, size)),
                        _ => Err(M68kError::Exception(Exceptions::IllegalInstruction)),
                    }
                }
            },
            None => {
                if (ins & 0x800) == 0 {
                    let target = self.decode_lower_effective_address(ins, Some(Size::Word))?;
                    let count = Target::Immediate(1);
                    let size = Size::Word;

                    if (ins & 0x0100) == 0 {
                        match (ins & 0x0600) >> 9 {
                            0b00 => Ok(Instruction::ASR(count, target, size)),
                            0b01 => Ok(Instruction::LSR(count, target, size)),
                            0b10 => Ok(Instruction::ROXR(count, target, size)),
                            0b11 => Ok(Instruction::ROR(count, target, size)),
                            _ => Err(M68kError::Exception(Exceptions::IllegalInstruction)),
                        }
                    } else {
                        match (ins & 0x0600) >> 9 {
                            0b00 => Ok(Instruction::ASL(count, target, size)),
                            0b01 => Ok(Instruction::LSL(count, target, size)),
                            0b10 => Ok(Instruction::ROXL(count, target, size)),
                            0b11 => Ok(Instruction::ROL(count, target, size)),
                            _ => Err(M68kError::Exception(Exceptions::IllegalInstruction)),
                        }
                    }
                } else if self.decoder.cputype > M68kType::MC68020 {
                    // Bitfield instructions (MC68020+)
                    let ext = self.read_instruction_word()?;
                    let reg = ((ext & 0x7000) >> 12) as u8;

                    let offset = match (ext & 0x0800) == 0 {
                        true => RegOrImmediate::Immediate(((ext & 0x07C0) >> 6) as u8),
                        false => RegOrImmediate::DReg(((ext & 0x01C0) >> 6) as u8),
                    };

                    let width = match (ext & 0x0020) == 0 {
                        true => RegOrImmediate::Immediate((ext & 0x001F) as u8),
                        false => RegOrImmediate::DReg((ext & 0x0007) as u8),
                    };

                    let target = self.decode_lower_effective_address(ins, Some(Size::Word))?;
                    match (ins & 0x0700) >> 8 {
                        0b010 => Ok(Instruction::BFCHG(target, offset, width)),
                        0b100 => Ok(Instruction::BFCLR(target, offset, width)),
                        0b011 => Ok(Instruction::BFEXTS(target, offset, width, reg)),
                        0b001 => Ok(Instruction::BFEXTU(target, offset, width, reg)),
                        0b101 => Ok(Instruction::BFFFO(target, offset, width, reg)),
                        0b111 => Ok(Instruction::BFINS(reg, target, offset, width)),
                        0b110 => Ok(Instruction::BFSET(target, offset, width)),
                        0b000 => Ok(Instruction::BFTST(target, offset, width)),
                        _ => Err(M68kError::Exception(Exceptions::IllegalInstruction)),
                    }
                } else {
                    Err(M68kError::Exception(Exceptions::IllegalInstruction))
                }
            },
        }
    }

    fn read_instruction_word(&mut self) -> Result<u16, M68kError<Bus::Error>> {
        let word = self
            .memory
            .read_instruction_word(self.bus, self.decoder.is_supervisor, self.decoder.end)?;
        self.decoder.end += 2;
        Ok(word)
    }

    fn read_instruction_long(&mut self) -> Result<u32, M68kError<Bus::Error>> {
        let word = self
            .memory
            .read_instruction_long(self.bus, self.decoder.is_supervisor, self.decoder.end)?;
        self.decoder.end += 4;
        Ok(word)
    }

    fn decode_lower_effective_address(&mut self, ins: u16, size: Option<Size>) -> Result<Target, M68kError<Bus::Error>> {
        let reg = get_low_reg(ins);
        let mode = get_low_mode(ins);
        self.get_mode_as_target(mode, reg, size)
    }

    fn decode_upper_effective_address(&mut self, ins: u16, size: Option<Size>) -> Result<Target, M68kError<Bus::Error>> {
        let reg = get_high_reg(ins);
        let mode = get_high_mode(ins);
        self.get_mode_as_target(mode, reg, size)
    }

    fn get_extension_displacement(&mut self, select: u16) -> Result<i32, M68kError<Bus::Error>> {
        let result = match select {
            0b00 | 0b01 => 0,
            0b10 => sign_extend_to_long(self.read_instruction_word()? as u32, Size::Word),
            0b11 => self.read_instruction_long()? as i32,
            _ => return Err(M68kError::Exception(Exceptions::IllegalInstruction)),
        };
        Ok(result)
    }

    fn decode_extension_word(&mut self, areg: Option<u8>) -> Result<Target, M68kError<Bus::Error>> {
        let brief_extension = self.read_instruction_word()?;

        let use_brief = (brief_extension & 0x0100) == 0;

        // Decode Index Register
        let xreg_num = ((brief_extension & 0x7000) >> 12) as u8;
        let xreg = if (brief_extension & 0x8000) == 0 {
            XRegister::DReg(xreg_num)
        } else {
            XRegister::AReg(xreg_num)
        };
        let size = if (brief_extension & 0x0800) == 0 {
            Size::Word
        } else {
            Size::Long
        };

        if self.decoder.cputype <= M68kType::MC68010 {
            let index_reg = IndexRegister {
                xreg,
                scale: 0,
                size,
            };
            let displacement = sign_extend_to_long((brief_extension & 0x00FF) as u32, Size::Byte);

            match areg {
                Some(areg) => Ok(Target::IndirectRegOffset(BaseRegister::AReg(areg), Some(index_reg), displacement)),
                None => Ok(Target::IndirectRegOffset(BaseRegister::PC, Some(index_reg), displacement)),
            }
        } else if use_brief {
            let scale = ((brief_extension & 0x0600) >> 9) as u8;
            let index_reg = IndexRegister {
                xreg,
                scale,
                size,
            };
            let displacement = sign_extend_to_long((brief_extension & 0x00FF) as u32, Size::Byte);

            match areg {
                Some(areg) => Ok(Target::IndirectRegOffset(BaseRegister::AReg(areg), Some(index_reg), displacement)),
                None => Ok(Target::IndirectRegOffset(BaseRegister::PC, Some(index_reg), displacement)),
            }
        } else {
            let scale = ((brief_extension & 0x0600) >> 9) as u8;
            let index_reg = IndexRegister {
                xreg,
                scale,
                size,
            };

            let use_base_reg = (brief_extension & 0x0080) == 0;
            let use_index = (brief_extension & 0x0040) == 0;
            let use_sub_indirect = (brief_extension & 0x0007) != 0;
            let pre_not_post = (brief_extension & 0x0004) == 0;

            let opt_base_reg = match (use_base_reg, areg) {
                (false, _) => BaseRegister::None,
                (true, None) => BaseRegister::PC,
                (true, Some(areg)) => BaseRegister::AReg(areg),
            };
            let opt_index_reg = if use_index { Some(index_reg) } else { None };
            let base_disp = self.get_extension_displacement((brief_extension & 0x0030) >> 4)?;
            let outer_disp = self.get_extension_displacement(brief_extension & 0x0003)?;

            match (use_sub_indirect, pre_not_post) {
                (false, _) => Ok(Target::IndirectRegOffset(opt_base_reg, opt_index_reg, base_disp)),
                (true, true) => Ok(Target::IndirectMemoryPreindexed(opt_base_reg, opt_index_reg, base_disp, outer_disp)),
                (true, false) => Ok(Target::IndirectMemoryPostindexed(opt_base_reg, opt_index_reg, base_disp, outer_disp)),
            }
        }
    }

    pub(super) fn get_mode_as_target(&mut self, mode: u8, reg: u8, size: Option<Size>) -> Result<Target, M68kError<Bus::Error>> {
        let value = match mode {
            0b000 => Target::DirectDReg(reg),
            0b001 => Target::DirectAReg(reg),
            0b010 => Target::IndirectAReg(reg),
            0b011 => Target::IndirectARegInc(reg),
            0b100 => Target::IndirectARegDec(reg),
            0b101 => {
                let displacement = sign_extend_to_long(self.read_instruction_word()? as u32, Size::Word);
                Target::IndirectRegOffset(BaseRegister::AReg(reg), None, displacement)
            },
            0b110 => self.decode_extension_word(Some(reg))?,
            0b111 => match reg {
                0b000 => {
                    let value = sign_extend_to_long(self.read_instruction_word()? as u32, Size::Word) as u32;
                    Target::IndirectMemory(value, Size::Word)
                },
                0b001 => {
                    let value = self.read_instruction_long()?;
                    Target::IndirectMemory(value, Size::Long)
                },
                0b010 => {
                    let displacement = sign_extend_to_long(self.read_instruction_word()? as u32, Size::Word);
                    Target::IndirectRegOffset(BaseRegister::PC, None, displacement)
                },
                0b011 => self.decode_extension_word(None)?,
                0b100 => {
                    let data = match size {
                        Some(Size::Byte) | Some(Size::Word) => self.read_instruction_word()? as u32,
                        Some(Size::Long) => self.read_instruction_long()?,
                        None => return Err(M68kError::Exception(Exceptions::IllegalInstruction)),
                    };
                    Target::Immediate(data)
                },
                _ => return Err(M68kError::Exception(Exceptions::IllegalInstruction)),
            },
            _ => return Err(M68kError::Exception(Exceptions::IllegalInstruction)),
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
