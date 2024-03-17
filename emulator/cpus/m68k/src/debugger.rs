use femtos::Instant;
use emulator_hal::bus::BusAccess;

use super::state::M68kError;
use super::execute::M68kCycleExecutor;
use super::memory::M68kAddress;

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


#[derive(Clone, Default)]
pub struct M68kDebugger {
    pub(crate) skip_breakpoint: usize,
    pub(crate) breakpoints: Vec<u32>,
    #[allow(dead_code)]
    pub(crate) step_until_return: Option<usize>,
    pub(crate) stack_tracer: StackTracer,
}

impl<'a, Bus, BusError> M68kCycleExecutor<'a, Bus>
where
    Bus: BusAccess<M68kAddress, Instant, Error = BusError>,
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
