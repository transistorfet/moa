
use crate::error::Error;
use crate::memory::{Address, AddressSpace};

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
    pub msp: u32,
    pub usp: u32,
    pub flags: u16,
    pub d_reg: [u32; 8],
    pub a_reg: [u32; 8],

    pub vbr: u32,
}

const FLAGS_ON_RESET: u16 = 0x2700;

const FLAGS_SUPERVISOR: u16 = 0x2000;

const ERR_BUS_ERROR: u32 = 2;
const ERR_ADDRESS_ERROR: u32 = 3;
const ERR_ILLEGAL_INSTRUCTION: u32 = 4;

#[derive(Copy, Clone, Debug, PartialEq)]
enum Sign {
    Signed,
    Unsigned,
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum Size {
    Byte,
    Word,
    Long,
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum Condition {
    CarryClear,
    CarrySet,
    Equal,
    NotEqual,
    GreaterThanOrEqual,
    GreaterThan,
    LessThanOrEqual,
    LessThan,
    Minus,
    Plus,
    OverflowClear,
    OverflowSet,
}

#[derive(Clone, Debug, PartialEq)]
enum Target {
    Immediate(u32),
    DirectDReg(u8),
    DirectAReg(u8),
    IndirectAReg(u8),
    IndirectARegInc(u8),
    IndirectARegDec(u8),
    IndirectARegOffset(u8, u16),
    IndirectARegDRegOffset(u8, u8, u16),
    IndirectMemory(u32),
    IndirectPCOffset(u16),
    IndirectPCRegOffset(u8, u16),
}

#[derive(Clone, Debug, PartialEq)]
enum Instruction {
    ADD(Target, Target, Size),
    AND(Target, Target, Size),
    ANDCCR(u8),
    ANDSR(u16),

    Bcc(Condition, u16),
    BRA(u16),
    BSR(u16),

    CLR(Target, Size),
    CMP(Target, Target, Size),

    DBcc(Condition, u16),
    DIV(Target, Target, Size, Sign),

    LEA(Target, u8),
    JSR(Target),
    JMP(Target),
}


const OPCG_BIT_OPS: u8 = 0x0;
const OPCG_MOVE_BYTE: u8 = 0x1;
const OPCG_MOVE_WORD: u8 = 0x2;
const OPCG_MOVE_LONG: u8 = 0x3;
const OPCG_MISC: u8 = 0x04;
const OPCG_ADDQ_SUBQ: u8 = 0x5;
const OPCG_BRANCH: u8 = 0x6;
const OPCG_MOVEQ: u8 = 0x7;
const OPCG_DIV_OR: u8 = 0x8;
const OPCG_SUB: u8 = 0x9;
const OPCG_RESERVED1: u8 = 0xA;
const OPCG_CMP_EOR: u8 = 0xB;
const OPCG_MUL_EXCH: u8 = 0xC;
const OPCG_ADD: u8 = 0xD;
const OPCG_SHIFT: u8 = 0xE;
const OPCG_RESERVED2: u8 = 0xF;


impl MC68010 {
    pub fn new() -> MC68010 {
        MC68010 {
            state: State::Init,
            pc: 0,
            msp: 0,
            usp: 0,
            flags: FLAGS_ON_RESET,
            d_reg: [0; 8],
            a_reg: [0; 8],
            vbr: 0,
        }
    }

    pub fn reset(&mut self) {
        self.state = State::Init;
        self.pc = 0;
        self.msp = 0;
        self.usp = 0;
        self.flags = FLAGS_ON_RESET;
        self.d_reg = [0; 8];
        self.a_reg = [0; 8];
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
        self.flags & FLAGS_SUPERVISOR != 0
    }

    fn read_instruction_word(&mut self, space: &mut AddressSpace) -> Result<u16, Error> {
        let word = space.read_beu16(self.pc as Address)?;
        println!("{:08x} {:04x?}", self.pc, word);
        self.pc += 2;
        Ok(word)
    }

    fn read_instruction_long(&mut self, space: &mut AddressSpace) -> Result<u32, Error> {
        let word = space.read_beu32(self.pc as Address)?;
        println!("{:08x} {:08x?}", self.pc, word);
        self.pc += 4;
        Ok(word)
    }

    fn push_long(&mut self, space: &mut AddressSpace, value: u32) -> Result<(), Error> {
        let reg = if self.is_supervisor() { &mut self.msp } else { &mut self.usp };
        *reg -= 4;
        space.write_beu32(*reg as Address, value)
    }

    fn execute_one(&mut self, space: &mut AddressSpace) -> Result<(), Error> {
        let ins = self.decode_one(space)?;

        match ins {
            Instruction::JSR(target) => {
                self.push_long(space, self.pc)?;
                self.pc = self.get_target_value(space, target)?;
            },
            _ => panic!(""),
        }

        Ok(())
    }

    fn get_target_value(&mut self, space: &mut AddressSpace, target: Target) -> Result<u32, Error> {
        match target {
            Target::Immediate(value) => Ok(value),
            Target::DirectDReg(reg) => Ok(self.d_reg[reg as usize]),
            _ => Err(Error::new(&format!("Unimplemented addressing target: {:?}", target))),
        }
    }

    fn decode_one(&mut self, space: &mut AddressSpace) -> Result<Instruction, Error> {
        let ins = self.read_instruction_word(space)?;

        match ((ins & 0xF000) >> 12) as u8 {
            OPCG_BIT_OPS => {
panic!("");
            },
            OPCG_MOVE_BYTE => {
                let data = self.read_instruction_word(space)?;

panic!("");
            },
            OPCG_MOVE_WORD => {
                let data = self.read_instruction_word(space)?;

panic!("");
            },
            OPCG_MOVE_LONG => {
                let data = self.read_instruction_long(space)?;

panic!("");
            },
            OPCG_MISC => {
                if (ins & 0b111000000) == 0b111000000 {
                    // LEA Instruction

                    debug!("LEA");
                    let src = self.decode_lower_effective_address(space, ins)?;
                    let dest = get_high_reg(ins);
                    Ok(Instruction::LEA(src, dest))

                } else if (ins & 0b101000000) == 0b100000000 {
                    // CHK Instruction
panic!("");
                } else if (ins & 0b101110000000) == 0b100010000000 {
                    // MOVEM Instruction
panic!("");
                } else if (ins & 0b111110000000) == 0b111010000000 {
                    // JMP/JSR Instruction
                    let target = self.decode_lower_effective_address(space, ins)?;
                    if (ins & 0b01000000) == 0 {
                        Ok(Instruction::JSR(target))
                    } else {
                        Ok(Instruction::JMP(target))
                    }

                } else {

panic!("");
                }
            },
            OPCG_ADDQ_SUBQ => {

panic!("");
            },
            OPCG_BRANCH => {

panic!("");
            },
            OPCG_MOVEQ => {

panic!("");
            },
            OPCG_DIV_OR => {

panic!("");
            },
            OPCG_SUB => {

panic!("");
            },
            OPCG_CMP_EOR => {

panic!("");
            },
            OPCG_MUL_EXCH => {

panic!("");
            },
            OPCG_ADD => {

panic!("");
            },
            OPCG_SHIFT => {

panic!("");
            },
            _ => return Err(Error::processor(ERR_ILLEGAL_INSTRUCTION)),
        }
    }

    fn decode_lower_effective_address(&mut self, space: &mut AddressSpace, ins: u16) -> Result<Target, Error> {
        let reg = get_low_reg(ins);
        let mode = get_mode(ins);
        self.get_mode_as_target(space, mode, reg)
    }

    fn get_mode_as_target(&mut self, space: &mut AddressSpace, mode: u8, reg: u8) -> Result<Target, Error> {
        let value = match mode {
            0b010 => Target::IndirectAReg(reg),
            0b101 => {
                let d16 = self.read_instruction_word(space)?;
                Target::IndirectARegOffset(reg, d16)
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
                        let d16 = self.read_instruction_word(space)?;
                        Target::IndirectPCOffset(d16)
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
    ((ins & 0x0D00) >> 9) as u8
}

#[inline(always)]
fn get_low_reg(ins: u16) -> u8 {
    (ins & 0x0007) as u8
}

#[inline(always)]
fn get_mode(ins: u16) -> u8 {
    ((ins & 0x0038) >> 3) as u8
}

/*
impl Processor for MC68010 {

}
*/
