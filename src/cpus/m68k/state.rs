
use crate::timers::CpuTimer;
use crate::memory::{Address};
use crate::system::{System};

use super::decode::M68kDecoder;
use super::debugger::M68kDebugger;

const FLAGS_ON_RESET: u16 = 0x2700;

#[repr(u16)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Flags {
    Carry       = 0x0001,
    Overflow    = 0x0002,
    Zero        = 0x0004,
    Negative    = 0x0008,
    Extend      = 0x0010,
    Supervisor  = 0x2000,
    Tracing     = 0x8000,
}

pub const ERR_BUS_ERROR: u32 = 2;
pub const ERR_ADDRESS_ERROR: u32 = 3;
pub const ERR_ILLEGAL_INSTRUCTION: u32 = 4;


#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Status {
    Init,
    Running,
    PendingExecption(u8),
    Stopped,
    Halted,
}

pub struct MC68010State {
    pub status: Status,

    pub pc: u32,
    pub sr: u16,
    pub d_reg: [u32; 8],
    pub a_reg: [u32; 7],
    pub msp: u32,
    pub usp: u32,

    pub vbr: u32,
}

impl MC68010State {
    pub fn new() -> MC68010State {
        MC68010State {
            status: Status::Init,

            pc: 0,
            sr: FLAGS_ON_RESET,
            d_reg: [0; 8],
            a_reg: [0; 7],
            msp: 0,
            usp: 0,

            vbr: 0,
        }
    }
}

pub struct MC68010 {
    pub state: MC68010State,
    pub decoder: M68kDecoder,
    pub debugger: M68kDebugger,
    pub timer: CpuTimer,
}

impl MC68010 {
    pub fn new() -> MC68010 {
        MC68010 {
            state: MC68010State::new(),
            decoder: M68kDecoder::new(0, 0),
            debugger: M68kDebugger::new(),
            timer: CpuTimer::new(),
        }
    }

    pub fn reset(&mut self) {
        self.state = MC68010State::new();
        self.decoder = M68kDecoder::new(0, 0);
        self.debugger = M68kDebugger::new();
    }

    pub fn dump_state(&self, system: &System) {
        println!("Status: {:?}", self.state.status);
        println!("PC: {:#010x}", self.state.pc);
        println!("SR: {:#06x}", self.state.sr);
        for i in 0..7 {
            println!("D{}: {:#010x}        A{}:  {:#010x}", i, self.state.d_reg[i as usize], i, self.state.a_reg[i as usize]);
        }
        println!("D7: {:#010x}", self.state.d_reg[7]);
        println!("MSP: {:#010x}", self.state.msp);
        println!("USP: {:#010x}", self.state.usp);

        println!("Current Instruction: {:#010x} {:?}", self.decoder.start, self.decoder.instruction);
        println!("");
        system.get_bus().dump_memory(self.state.msp as Address, 0x40);
        println!("");
    }
}

