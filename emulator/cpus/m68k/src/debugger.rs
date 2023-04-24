
use moa_core::{System, Error, ClockTime, Address, Addressable, Debuggable};

use super::state::M68k;
use super::decode::M68kDecoder;

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
    pub enabled: bool,
    pub breakpoints: Vec<u32>,
    pub use_tracing: bool,
    pub step_until_return: Option<usize>,
    pub stack_tracer: StackTracer,
}

impl Debuggable for M68k {
    fn debugging_enabled(&mut self) -> bool {
        self.debugger.enabled
    }

    fn set_debugging(&mut self, enable: bool) {
        self.debugger.enabled = enable;
    }

    fn add_breakpoint(&mut self, addr: Address) {
        self.debugger.breakpoints.push(addr as u32);
        self.debugger.enabled = true;
    }

    fn remove_breakpoint(&mut self, addr: Address) {
        if let Some(index) = self.debugger.breakpoints.iter().position(|a| *a == addr as u32) {
            self.debugger.breakpoints.remove(index);
            self.debugger.enabled = !self.debugger.breakpoints.is_empty();
        }
    }

    fn print_current_step(&mut self, system: &System) -> Result<(), Error> {
        self.decoder.decode_at(&mut self.port, system.clock, self.state.pc)?;
        self.decoder.dump_decoded(&mut self.port);
        self.dump_state(system.clock);
        Ok(())
    }

    fn print_disassembly(&mut self, addr: Address, count: usize) {
        let mut decoder = M68kDecoder::new(self.cputype, ClockTime::START, 0);
        decoder.dump_disassembly(&mut self.port, addr as u32, count as u32);
    }

    fn execute_command(&mut self, system: &System, args: &[&str]) -> Result<bool, Error> {
        match args[0] {
            "ds" | "stack" | "dumpstack" => {
                println!("Stack:");
                for addr in &self.debugger.stack_tracer.calls {
                    println!("  {:08x}", self.port.read_beu32(system.clock, *addr as Address)?);
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

impl M68k {
    #[allow(dead_code)]
    pub fn enable_tracing(&mut self) {
        self.debugger.use_tracing = true;
    }

    pub fn check_breakpoints(&mut self, system: &System) {
        for breakpoint in &self.debugger.breakpoints {
            if *breakpoint == self.state.pc {
                println!("Breakpoint reached: {:08x}", *breakpoint);
                system.enable_debugging();
                break;
            }
        }
    }
}

