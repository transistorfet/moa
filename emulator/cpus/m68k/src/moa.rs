
use femtos::{Instant, Duration};
use emulator_hal::bus;

use moa_core::{System, Error, Address, Steppable, Interruptable, Addressable, Debuggable, Transmutable};

use crate::state::{M68k, M68kError};
use crate::decode::M68kDecoder;
use crate::execute::M68kCycle;

impl Steppable for M68k {
    fn step(&mut self, system: &System) -> Result<Duration, Error> {
        let cycle = M68kCycle::new(self, system.clock);

        let mut bus = system.bus.borrow_mut();
        let mut adapter: bus::BusAdapter<u32, u64, Instant, &mut dyn Addressable, Error> = bus::BusAdapter::new(
            &mut *bus,
            |addr| addr as u64,
            |err| err.try_into().unwrap(),
        );

        let mut executor = cycle.begin(self, &mut adapter);
        executor.check_breakpoints()?;
        executor.step()?;

        let interrupt = system.get_interrupt_controller().check();
        if let (priority, Some(ack)) = executor.check_pending_interrupts(interrupt)? {
            log::debug!("interrupt: {:?} @ {} ns", priority, system.clock.as_duration().as_nanos());
            system.get_interrupt_controller().acknowledge(priority as u8)?;
        }

        self.cycle = Some(executor.end());
        Ok(self.last_cycle_duration())
    }

    fn on_error(&mut self, _system: &System) {
        let mut output = String::with_capacity(256);
        self.dump_state(&mut output);
        println!("{}", output);
    }
}

impl Interruptable for M68k { }

impl Transmutable for M68k {
    fn as_steppable(&mut self) -> Option<&mut dyn Steppable> {
        Some(self)
    }

    fn as_interruptable(&mut self) -> Option<&mut dyn Interruptable> {
        Some(self)
    }

    fn as_debuggable(&mut self) -> Option<&mut dyn Debuggable> {
        Some(self)
    }
}

impl<BusError> From<Error> for M68kError<BusError> {
    fn from(err: Error) -> Self {
        match err {
            Error::Processor(ex) => M68kError::Interrupt(ex as u8),
            Error::Breakpoint(msg) => M68kError::Breakpoint,
            Error::Other(msg) | Error::Assertion(msg) | Error::Emulator(_, msg) => M68kError::Other(format!("{}", msg)),
        }
    }
}

impl<BusError: bus::Error> From<M68kError<BusError>> for Error {
    fn from(err: M68kError<BusError>) -> Self {
        match err {
            M68kError::Halted => Self::Other("cpu halted".to_string()),
            M68kError::Exception(ex) => Self::Processor(ex as u32),
            M68kError::Interrupt(num) => Self::Processor(num as u32),
            M68kError::Breakpoint => Self::Breakpoint("breakpoint".to_string()),
            M68kError::InvalidTarget(target) => Self::new(target.to_string()),
            M68kError::BusError(msg) => Self::Other(format!("{:?}", msg)),
            M68kError::Other(msg) => Self::Other(msg),
        }
    }
}


impl Debuggable for M68k {
    fn add_breakpoint(&mut self, addr: Address) {
        self.debugger.breakpoints.push(addr as u32);
    }

    fn remove_breakpoint(&mut self, addr: Address) {
        if let Some(index) = self.debugger.breakpoints.iter().position(|a| *a == addr as u32) {
            self.debugger.breakpoints.remove(index);
        }
    }

    fn print_current_step(&mut self, _system: &System) -> Result<(), Error> {
        // TODO this is called by the debugger, but should be called some other way
        //let _ = self.decoder.decode_at(&mut self.port, true, self.state.pc);
        //self.decoder.dump_decoded(&mut self.port);
        //self.dump_state();
        Ok(())
    }

    fn print_disassembly(&mut self, addr: Address, count: usize) {
        let mut decoder = M68kDecoder::new(self.info.chip, true, 0);
        //decoder.dump_disassembly(&mut self.port, self.cycle.memory, addr as u32, count as u32);
    }

    fn run_command(&mut self, system: &System, args: &[&str]) -> Result<bool, Error> {
        match args[0] {
            "ds" | "stack" | "dumpstack" => {
                println!("Stack:");
                for addr in &self.debugger.stack_tracer.calls {
                    println!("  {:08x}", system.bus.borrow_mut().read_beu32(system.clock, *addr as Address)?);
                }
            },
            "so" | "stepout" => {
                self.debugger.step_until_return = Some(self.debugger.stack_tracer.calls.len() - 1);
            },
            _ => { return Ok(true); },
        }
        Ok(false)
    }
}

