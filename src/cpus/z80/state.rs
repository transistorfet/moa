
use crate::system::System;
use crate::devices::Address;
use crate::memory::BusPort;

use super::decode::Z80Decoder;
//use super::debugger::M68kDebugger;


#[allow(dead_code)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Z80Type {
    Z80,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Status {
    Init,
    Running,
    Halted,
}


#[repr(u8)]
#[allow(dead_code)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Flags {
    Carry       = 0x0001,
    Overflow    = 0x0002,
    Zero        = 0x0004,
    Negative    = 0x0008,
    Extend      = 0x0010,
}

/*
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
*/

#[derive(Clone, Debug, PartialEq)]
pub struct Z80State {
    pub status: Status,

    pub pc: u16,
    pub sp: u16,
    pub ix: u16,
    pub iy: u16,

    pub reg: [u8; 8],
    pub alt_reg: [u8; 8],

    pub i: u8,
    pub r: u8,
}

impl Z80State {
    pub fn new() -> Self {
        Self {
            status: Status::Init,

            pc: 0,
            sp: 0,
            ix: 0,
            iy: 0,

            reg: [0; 8],
            alt_reg: [0; 8],

            i: 0,
            r: 0,
        }
    }
}

pub struct Z80 {
    pub cputype: Z80Type,
    pub frequency: u32,
    pub state: Z80State,
    pub decoder: Z80Decoder,
    //pub debugger: M68kDebugger,
    pub port: BusPort,
}

impl Z80 {
    pub fn new(cputype: Z80Type, frequency: u32, port: BusPort) -> Self {
        Self {
            cputype,
            frequency,
            state: Z80State::new(),
            decoder: Z80Decoder::new(),
            //debugger: M68kDebugger::new(),
            port: port,
        }
    }

    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.state = Z80State::new();
        //self.decoder = M68kDecoder::new(self.cputype, 0);
        //self.debugger = M68kDebugger::new();
    }

    /*
    pub fn dump_state(&mut self, system: &System) {
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
        self.port.dump_memory(self.state.msp as Address, 0x40);
        println!("");
    }
    */
}

