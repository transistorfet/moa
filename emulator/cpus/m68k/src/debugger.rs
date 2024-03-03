
use moa_core::{System, Error, Address, Addressable, Debuggable};

use super::state::M68k;
use super::decode::M68kDecoder;
use super::execute::M68kCycleGuard;

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
    pub(crate) step_until_return: Option<usize>,
    pub(crate) stack_tracer: StackTracer,
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
        let mut decoder = M68kDecoder::new(self.cputype, true, 0);
        decoder.dump_disassembly(&mut self.port, addr as u32, count as u32);
    }

    fn run_command(&mut self, system: &System, args: &[&str]) -> Result<bool, Error> {
        match args[0] {
            "ds" | "stack" | "dumpstack" => {
                println!("Stack:");
                for addr in &self.debugger.stack_tracer.calls {
                    println!("  {:08x}", self.port.port.read_beu32(system.clock, *addr as Address)?);
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

impl<'a> M68kCycleGuard<'a> {
    pub fn check_breakpoints(&mut self) -> Result<(), Error> {
        for breakpoint in &self.debugger.breakpoints {
            if *breakpoint == self.state.pc {
                if self.debugger.skip_breakpoint > 0 {
                    self.debugger.skip_breakpoint -= 1;
                    return Ok(());
                } else {
                    self.debugger.skip_breakpoint = 1;
                    return Err(Error::breakpoint(format!("breakpoint reached: {:08x}", *breakpoint)));
                }
            }
        }
        Ok(())
    }
}

