
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
    pub interrupts_enabled: bool,

    pub pc: u16,
    pub sp: u16,
    pub ix: u16,
    pub iy: u16,

    pub reg: [u8; 8],
    pub shadow_reg: [u8; 8],

    pub i: u8,
    pub r: u8,
}

impl Z80State {
    pub fn new() -> Self {
        Self {
            status: Status::Init,
            interrupts_enabled: false,

            pc: 0,
            sp: 0,
            ix: 0,
            iy: 0,

            reg: [0; 8],
            shadow_reg: [0; 8],

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

    pub fn dump_state(&mut self, system: &System) {
        println!("Status: {:?}", self.state.status);
        println!("PC: {:#06x}", self.state.pc);
        println!("SP: {:#06x}", self.state.sp);
        println!("IX: {:#06x}", self.state.ix);
        println!("IY: {:#06x}", self.state.iy);

        println!("A: {:#04x}        F:  {:#04x}", self.state.reg[Register::A as usize], self.state.reg[Register::F as usize]);
        println!("B: {:#04x}        C:  {:#04x}", self.state.reg[Register::B as usize], self.state.reg[Register::C as usize]);
        println!("D: {:#04x}        E:  {:#04x}", self.state.reg[Register::D as usize], self.state.reg[Register::B as usize]);
        println!("H: {:#04x}        L:  {:#04x}", self.state.reg[Register::H as usize], self.state.reg[Register::L as usize]);

        println!("Current Instruction: {} {:?}", self.decoder.format_instruction_bytes(&mut self.port), self.decoder.instruction);
        println!("");
        self.port.dump_memory(self.state.sp as Address, 0x40);
        println!("");
    }
}

