// m68k Instruction Timing Calclator

use crate::M68kType;
use crate::state::ClockCycles;
use crate::instructions::{Size, Sign, Direction, Target, Instruction};


#[derive(Clone, Debug, PartialEq, Eq)]
pub struct M68kInstructionTiming {
    pub cputype: M68kType,
    pub bus_size: Size,

    pub branched: bool,
    pub reps: u16,

    pub accesses: u8,
    pub internal: u8,
    pub on_branch: u8,
    pub per_rep: u8,
}

impl M68kInstructionTiming {
    pub fn new(cputype: M68kType, bus_width: u8) -> Self {
        let bus_size = if bus_width <= 16 { Size::Word } else { Size::Long };
        Self {
            cputype,
            bus_size,

            branched: false,
            reps: 0,

            accesses: 0,
            internal: 0,
            on_branch: 0,
            per_rep: 0,
        }
    }

    pub fn reset(&mut self) {
        self.accesses = 0;
        self.internal = 0;
        self.on_branch = 0;
        self.per_rep = 0;
    }

    #[inline(always)]
    pub fn add_access(&mut self, size: Size) -> &mut Self {
        self.accesses += self.access_size(size);
        self
    }

    #[inline(always)]
    pub fn add_internal(&mut self, clocks: u8) -> &mut Self {
        self.internal += clocks;
        self
    }

    #[inline(always)]
    pub fn sub_internal(&mut self, clocks: u8) -> &mut Self {
        self.internal = self.internal.saturating_sub(clocks);
        self
    }

    #[inline(always)]
    pub fn add_if_long(&mut self, size: Size, clocks: u8) -> &mut Self {
        self.internal += if size == Size::Long { clocks } else { 0 };
        self
    }

    #[inline(always)]
    pub fn add_reg_v_mem(&mut self, target: &Target, reg: u8, mem: u8) -> &mut Self {
        self.internal += match target {
            Target::DirectDReg(_) | Target::DirectAReg(_) => reg,
            _ => mem,
        };
        self
    }

    #[inline(always)]
    pub fn add_word_v_long(&mut self, size: Size, word: u8, long: u8) -> &mut Self {
        self.internal += if size != Size::Long { word } else { long };
        self
    }

    #[inline(always)]
    pub fn add_standard_set(&mut self, size: Size, target: &Target, areg: (u8, u8), dreg: (u8, u8), mem: (u8, u8)) -> &mut Self {
        match target {
            Target::DirectDReg(_) => self.add_word_v_long(size, dreg.0, dreg.1),
            Target::DirectAReg(_) => self.add_word_v_long(size, areg.0, dreg.1),
            _ => self.add_word_v_long(size, mem.0, mem.1),
        }
    }

    #[inline(always)]
    pub fn add_immediate_set(&mut self, size: Size, target: &Target, reg: (u8, u8), mem: (u8, u8)) -> &mut Self {
        match target {
            Target::DirectDReg(_) | Target::DirectAReg(_) => self.add_word_v_long(size, reg.0, reg.1),
            _ => self.add_word_v_long(size, mem.0, mem.1),
        }
    }

    #[inline(always)]
    pub fn add_indirect_set(&mut self, target: &Target, areg: u8, aoff: u8, indoff: u8, indw: u8, indl: u8) -> &mut Self {
        match target {
            Target::IndirectAReg(_) => self.add_internal(areg),
            Target::IndirectRegOffset(_, None, _) => self.add_internal(aoff),
            Target::IndirectRegOffset(_, Some(_), _) => self.add_internal(indoff),
            Target::IndirectMemory(_, Size::Long) => self.add_internal(indl),
            Target::IndirectMemory(_, _) => self.add_internal(indw),
            _ => panic!("This timing set can't be used with {:?} targetting", target),
        }
    }

    #[inline(always)]
    pub fn add_reg_mem_set(&mut self, size: Size, target: &Target, reg: (u8, u8), mem: (u8, u8)) -> &mut Self {
        match target {
            Target::DirectDReg(_) | Target::DirectAReg(_) => self.add_word_v_long(size, reg.0, reg.1),
            _ => self.add_word_v_long(size, mem.0, mem.1),
        }
    }

    #[inline(always)]
    pub fn add_on_branch(&mut self, clocks: u8) -> &mut Self {
        self.on_branch += clocks;
        self
    }

    #[inline(always)]
    pub fn add_per_rep(&mut self, clocks: u8) -> &mut Self {
        self.per_rep += clocks;
        self
    }

    pub fn add_target(&mut self, size: Size, target: &Target) -> &mut Self {
        match target {
            Target::Immediate(_) => self.add_access(size),
            Target::DirectDReg(_) => self,
            Target::DirectAReg(_) => self,
            Target::IndirectAReg(_) => self.add_access(size),
            Target::IndirectARegInc(_) => self.add_access(size),
            Target::IndirectARegDec(_) => self.add_access(size).add_internal(2),
            Target::IndirectRegOffset(_, index_reg, _) => match index_reg {
                None => self.add_access(size).add_internal(4),
                Some(_) => self.add_access(size).add_internal(6),
            },
            Target::IndirectMemoryPreindexed(_, index_reg, _, _) | Target::IndirectMemoryPostindexed(_, index_reg, _, _) => {
                // TODO this is very wrong, but the 68020 timings are complicated
                match index_reg {
                    None => self.add_access(size).add_internal(4),
                    Some(_) => self.add_access(size).add_internal(6),
                }
            },
            Target::IndirectMemory(_, addr_size) => self.add_access(*addr_size).add_access(size),
        }
    }

    pub fn add_two_targets(&mut self, size: Size, src: &Target, dest: &Target) -> &mut Self {
        match (src, dest) {
            (Target::IndirectARegDec(_), Target::IndirectARegDec(_)) => {
                self.add_target(size, src).add_target(size, dest).sub_internal(2)
            },
            _ => self.add_target(size, src).add_target(size, dest),
        }
    }

    pub fn add_movem(&mut self, size: Size, target: &Target, dir: Direction, n: u8) -> &mut Self {
        if dir == Direction::FromTarget {
            match target {
                Target::IndirectAReg(_) => self.add_word_v_long(size, 12 + 4 * n, 12 + 8 * n),
                Target::IndirectARegInc(_) => self.add_word_v_long(size, 12 + 4 * n, 12 + 8 * n),
                Target::IndirectRegOffset(_, None, _) => self.add_word_v_long(size, 16 + 4 * n, 16 + 8 * n),
                Target::IndirectRegOffset(_, Some(_), _) => self.add_word_v_long(size, 18 + 4 * n, 18 + 8 * n),
                Target::IndirectMemory(_, Size::Long) => self.add_word_v_long(size, 20 + 4 * n, 20 + 8 * n),
                Target::IndirectMemory(_, _) => self.add_word_v_long(size, 16 + 4 * n, 16 + 8 * n),
                _ => panic!("This timing set can't be used with {:?} targetting", target),
            }
        } else {
            match target {
                Target::IndirectAReg(_) => self.add_word_v_long(size, 8 + 4 * n, 8 + 8 * n),
                Target::IndirectARegDec(_) => self.add_word_v_long(size, 8 + 4 * n, 8 + 8 * n),
                Target::IndirectRegOffset(_, None, _) => self.add_word_v_long(size, 12 + 4 * n, 12 + 8 * n),
                Target::IndirectRegOffset(_, Some(_), _) => self.add_word_v_long(size, 14 + 4 * n, 14 + 8 * n),
                Target::IndirectMemory(_, Size::Long) => self.add_word_v_long(size, 16 + 4 * n, 16 + 8 * n),
                Target::IndirectMemory(_, _) => self.add_word_v_long(size, 12 + 4 * n, 12 + 8 * n),
                _ => panic!("This timing set can't be used with {:?} targetting", target),
            }
        }
    }

    pub fn add_instruction(&mut self, instruction: &Instruction) -> &mut Self {
        match self.cputype {
            M68kType::MC68000 | M68kType::MC68010 => self.add_instruction_68000(instruction),
            _ => self.add_instruction_68020(instruction),
        }
    }

    pub fn add_instruction_68000(&mut self, instruction: &Instruction) -> &mut Self {
        match instruction {
            Instruction::ABCD(_, dest) => self.add_reg_v_mem(dest, 6, 18),

            Instruction::ADD(Target::Immediate(x), dest, size) if *x <= 8 => {
                self.add_immediate_set(*size, dest, (4, 8), (8, 12)).add_target(*size, dest)
            }, // ADDQ
            Instruction::ADD(Target::Immediate(_), dest, size) => {
                self.add_immediate_set(*size, dest, (8, 16), (12, 20)).add_target(*size, dest)
            }, // ADDI

            Instruction::ADD(src, dest, size) => self
                .add_standard_set(*size, dest, (8, 6), (4, 6), (8, 12))
                .add_two_targets(*size, src, dest),
            Instruction::ADDA(target, _, size) => self.add_word_v_long(*size, 8, 6).add_target(*size, target),
            Instruction::ADDX(_, dest, size) => self.add_reg_mem_set(*size, dest, (4, 8), (18, 30)),

            Instruction::AND(Target::Immediate(_), dest, size) => {
                self.add_immediate_set(*size, dest, (8, 14), (12, 20)).add_target(*size, dest)
            },
            Instruction::AND(src, dest, size) => self
                .add_standard_set(*size, dest, (0, 0), (4, 6), (8, 12))
                .add_two_targets(*size, src, dest),
            Instruction::ANDtoCCR(_) => self.add_internal(20),
            Instruction::ANDtoSR(_) => self.add_internal(20),

            Instruction::ASL(_, target, size) | Instruction::ASR(_, target, size) => {
                self.add_word_v_long(*size, 6, 8).add_per_rep(2).add_target(*size, target)
            },

            Instruction::Bcc(_, _) => self.add_internal(8).add_on_branch(2),
            Instruction::BRA(_) => self.add_internal(10),
            Instruction::BSR(_) => self.add_internal(18),

            Instruction::BCHG(bit, target, size) => match bit {
                Target::Immediate(_) => self.add_reg_v_mem(target, 12, 12),
                _ => self.add_reg_v_mem(target, 8, 8),
            }
            .add_target(*size, target),
            Instruction::BCLR(bit, target, size) => match bit {
                Target::Immediate(_) => self.add_reg_v_mem(target, 14, 12),
                _ => self.add_reg_v_mem(target, 10, 8),
            }
            .add_target(*size, target),
            Instruction::BSET(bit, target, size) => match bit {
                Target::Immediate(_) => self.add_reg_v_mem(target, 12, 12),
                _ => self.add_reg_v_mem(target, 8, 8),
            }
            .add_target(*size, target),
            Instruction::BTST(bit, target, size) => match bit {
                Target::Immediate(_) => self.add_reg_v_mem(target, 10, 8),
                _ => self.add_reg_v_mem(target, 6, 4),
            }
            .add_target(*size, target),

            Instruction::CHK(_, _, _) => self.add_internal(10),
            Instruction::CLR(target, size) => self
                .add_reg_v_mem(target, 4, 8)
                .add_word_v_long(*size, 0, 2)
                .add_target(*size, target),

            Instruction::CMP(Target::Immediate(_), dest, size) => {
                self.add_immediate_set(*size, dest, (8, 14), (8, 12)).add_target(*size, dest)
            },
            Instruction::CMP(src, dest, size) => self
                .add_standard_set(*size, dest, (6, 6), (4, 6), (0, 0))
                .add_two_targets(*size, src, dest),
            Instruction::CMPA(target, _, size) => self.add_word_v_long(*size, 6, 6).add_target(*size, target),

            Instruction::DBcc(_, _, _) => self.add_internal(10).add_on_branch(4),
            Instruction::DIVW(src, _, sign) => match sign {
                Sign::Unsigned => self.add_internal(140).add_target(Size::Long, src),
                Sign::Signed => self.add_internal(158).add_target(Size::Long, src),
            },

            Instruction::EOR(Target::Immediate(_), dest, size) => {
                self.add_immediate_set(*size, dest, (8, 16), (12, 20)).add_target(*size, dest)
            },
            Instruction::EOR(src, dest, size) => self
                .add_standard_set(*size, dest, (0, 0), (4, 8), (8, 12))
                .add_two_targets(*size, src, dest),
            Instruction::EORtoCCR(_) => self.add_internal(20),
            Instruction::EORtoSR(_) => self.add_internal(20),

            Instruction::EXG(_, _) => self.add_internal(6),
            Instruction::EXT(_, _, _) => self.add_internal(4),

            Instruction::ILLEGAL => self.add_internal(4),

            Instruction::JMP(target) => self.add_indirect_set(target, 8, 10, 14, 10, 12),
            Instruction::JSR(target) => self.add_indirect_set(target, 16, 18, 22, 18, 20),

            Instruction::LEA(target, _) => self.add_indirect_set(target, 4, 8, 12, 8, 12),
            Instruction::LINK(_, _) => self.add_internal(16),
            Instruction::LSL(_, target, size) | Instruction::LSR(_, target, size) => {
                self.add_word_v_long(*size, 6, 8).add_per_rep(2).add_target(*size, target)
            },

            Instruction::MOVE(src, dest, size) => self.add_internal(4).add_two_targets(*size, src, dest),
            Instruction::MOVEA(target, _, size) => self.add_internal(4).add_target(*size, target),
            Instruction::MOVEfromSR(target) => self.add_reg_v_mem(target, 6, 8).add_target(Size::Word, target),
            Instruction::MOVEtoSR(target) => self.add_internal(12).add_target(Size::Word, target),
            Instruction::MOVEfromCCR(target) => self.add_internal(12).add_target(Size::Word, target),
            Instruction::MOVEtoCCR(target) => self.add_internal(12).add_target(Size::Word, target),
            Instruction::MOVEC(target, _, _) => self.add_reg_v_mem(target, 10, 12),
            Instruction::MOVEM(target, size, dir, mask) => self.add_movem(*size, target, *dir, mask.count_ones() as u8),
            Instruction::MOVEP(_, _, _, size, _) => self.add_word_v_long(*size, 16, 24),
            Instruction::MOVEQ(_, _) => self.add_internal(4),
            Instruction::MOVEUSP(_, _) => self.add_internal(4),

            Instruction::MULW(src, _, _) => self.add_internal(70).add_target(Size::Word, src),

            Instruction::NBCD(target) => self.add_reg_v_mem(target, 6, 8),
            Instruction::NEG(target, size) => self.add_reg_mem_set(*size, target, (4, 6), (8, 12)).add_target(*size, target),
            Instruction::NEGX(target, size) => self.add_reg_mem_set(*size, target, (4, 6), (8, 12)).add_target(*size, target),

            Instruction::NOP => self.add_internal(4),
            Instruction::NOT(target, size) => self.add_reg_mem_set(*size, target, (4, 6), (8, 12)).add_target(*size, target),

            Instruction::OR(Target::Immediate(_), dest, size) => {
                self.add_immediate_set(*size, dest, (8, 16), (12, 20)).add_target(*size, dest)
            },
            Instruction::OR(src, dest, size) => self
                .add_standard_set(*size, dest, (0, 0), (4, 6), (8, 12))
                .add_two_targets(*size, src, dest),
            Instruction::ORtoCCR(_) => self.add_internal(20),
            Instruction::ORtoSR(_) => self.add_internal(20),

            Instruction::PEA(target) => self.add_indirect_set(target, 12, 16, 20, 16, 20),

            Instruction::RESET => self.add_internal(132),

            Instruction::ROL(_, target, size) | Instruction::ROR(_, target, size) => {
                self.add_word_v_long(*size, 6, 8).add_per_rep(2).add_target(*size, target)
            },
            Instruction::ROXL(_, target, size) | Instruction::ROXR(_, target, size) => {
                self.add_word_v_long(*size, 6, 8).add_per_rep(2).add_target(*size, target)
            },

            Instruction::RTE => self.add_internal(20),
            Instruction::RTR => self.add_internal(20),
            Instruction::RTS => self.add_internal(16),
            //Instruction::RTD(offset) => ,
            Instruction::SBCD(_, dest) => self.add_reg_v_mem(dest, 6, 18),
            Instruction::Scc(_, target) => self
                .add_reg_v_mem(target, 4, 8)
                .add_on_branch(2)
                .add_target(Size::Byte, target),
            Instruction::STOP(_) => self.add_internal(4),

            Instruction::SUB(Target::Immediate(x), Target::DirectAReg(_), Size::Byte)
            | Instruction::SUB(Target::Immediate(x), Target::DirectAReg(_), Size::Word)
                if *x <= 8 =>
            {
                self.add_internal(8)
            }, // SUBQ with an address reg as dest
            Instruction::SUB(Target::Immediate(x), dest, size) if *x <= 8 => {
                self.add_immediate_set(*size, dest, (4, 8), (8, 12)).add_target(*size, dest)
            }, // SUBQ
            Instruction::SUB(Target::Immediate(_), dest, size) => {
                self.add_immediate_set(*size, dest, (8, 16), (12, 20)).add_target(*size, dest)
            }, // SUBI

            Instruction::SUB(src, dest, size) => self
                .add_standard_set(*size, dest, (0, 0), (4, 6), (8, 12))
                .add_two_targets(*size, src, dest),
            Instruction::SUBA(target, _, size) => self.add_word_v_long(*size, 8, 6).add_target(*size, target),
            Instruction::SUBX(_, dest, size) => self.add_reg_mem_set(*size, dest, (4, 8), (18, 30)),

            Instruction::SWAP(_) => self.add_internal(4),

            Instruction::TAS(target) => self.add_reg_v_mem(target, 4, 14).add_target(Size::Byte, target),
            Instruction::TST(target, size) => self.add_internal(4).add_target(*size, target),
            Instruction::TRAP(_) => self.add_internal(34),
            Instruction::TRAPV => self.add_internal(34),

            Instruction::UNLK(_) => self.add_internal(12),
            Instruction::UnimplementedA(_) => self.add_internal(34),
            Instruction::UnimplementedF(_) => self.add_internal(34),
            _ => {
                //panic!("Unexpected instruction for cpu type {:?}: {:?}", self.cputype, instruction);
                self.add_internal(4)
            },
        }
    }

    pub fn add_instruction_68020(&mut self, _instruction: &Instruction) -> &mut Self {
        //match instruction {
        //    // TODO implement
        //    _ => self.add_internal(4),
        //}
        self.add_internal(4)
    }

    pub fn performed_reset(&mut self) {
        self.internal = 0;
        self.accesses = 4;
        self.branched = false;
        self.reps = 0;
    }

    pub fn increase_reps(&mut self, reps: u16) {
        self.reps += reps;
    }

    pub fn branch_taken(&mut self) {
        self.branched = true;
    }

    pub fn calculate_clocks(&self) -> ClockCycles {
        //println!("{:?}", self);
        (self.accesses as ClockCycles * 4)
            + self.internal as ClockCycles
            + (if self.branched { self.on_branch as ClockCycles } else { 0 })
            + self.per_rep as ClockCycles * self.reps
    }

    #[inline(always)]
    pub fn access_size(&self, size: Size) -> u8 {
        if self.bus_size == Size::Word && size == Size::Long {
            2
        } else {
            1
        }
    }
}
