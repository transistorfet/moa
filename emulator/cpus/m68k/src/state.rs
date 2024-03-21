// m68k CPU State

use femtos::Frequency;
use core::fmt::{self, Write};
use emulator_hal::time;

use crate::{M68kDebugger, M68kCycle};
use crate::instructions::Target;


pub type ClockCycles = u16;


#[allow(dead_code)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum AddressWidth {
    A32 = 32, // MC68020+
    A24 = 24, // MC68000 64-Pin, MC68010
    A22 = 22, // MC68008 52-Pin
    A20 = 20, // MC68008 48-Pin
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum DataWidth {
    D32 = 32,
    D16 = 16,
    D8 = 8,
}

/// The instruction set of the chip
#[allow(dead_code)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CoreType {
    MC68000,
    MC68010,
    MC68020,
    MC68030,
}

/// Complete collection of information about the CPU being simulated
#[allow(dead_code)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CpuInfo {
    pub chip: M68kType,
    pub core_type: CoreType,
    pub address_width: AddressWidth,
    pub data_width: DataWidth,
    pub frequency: Frequency,
}

/// The variant of the 68k family of CPUs that is being emulated
///
/// This can be used as a shorthand for creating a CpuInfo that
/// can be used by the simuation code to determine behaviour
#[allow(dead_code)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum M68kType {
    MC68000,
    MC68008,
    MC68010,
    MC68020,
    MC68030,
}

impl From<M68kType> for CoreType {
    fn from(cputype: M68kType) -> Self {
        match cputype {
            M68kType::MC68000 => CoreType::MC68000,
            M68kType::MC68008 => CoreType::MC68000,
            M68kType::MC68010 => CoreType::MC68010,
            M68kType::MC68020 => CoreType::MC68020,
            M68kType::MC68030 => CoreType::MC68030,
        }
    }
}

impl CpuInfo {
    pub fn from_type(cputype: M68kType, frequency: Frequency) -> Self {
        match cputype {
            M68kType::MC68008 => Self {
                chip: cputype,
                core_type: cputype.into(),
                address_width: AddressWidth::A22,
                data_width: DataWidth::D8,
                frequency,
            },
            M68kType::MC68000 | M68kType::MC68010 => Self {
                chip: cputype,
                core_type: cputype.into(),
                address_width: AddressWidth::A24,
                data_width: DataWidth::D16,
                frequency,
            },
            M68kType::MC68020 | M68kType::MC68030 => Self {
                chip: cputype,
                core_type: cputype.into(),
                address_width: AddressWidth::A32,
                data_width: DataWidth::D32,
                frequency,
            },
        }
    }
}

const FLAGS_ON_RESET: u16 = 0x2700;

#[repr(u16)]
#[allow(dead_code)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[rustfmt::skip]
pub enum Flags {
    Carry       = 0x0001,
    Overflow    = 0x0002,
    Zero        = 0x0004,
    Negative    = 0x0008,
    Extend      = 0x0010,
    IntMask     = 0x0700,
    Interrupt   = 0x1000,
    Supervisor  = 0x2000,
    Tracing     = 0x8000,
}

#[repr(u8)]
#[allow(dead_code)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[rustfmt::skip]
pub enum Exceptions {
    BusError            = 2,
    AddressError        = 3,
    IllegalInstruction  = 4,
    ZeroDivide          = 5,
    ChkInstruction      = 6,
    TrapvInstruction    = 7,
    PrivilegeViolation  = 8,
    Trace               = 9,
    LineAEmulator       = 10,
    LineFEmulator       = 11,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Init,
    Running,
    Stopped,
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum InterruptPriority {
    NoInterrupt = 0,
    Level1 = 1,
    Level2 = 2,
    Level3 = 3,
    Level4 = 4,
    Level5 = 5,
    Level6 = 6,
    Level7 = 7,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct M68kState {
    pub status: Status,
    pub current_ipl: InterruptPriority,
    pub pending_ipl: InterruptPriority,

    pub pc: u32,
    pub sr: u16,
    pub d_reg: [u32; 8],
    pub a_reg: [u32; 7],
    pub ssp: u32,
    pub usp: u32,

    pub vbr: u32,
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum M68kError<BusError> {
    #[error("cpu halted")]
    Halted,
    #[error("processor exception {0:?}")]
    Exception(Exceptions),
    #[error("interrupt vector {0} occurred")]
    Interrupt(u8),
    #[error("breakpoint reached")]
    Breakpoint,
    #[error("invalid instruction target, direct value used as a pointer: {0:?}")]
    InvalidTarget(Target),
    #[error("bus error")]
    BusError(BusError),
    #[error("error: {0}")]
    Other(String),
}

#[derive(Clone, Default)]
pub struct M68kStatistics {
    pub cycle_number: usize,
}

#[derive(Clone)]
pub struct M68k<Instant> {
    pub info: CpuInfo,
    pub state: M68kState,
    pub debugger: M68kDebugger,
    pub stats: M68kStatistics,
    pub cycle: Option<M68kCycle<Instant>>,
}

impl Default for M68kState {
    fn default() -> M68kState {
        M68kState {
            status: Status::Init,
            current_ipl: InterruptPriority::NoInterrupt,
            pending_ipl: InterruptPriority::NoInterrupt,

            pc: 0,
            sr: FLAGS_ON_RESET,
            d_reg: [0; 8],
            a_reg: [0; 7],
            ssp: 0,
            usp: 0,

            vbr: 0,
        }
    }
}

impl M68kState {
    pub fn dump_state<W: Write>(&self, writer: &mut W) -> Result<(), fmt::Error> {
        writeln!(writer, "Status: {:?}", self.status)?;
        writeln!(writer, "PC: {:#010x}", self.pc)?;
        writeln!(writer, "SR: {:#06x}", self.sr)?;
        for i in 0..7 {
            writeln!(writer, "D{}: {:#010x}        A{}: {:#010x}", i, self.d_reg[i as usize], i, self.a_reg[i as usize])?;
        }
        writeln!(writer, "D7: {:#010x}       USP: {:#010x}", self.d_reg[7], self.usp)?;
        writeln!(writer, "                     SSP: {:#010x}", self.ssp)?;
        Ok(())
    }
}

impl<Instant> M68k<Instant>
where
    Instant: time::Instant,
{
    pub fn new(info: CpuInfo) -> Self {
        M68k {
            info,
            state: M68kState::default(),
            debugger: M68kDebugger::default(),
            stats: Default::default(),
            cycle: None,
        }
    }

    pub fn from_type(cputype: M68kType, freq: Frequency) -> Self {
        Self::new(CpuInfo::from_type(cputype, freq))
    }

    pub fn dump_state<W: Write>(&self, writer: &mut W) -> Result<(), fmt::Error> {
        self.state.dump_state(writer)?;

        if let Some(cycle) = self.cycle.as_ref() {
            writeln!(writer, "Current Instruction: {:#010x} {:?}", cycle.decoder.start, cycle.decoder.instruction)?;
            writeln!(writer)?;
        }
        //memory::dump_memory(&mut self.bus, self.cycle.current_clock, self.state.ssp, 0x40);
        writeln!(writer)?;
        Ok(())
    }

    #[inline]
    pub fn last_cycle_duration(&self) -> Instant::Duration {
        let clocks = self.cycle.as_ref().map(|cycle| cycle.timing.calculate_clocks()).unwrap_or(4);
        //self.info.frequency.period_duration() * clocks as u64
        Instant::hertz_to_duration(self.info.frequency.as_hz() as u64) * clocks as u32
    }
}

impl InterruptPriority {
    pub fn from_u8(priority: u8) -> InterruptPriority {
        match priority {
            0 => InterruptPriority::NoInterrupt,
            1 => InterruptPriority::Level1,
            2 => InterruptPriority::Level2,
            3 => InterruptPriority::Level3,
            4 => InterruptPriority::Level4,
            5 => InterruptPriority::Level5,
            6 => InterruptPriority::Level6,
            _ => InterruptPriority::Level7,
        }
    }
}
