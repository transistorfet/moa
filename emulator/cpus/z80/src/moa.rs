use std::rc::Rc;
use std::cell::RefCell;
use femtos::{Instant, Duration};
use emulator_hal::{BusAdapter, NoBus, Instant as EmuInstant};

use moa_core::{System, Error, Bus, Address, Steppable, Interruptable, /* Signalable, Signal,*/ Debuggable, Transmutable};

use crate::{Z80, Z80Error, Z80Decoder};
use crate::instructions::Register;
use crate::emuhal::Z80Port;

pub struct MoaZ80<Instant>
where
    Instant: EmuInstant,
{
    pub bus: Rc<RefCell<Bus>>,
    pub cpu: Z80<Instant>,
}

impl Steppable for MoaZ80<Instant>
where
    Instant: EmuInstant,
{
    fn step(&mut self, system: &System) -> Result<Duration, Error> {
        let bus = &mut *self.bus.borrow_mut();
        let mut adapter = BusAdapter::<_, _, _, Z80Error>::new(bus, |addr| addr as u64);
        let mut io_bus = NoBus::new();
        let mut bus = Z80Port::new(&mut adapter, &mut io_bus);

        let mut executor = self.cpu.begin(system.clock, &mut bus)?;
        let clocks = executor.step_one()?;
        self.cpu.previous_cycle = executor.end();
        Ok(Instant::hertz_to_duration(self.cpu.frequency.as_hz() as u64) * clocks as u32)
    }

    fn on_error(&mut self, system: &System) {
        let bus = &mut *system.bus.borrow_mut();
        let mut adapter = BusAdapter::<_, _, _, Z80Error>::new(bus, |addr| addr as u64);
        let mut io_bus = NoBus::new();
        let mut bus = Z80Port::new(&mut adapter, &mut io_bus);
        let mut output = String::with_capacity(256);
        let _ = self.cpu.dump_state(&mut output, system.clock, &mut bus);
        println!("{}", output);
    }
}

impl Interruptable for MoaZ80<Instant> {}

/*
impl Signalable for Z80<Instant> {
    fn set_signal(&mut self, signal: Signal, flag: bool) -> Result<(), Error> {
        match signal {
            Signal::Reset => self.signals.reset = flag,
            Signal::BusRequest => self.signals.bus_request = flag,
        }
        Ok(())
    }

    fn signal(&mut self, signal: Signal) -> Option<bool> {
        match signal {
            Signal::Reset => Some(self.signals.reset),
            Signal::BusRequest => Some(self.signals.bus_request),
        }
    }
}
*/

impl Transmutable for MoaZ80<Instant> {
    fn as_steppable(&mut self) -> Option<&mut dyn Steppable> {
        Some(self)
    }

    fn as_interruptable(&mut self) -> Option<&mut dyn Interruptable> {
        Some(self)
    }

    fn as_debuggable(&mut self) -> Option<&mut dyn Debuggable> {
        Some(self)
    }

    //#[inline]
    //fn as_signalable(&mut self) -> Option<&mut dyn Signalable> {
    //    Some(self)
    //}
}

impl From<Z80Error> for Error {
    fn from(err: Z80Error) -> Self {
        match err {
            Z80Error::Halted => Self::Other("cpu halted".to_string()),
            Z80Error::Breakpoint => Self::Breakpoint("breakpoint".to_string()),
            Z80Error::Unimplemented(instruction) => Self::new(format!("unimplemented instruction {:?}", instruction)),
            Z80Error::UnexpectedInstruction(instruction) => Self::new(format!("unexpected instruction {:?}", instruction)),
            Z80Error::Other(msg) => Self::Other(msg),
            Z80Error::BusError(msg) => Self::Other(msg),
        }
    }
}

impl From<Error> for Z80Error {
    fn from(err: Error) -> Self {
        match err {
            Error::Processor(ex) => Z80Error::BusError(format!("processor error {}", ex)),
            Error::Breakpoint(_) => Z80Error::Breakpoint,
            Error::Other(msg) | Error::Assertion(msg) | Error::Emulator(_, msg) => Z80Error::BusError(msg),
        }
    }
}

impl Debuggable for MoaZ80<Instant> {
    fn add_breakpoint(&mut self, addr: Address) {
        self.cpu.debugger.breakpoints.push(addr as u16);
    }

    fn remove_breakpoint(&mut self, addr: Address) {
        if let Some(index) = self.cpu.debugger.breakpoints.iter().position(|a| *a == addr as u16) {
            self.cpu.debugger.breakpoints.remove(index);
        }
    }

    fn print_current_step(&mut self, system: &System) -> Result<(), Error> {
        let bus = &mut *system.bus.borrow_mut();
        let mut adapter = BusAdapter::<_, _, _, Z80Error>::new(bus, |addr| addr as u64);
        let mut io_bus = NoBus::new();
        let mut bus = Z80Port::new(&mut adapter, &mut io_bus);

        self.cpu.previous_cycle.decoder.dump_decoded(&mut bus);
        let mut output = String::with_capacity(256);
        let _ = self.cpu.dump_state(&mut output, system.clock, &mut bus);
        println!("{}", output);
        Ok(())
    }

    fn print_disassembly(&mut self, system: &System, addr: Address, count: usize) {
        let bus = &mut *system.bus.borrow_mut();
        let mut adapter = BusAdapter::<_, _, _, Z80Error>::new(bus, |addr| addr as u64);
        let mut io_bus = NoBus::new();
        let mut bus = Z80Port::new(&mut adapter, &mut io_bus);

        Z80Decoder::dump_disassembly(&mut bus, addr as u16, count as u16);
    }

    fn run_command(&mut self, _system: &System, args: &[&str]) -> Result<bool, Error> {
        match args[0] {
            "l" => self.cpu.state.reg[Register::L as usize] = 0x05,
            _ => {
                return Ok(true);
            },
        }
        Ok(false)
    }
}
