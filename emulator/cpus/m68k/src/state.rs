
use std::rc::Rc;
use std::cell::RefCell;
use femtos::{Instant, Frequency};

use moa_core::{Address, Bus, BusPort};

use crate::debugger::M68kDebugger;
use crate::memory::M68kBusPort;
use crate::instructions::Target;
use crate::execute::M68kCycle;


pub type ClockCycles = u16;


#[allow(dead_code)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum AddressWidth {
    A32 = 32,    // MC68020+
    A24 = 24,    // MC68000 64-Pin, MC68010
    A22 = 22,    // MC68008 52-Pin
    A20 = 20,    // MC68008 48-Pin
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
    fn from(cputype: M68kType, frequency: Frequency) -> Self {
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
            }
        }
    }
}

const FLAGS_ON_RESET: u16 = 0x2700;

#[repr(u16)]
#[allow(dead_code)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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
pub enum M68kError {
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
    #[error("error: {0}")]
    Other(String),
}

#[derive(Clone)]
pub struct M68k {
    pub info: CpuInfo,
    pub state: M68kState,
    pub debugger: M68kDebugger,
    pub port: BusPort,
    pub cycle: Option<M68kCycle>,
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

impl M68k {
    pub fn new(info: CpuInfo, port: BusPort) -> M68k {
        M68k {
            info,
            state: M68kState::default(),
            debugger: M68kDebugger::default(),
            port,
            cycle: None,
        }
    }

    pub fn from_type(cputype: M68kType, frequency: Frequency, bus: Rc<RefCell<Bus>>, addr_offset: Address) -> Self {
        let info = CpuInfo::from(cputype, frequency);
        Self::new(info, BusPort::new(addr_offset, info.address_width as u8, info.data_width as u8, bus))
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

