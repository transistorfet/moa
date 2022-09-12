
use crate::devices::Address;
use crate::timers::CpuTimer;
use crate::memory::BusPort;

use super::instructions::Size;
use super::decode::M68kDecoder;
use super::debugger::M68kDebugger;
use super::timing::M68kInstructionTiming;

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
#[derive(Copy, Clone, Debug, PartialEq)]
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
#[derive(Copy, Clone, Debug, PartialEq)]
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

#[repr(u8)]
#[allow(dead_code)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum FunctionCode {
    Reserved0           = 0,
    UserData            = 1,
    UserProgram         = 2,
    Reserved3           = 3,
    Reserved4           = 4,
    SupervisorData      = 5,
    SupervisorProgram   = 6,
    CpuSpace            = 7,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Status {
    Init,
    Running,
    Stopped,
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq)]
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

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MemType {
    Program,
    Data,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MemAccess {
    Read,
    Write,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MemoryRequest {
    pub access: MemAccess,
    pub code: FunctionCode,
    pub size: Size,
    pub address: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct M68kState {
    pub status: Status,
    pub request: MemoryRequest,
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
    pub frequency: u32,
    pub state: M68kState,
    pub decoder: M68kDecoder,
    pub timing: M68kInstructionTiming,
    pub debugger: M68kDebugger,
    pub port: BusPort,
    pub timer: CpuTimer,
}

impl M68kState {
    pub fn new() -> M68kState {
        M68kState {
            status: Status::Init,
            request: MemoryRequest::new(),
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
    pub fn new(cputype: M68kType, frequency: u32, port: BusPort) -> M68k {
        M68k {
            cputype,
            frequency,
            state: M68kState::new(),
            decoder: M68kDecoder::new(cputype, 0),
            timing: M68kInstructionTiming::new(cputype, port.data_width()),
            debugger: M68kDebugger::new(),
            port: port,
            timer: CpuTimer::new(),
        }
    }

    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.state = M68kState::new();
        self.decoder = M68kDecoder::new(self.cputype, 0);
        self.timing = M68kInstructionTiming::new(self.cputype, self.port.data_width());
        self.debugger = M68kDebugger::new();
    }

    pub fn dump_state(&mut self) {
        println!("Status: {:?}", self.state.status);
        println!("PC: {:#010x}", self.state.pc);
        println!("SR: {:#06x}", self.state.sr);
        for i in 0..7 {
            println!("D{}: {:#010x}        A{}:  {:#010x}", i, self.state.d_reg[i as usize], i, self.state.a_reg[i as usize]);
        }
        println!("D7: {:#010x}", self.state.d_reg[7]);
        println!("SSP: {:#010x}", self.state.ssp);
        println!("USP: {:#010x}", self.state.usp);

        println!("Current Instruction: {:#010x} {:?}", self.decoder.start, self.decoder.instruction);
        println!("");
        self.port.dump_memory(self.state.ssp as Address, 0x40);
        println!("");
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

impl FunctionCode {
    pub fn program(sr: u16) -> Self {
        if sr & Flags::Supervisor as u16 != 0 {
            FunctionCode::SupervisorProgram
        } else {
            FunctionCode::UserProgram
        }
    }

    pub fn data(sr: u16) -> Self {
        if sr & Flags::Supervisor as u16 != 0 {
            FunctionCode::SupervisorData
        } else {
            FunctionCode::UserData
        }
    }
}

impl MemoryRequest {
    pub fn new() -> Self {
        Self {
            access: MemAccess::Read,
            code: FunctionCode::Reserved0,
            size: Size::Word,
            address: 0,
        }
    }

    pub fn get_type_code(&self) -> u16 {
        let ins = match self.code {
            // TODO this is wrong (should be opposite results), but is a temporary hack
            FunctionCode::SupervisorProgram | FunctionCode::UserProgram => 0x0008,
            _ => 0x0000,
        };

        let rw = match self.access {
            MemAccess::Write => 0x0000,
            MemAccess::Read => 0x0010,
        };

        ins | rw | (self.code as u16)
    }
}

