
use crate::error::Error;
use crate::system::System;
use crate::devices::{Address, Addressable, Debuggable};

use super::state::M68k;
use super::decode::M68kDecoder;

pub struct StackTracer {
    pub calls: Vec<u32>,
}

impl StackTracer {
    pub fn new() -> StackTracer {
        StackTracer {
            calls: vec![],
        }
    }

    pub fn push_return(&mut self, addr: u32) {
        self.calls.push(addr);
    }

    pub fn pop_return(&mut self) {
        self.calls.pop();
    }
}


pub struct M68kDebugger {
    pub breakpoints: Vec<u32>,
    pub use_tracing: bool,
    pub step_until_return: Option<usize>,
    pub stack_tracer: StackTracer,
}

impl M68kDebugger {
    pub fn new() -> M68kDebugger {
        M68kDebugger {
            breakpoints: vec!(),
            use_tracing: false,
            step_until_return: None,
            stack_tracer: StackTracer::new(),
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

    fn print_current_step(&mut self, system: &System) -> Result<(), Error> {
        self.decoder.decode_at(&mut self.port, self.state.pc)?;
        self.decoder.dump_decoded(&mut self.port);
        self.dump_state(system);
        Ok(())
    }

    fn print_disassembly(&mut self, addr: Address, count: usize) {
        let mut decoder = M68kDecoder::new(self.cputype, 0);
        decoder.dump_disassembly(&mut self.port, addr as u32, count as u32);
    }

    fn execute_command(&mut self, system: &System, args: &[&str]) -> Result<bool, Error> {
        match args[0] {
            "ds" | "stack" | "dumpstack" => {
                println!("Stack:");
                for addr in &self.debugger.stack_tracer.calls {
                    println!("  {:08x}", self.port.read_beu32(*addr as Address)?);
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

