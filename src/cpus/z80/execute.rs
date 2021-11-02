
use crate::system::System;
use crate::error::{ErrorType, Error};
use crate::devices::{ClockElapsed, Address, Steppable, Interruptable, Addressable, Debuggable, Transmutable};

use super::decode::Z80Decoder;
use super::state::{Z80, Z80State, Status};

impl Steppable for Z80 {
    fn step(&mut self, system: &System) -> Result<ClockElapsed, Error> {
        self.step_internal(system)?;
        Ok((1_000_000_000 / self.frequency as u64) * 4)
    }

    fn on_error(&mut self, system: &System) {
        //self.dump_state(system);
    }
}

impl Interruptable for Z80 { }

impl Transmutable for Z80 {
    fn as_steppable(&mut self) -> Option<&mut dyn Steppable> {
        Some(self)
    }

    fn as_interruptable(&mut self) -> Option<&mut dyn Interruptable> {
        Some(self)
    }

    //fn as_debuggable(&mut self) -> Option<&mut dyn Debuggable> {
    //    Some(self)
    //}
}



impl Z80 {
    pub fn step_internal(&mut self, system: &System) -> Result<(), Error> {
        match self.state.status {
            Status::Init => self.init(system),
            Status::Halted => Err(Error::new("CPU stopped")),
            Status::Running => {
                match self.cycle_one(system) {
                    Ok(()) => Ok(()),
                    //Err(Error { err: ErrorType::Processor, native, .. }) => {
                    Err(Error { err: ErrorType::Processor, native, .. }) => {
                        //self.exception(system, native as u8, false)?;
                        Ok(())
                    },
                    Err(err) => Err(err),
                }
            },
        }
    }

    pub fn init(&mut self, system: &System) -> Result<(), Error> {
        //self.state.msp = self.port.read_beu32(0)?;
        //self.state.pc = self.port.read_beu32(4)?;
        self.state.status = Status::Running;
        Ok(())
    }

    pub fn cycle_one(&mut self, system: &System) -> Result<(), Error> {
        //self.timer.cycle.start();
        self.decode_next(system)?;
        //self.execute_current(system)?;
        //self.timer.cycle.end();

        //if (self.timer.cycle.events % 500) == 0 {
        //    println!("{}", self.timer);
        //}

        //self.check_pending_interrupts(system)?;
        Ok(())
    }

    pub fn decode_next(&mut self, system: &System) -> Result<(), Error> {
        //self.check_breakpoints(system);

        //self.timer.decode.start();
        self.decoder.decode_at(&mut self.port, self.state.pc)?;
        //self.timer.decode.end();

        //if self.debugger.use_tracing {
            self.decoder.dump_decoded(&mut self.port);
        //}

        self.state.pc = self.decoder.end;
        Ok(())
    }

    pub fn execute_current(&mut self, system: &System) -> Result<(), Error> {
        panic!("unimplemented");
    }
}

