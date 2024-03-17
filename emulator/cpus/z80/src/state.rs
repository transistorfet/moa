use std::rc::Rc;
use std::cell::RefCell;
use femtos::{Instant, Frequency};

use moa_core::{Address, Bus, BusPort};
use moa_signals::Signal;

use crate::decode::Z80Decoder;
use crate::debugger::Z80Debugger;
use crate::execute::Z80Executor;
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

#[derive(Clone, Debug, thiserror::Error)]
pub enum Z80Error /* <B: fmt::Display> */ {
    #[error("cpu halted")]
    Halted,
    #[error("breakpoint reached")]
    Breakpoint,
    #[error("unimplemented instruction {0:?}")]
    Unimplemented(Instruction),
    #[error("bus error: {0}")]
    BusError(String /* B */),
}

#[derive(Clone)]
pub struct Z80 {
    pub cputype: Z80Type,
    pub frequency: Frequency,
    pub state: Z80State,
    pub decoder: Z80Decoder,
    pub debugger: Z80Debugger,
    pub executor: Z80Executor,
    pub port: BusPort,
    pub ioport: Option<BusPort>,
    pub reset: Signal<bool>,
    pub bus_request: Signal<bool>,
}

impl Z80 {
    pub fn new(cputype: Z80Type, frequency: Frequency, port: BusPort, ioport: Option<BusPort>) -> Self {
        Self {
            cputype,
            frequency,
            state: Z80State::default(),
            decoder: Z80Decoder::default(),
            debugger: Z80Debugger::default(),
            executor: Z80Executor::at_time(Instant::START),
            port,
            ioport,
            reset: Signal::new(false),
            bus_request: Signal::new(false),
        }
    }

    pub fn from_type(
        cputype: Z80Type,
        frequency: Frequency,
        bus: Rc<RefCell<Bus>>,
        addr_offset: Address,
        io_bus: Option<(Rc<RefCell<Bus>>, Address)>,
    ) -> Self {
        match cputype {
            Z80Type::Z80 => Self::new(
                cputype,
                frequency,
                BusPort::new(addr_offset, 16, 8, bus),
                io_bus.map(|(io_bus, io_offset)| BusPort::new(io_offset, 16, 8, io_bus)),
            ),
        }
    }

    #[allow(dead_code)]
    pub fn clear_state(&mut self) {
        self.state = Z80State::default();
        self.decoder = Z80Decoder::default();
        self.debugger = Z80Debugger::default();
        self.executor = Z80Executor::at_time(Instant::START);
    }

    pub fn dump_state(&mut self, clock: Instant) {
        println!("Status: {:?}", self.state.status);
        println!("PC: {:#06x}", self.state.pc);
        println!("SP: {:#06x}", self.state.sp);
        println!("IX: {:#06x}", self.state.ix);
        println!("IY: {:#06x}", self.state.iy);

        println!(
            "A: {:#04x}    F:  {:#04x}           A': {:#04x}    F':  {:#04x}",
            self.state.reg[Register::A as usize],
            self.state.reg[Register::F as usize],
            self.state.shadow_reg[Register::A as usize],
            self.state.shadow_reg[Register::F as usize]
        );
        println!(
            "B: {:#04x}    C:  {:#04x}           B': {:#04x}    C':  {:#04x}",
            self.state.reg[Register::B as usize],
            self.state.reg[Register::C as usize],
            self.state.shadow_reg[Register::B as usize],
            self.state.shadow_reg[Register::C as usize]
        );
        println!(
            "D: {:#04x}    E:  {:#04x}           D': {:#04x}    E':  {:#04x}",
            self.state.reg[Register::D as usize],
            self.state.reg[Register::E as usize],
            self.state.shadow_reg[Register::D as usize],
            self.state.shadow_reg[Register::E as usize]
        );
        println!(
            "H: {:#04x}    L:  {:#04x}           H': {:#04x}    L':  {:#04x}",
            self.state.reg[Register::H as usize],
            self.state.reg[Register::L as usize],
            self.state.shadow_reg[Register::H as usize],
            self.state.shadow_reg[Register::L as usize]
        );

        println!("I: {:#04x}    R:  {:#04x}", self.state.i, self.state.r);
        println!("IM: {:?}  IFF1: {:?}  IFF2: {:?}", self.state.im, self.state.iff1, self.state.iff2);

        println!(
            "Current Instruction: {} {:?}",
            self.decoder.format_instruction_bytes(&mut self.port),
            self.decoder.instruction
        );
        println!();
        self.port.dump_memory(clock, self.state.sp as Address, 0x40);
        println!();
    }
}
