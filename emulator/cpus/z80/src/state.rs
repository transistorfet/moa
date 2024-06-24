use core::fmt::{self, Write};
use femtos::Frequency;
use emulator_hal::{Instant as EmuInstant, BusAccess};

use moa_signals::Signal;

use crate::debugger::Z80Debugger;
use crate::execute::Z80Cycle;
use crate::instructions::{Instruction, Register, InterruptMode};


#[allow(dead_code)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Z80Type {
    Z80,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Init,
    Running,
    Halted,
}

#[repr(u8)]
#[allow(dead_code)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[rustfmt::skip]
pub enum Flags {
    Carry       = 0x01,
    AddSubtract = 0x02,
    Parity      = 0x04,
    F3          = 0x08,
    HalfCarry   = 0x10,
    F5          = 0x20,
    Zero        = 0x40,
    Sign        = 0x80,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Z80State {
    pub status: Status,

    pub pc: u16,
    pub sp: u16,
    pub ix: u16,
    pub iy: u16,

    pub reg: [u8; 8],
    pub shadow_reg: [u8; 8],

    pub i: u8,
    pub r: u8,

    pub iff1: bool,
    pub iff2: bool,
    pub im: InterruptMode,
}

impl Default for Z80State {
    fn default() -> Self {
        Self {
            status: Status::Init,

            pc: 0,
            sp: 0,
            ix: 0,
            iy: 0,

            reg: [0; 8],
            shadow_reg: [0; 8],

            i: 0,
            r: 0,

            iff1: false,
            iff2: false,
            im: InterruptMode::Mode0,
        }
    }
}

impl Z80State {
    pub fn get_register(&mut self, reg: Register) -> u8 {
        self.reg[reg as usize]
    }

    pub fn set_register(&mut self, reg: Register, value: u8) {
        self.reg[reg as usize] = value;
    }
}

#[derive(Clone, Debug, Default)]
pub struct Z80Signals {
    //pub reset: bool,
    //pub bus_request: bool,
    pub reset: Signal<bool>,
    pub bus_request: Signal<bool>,
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum Z80Error /* <B: fmt::Display> */ {
    #[error("cpu halted")]
    Halted,
    #[error("breakpoint reached")]
    Breakpoint,
    #[error("unimplemented instruction {0:?}")]
    Unimplemented(Instruction),
    #[error("unexpected instruction {0:?}")]
    UnexpectedInstruction(Instruction),
    #[error("bus error: {0}")]
    BusError(String /* B */),
    #[error("{0}")]
    Other(String),
}


pub type Z80Address = u16;
pub type Z80IOAddress = u16;

#[derive(Copy, Clone, Debug)]
pub enum Z80AddressSpace {
    Memory(Z80Address),
    IO(Z80IOAddress),
}

#[derive(Clone)]
pub struct Z80<Instant> {
    pub cputype: Z80Type,
    pub frequency: Frequency,
    pub state: Z80State,
    pub debugger: Z80Debugger,
    pub previous_cycle: Z80Cycle<Instant>,
    pub signals: Z80Signals,
}

impl<Instant> Z80<Instant>
where
    Instant: EmuInstant,
{
    pub fn new(cputype: Z80Type, frequency: Frequency) -> Self {
        Self {
            cputype,
            frequency,
            state: Z80State::default(),
            debugger: Z80Debugger::default(),
            previous_cycle: Z80Cycle::at_time(Instant::START),
            signals: Z80Signals::default(),
        }
    }

    pub fn from_type(cputype: Z80Type, frequency: Frequency) -> Self {
        match cputype {
            Z80Type::Z80 => Self::new(cputype, frequency),
        }
    }

    #[allow(dead_code)]
    pub fn clear_state(&mut self) {
        self.state = Z80State::default();
        self.debugger = Z80Debugger::default();
    }

    pub fn dump_state<W, Bus>(&mut self, writer: &mut W, _clock: Instant, bus: &mut Bus) -> Result<(), fmt::Error>
    where
        W: Write,
        Bus: BusAccess<Z80AddressSpace, Instant = Instant>,
    {
        writeln!(writer, "Status: {:?}", self.state.status)?;
        writeln!(writer, "PC: {:#06x}", self.state.pc)?;
        writeln!(writer, "SP: {:#06x}", self.state.sp)?;
        writeln!(writer, "IX: {:#06x}", self.state.ix)?;
        writeln!(writer, "IY: {:#06x}", self.state.iy)?;

        writeln!(
            writer,
            "A: {:#04x}    F:  {:#04x}           A': {:#04x}    F':  {:#04x}",
            self.state.reg[Register::A as usize],
            self.state.reg[Register::F as usize],
            self.state.shadow_reg[Register::A as usize],
            self.state.shadow_reg[Register::F as usize]
        )?;
        writeln!(
            writer,
            "B: {:#04x}    C:  {:#04x}           B': {:#04x}    C':  {:#04x}",
            self.state.reg[Register::B as usize],
            self.state.reg[Register::C as usize],
            self.state.shadow_reg[Register::B as usize],
            self.state.shadow_reg[Register::C as usize]
        )?;
        writeln!(
            writer,
            "D: {:#04x}    E:  {:#04x}           D': {:#04x}    E':  {:#04x}",
            self.state.reg[Register::D as usize],
            self.state.reg[Register::E as usize],
            self.state.shadow_reg[Register::D as usize],
            self.state.shadow_reg[Register::E as usize]
        )?;
        writeln!(
            writer,
            "H: {:#04x}    L:  {:#04x}           H': {:#04x}    L':  {:#04x}",
            self.state.reg[Register::H as usize],
            self.state.reg[Register::L as usize],
            self.state.shadow_reg[Register::H as usize],
            self.state.shadow_reg[Register::L as usize]
        )?;

        writeln!(writer, "I: {:#04x}    R:  {:#04x}", self.state.i, self.state.r)?;
        writeln!(writer, "IM: {:?}  IFF1: {:?}  IFF2: {:?}", self.state.im, self.state.iff1, self.state.iff2)?;

        writeln!(
            writer,
            "Current Instruction: {} {:?}",
            self.previous_cycle.decoder.format_instruction_bytes(bus),
            self.previous_cycle.decoder.instruction
        )?;
        writeln!(writer, "Previous Instruction: {:?}", self.previous_cycle.decoder.instruction)?;
        writeln!(writer)?;
        // TODO disabled until function is reimplemented
        //self.port.dump_memory(clock, self.state.sp as Address, 0x40);
        writeln!(writer)?;
        Ok(())
    }
}
