
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
    Carry       = 0x01,
    AddSubtract = 0x02,
    Parity      = 0x04,
    HalfCarry   = 0x10,
    Zero        = 0x40,
    Sign        = 0x80,
}

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
pub enum Register {
    B = 0,
    C = 1,
    D = 2,
    E = 3,
    H = 4,
    L = 5,
    A = 6,
    F = 7,
}


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

