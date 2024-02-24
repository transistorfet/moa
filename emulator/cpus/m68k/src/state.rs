
use std::rc::Rc;
use std::cell::RefCell;
use femtos::{Instant, Frequency};

use moa_core::{Address, Bus, BusPort};

use crate::decode::M68kDecoder;
use crate::debugger::M68kDebugger;
use crate::memory::M68kBusPort;
use crate::timing::M68kInstructionTiming;


pub type ClockCycles = u16;

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum M68kType {
    MC68000,
    MC68010,
    MC68020,
    MC68030,
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

#[derive(Clone)]
pub struct M68k {
    pub cputype: M68kType,
    pub frequency: Frequency,
    pub state: M68kState,
    pub decoder: M68kDecoder,
    pub timing: M68kInstructionTiming,
    pub debugger: M68kDebugger,
    pub port: M68kBusPort,
    pub current_clock: Instant,
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
    pub fn new(cputype: M68kType, frequency: Frequency, port: BusPort) -> M68k {
        M68k {
            cputype,
            frequency,
            state: M68kState::default(),
            decoder: M68kDecoder::new(cputype, true, 0),
            timing: M68kInstructionTiming::new(cputype, port.data_width()),
            debugger: M68kDebugger::default(),
            port: M68kBusPort::new(port),
            current_clock: Instant::START,
        }
    }

    pub fn from_type(cputype: M68kType, frequency: Frequency, bus: Rc<RefCell<Bus>>, addr_offset: Address) -> Self {
        match cputype {
            M68kType::MC68000 |
            M68kType::MC68010 => Self::new(cputype, frequency, BusPort::new(addr_offset, 24, 16, bus)),
            M68kType::MC68020 |
            M68kType::MC68030 => Self::new(cputype, frequency, BusPort::new(addr_offset, 32, 32, bus)),
        }
    }

    pub fn dump_state(&mut self) {
        println!("Status: {:?}", self.state.status);
        println!("PC: {:#010x}", self.state.pc);
        println!("SR: {:#06x}", self.state.sr);
        for i in 0..7 {
            println!("D{}: {:#010x}        A{}: {:#010x}", i, self.state.d_reg[i as usize], i, self.state.a_reg[i as usize]);
        }
        println!("D7: {:#010x}       USP: {:#010x}", self.state.d_reg[7], self.state.usp);
        println!("                     SSP: {:#010x}", self.state.ssp);

        println!("Current Instruction: {:#010x} {:?}", self.decoder.start, self.decoder.instruction);
        println!();
        self.port.dump_memory(self.state.ssp, 0x40);
        println!();
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

