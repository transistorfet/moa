// m68k Debugger

use core::fmt;

use emulator_hal::time;
use emulator_hal::bus::{self, BusAccess};
use emulator_hal::step::{Inspect, Debug};

use crate::{M68k, M68kError, M68kAddress, M68kCycleExecutor};

#[derive(Clone, Default)]
pub struct StackTracer {
    pub calls: Vec<u32>,
}

impl StackTracer {
    pub fn push_return(&mut self, addr: u32) {
        self.calls.push(addr);
    }

    pub fn pop_return(&mut self) {
        self.calls.pop();
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum M68kInfo {
    State,
}

impl<Bus, BusError, Instant, Writer> Inspect<M68kAddress, Bus, Writer> for M68k<Instant>
where
    Bus: BusAccess<M68kAddress, Instant = Instant, Error = BusError>,
    BusError: bus::Error,
    Writer: fmt::Write,
{
    type InfoType = M68kInfo;

    type Error = M68kError<BusError>;

    fn inspect(&mut self, info: Self::InfoType, _bus: &mut Bus, writer: &mut Writer) -> Result<(), Self::Error> {
        match info {
            M68kInfo::State => self
                .state
                .dump_state(writer)
                .map_err(|_| M68kError::Other("error while formatting state".to_string())),
        }
    }

    fn brief_summary(&mut self, bus: &mut Bus, writer: &mut Writer) -> Result<(), Self::Error> {
        self.inspect(M68kInfo::State, bus, writer)
    }

    fn detailed_summary(&mut self, bus: &mut Bus, writer: &mut Writer) -> Result<(), Self::Error> {
        self.inspect(M68kInfo::State, bus, writer)
    }
}

/// Control the execution of a CPU device for debugging purposes
impl<Bus, BusError, Instant, Writer> Debug<M68kAddress, Bus, Writer> for M68k<Instant>
where
    Bus: BusAccess<M68kAddress, Instant = Instant, Error = BusError>,
    BusError: bus::Error,
    Instant: time::Instant,
    Writer: fmt::Write,
{
    // TODO this should be a new type
    type DebugError = M68kError<BusError>;

    fn get_execution_address(&mut self) -> Result<M68kAddress, Self::DebugError> {
        Ok(self.state.pc)
    }

    fn set_execution_address(&mut self, address: M68kAddress) -> Result<(), Self::DebugError> {
        self.state.pc = address;
        Ok(())
    }

    fn add_breakpoint(&mut self, address: M68kAddress) {
        self.debugger.breakpoints.push(address);
    }

    fn remove_breakpoint(&mut self, address: M68kAddress) {
        if let Some(index) = self.debugger.breakpoints.iter().position(|a| *a == address) {
            self.debugger.breakpoints.remove(index);
        }
    }

    fn clear_breakpoints(&mut self) {
        self.debugger.breakpoints.clear();
    }
}


#[derive(Clone, Default)]
pub struct M68kDebugger {
    pub(crate) skip_breakpoint: usize,
    pub(crate) breakpoints: Vec<u32>,
    #[allow(dead_code)]
    pub(crate) step_until_return: Option<usize>,
    pub(crate) stack_tracer: StackTracer,
}

impl<'a, Bus, BusError, Instant> M68kCycleExecutor<'a, Bus, Instant>
where
    Bus: BusAccess<M68kAddress, Instant = Instant, Error = BusError>,
    Instant: Copy,
{
    pub fn check_breakpoints(&mut self) -> Result<(), M68kError<BusError>> {
        for breakpoint in &self.debugger.breakpoints {
            if *breakpoint == self.state.pc {
                if self.debugger.skip_breakpoint > 0 {
                    self.debugger.skip_breakpoint -= 1;
                    return Ok(());
                } else {
                    self.debugger.skip_breakpoint = 1;
                    return Err(M68kError::Breakpoint);
                }
            }
        }
        Ok(())
    }
}
