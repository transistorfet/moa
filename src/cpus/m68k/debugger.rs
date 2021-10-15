
use crate::error::Error;
use crate::system::System;
use crate::memory::{Address, Addressable};

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
    pub use_debugger: bool,
    pub step_until_return: Option<usize>,
    pub stack_tracer: StackTracer,
}

impl M68kDebugger {
    pub fn new() -> M68kDebugger {
        M68kDebugger {
            breakpoints: vec!(),
            use_tracing: false,
            use_debugger: false,
            step_until_return: None,
            stack_tracer: StackTracer::new(),
        }
    }
}

impl M68k {
    #[allow(dead_code)]
    pub fn enable_tracing(&mut self) {
        self.debugger.use_tracing = true;
    }

    #[allow(dead_code)]
    pub fn enable_debugging(&mut self) {
        self.debugger.use_tracing = true;
        self.debugger.use_debugger = true;
    }

    #[allow(dead_code)]
    pub fn add_breakpoint(&mut self, addr: Address) {
        self.debugger.breakpoints.push(addr as u32);
    }

    pub fn check_breakpoints(&mut self) {
        for breakpoint in &self.debugger.breakpoints {
            if *breakpoint == self.state.pc {
                println!("Breakpoint reached: {:08x}", *breakpoint);
                self.enable_debugging();
                break;
            }
        }
    }

    pub fn run_debugger(&mut self, system: &System) {
        self.dump_state(system);

        match self.debugger.step_until_return {
            Some(level) if level == self.debugger.stack_tracer.calls.len() => { self.debugger.step_until_return = None; },
            Some(_) => { return; },
            None => { },
        }

        loop {
            let mut buffer = String::new();
            std::io::stdin().read_line(&mut buffer).unwrap();
            let args: Vec<&str> = buffer.split_whitespace().collect();
            match self.run_debugger_command(system, args) {
                Ok(true) => return,
                Ok(false) => { },
                Err(err) => {
                    println!("Error: {}", err.msg);
                },
            }
        }
    }

    pub fn run_debugger_command(&mut self, system: &System, args: Vec<&str>) -> Result<bool, Error> {
        if args.len() <= 0 {
            return Ok(true);
        }

        match args[0] {
            "b" | "break" | "breakpoint" => {
                if args.len() != 2 {
                    println!("Usage: breakpoint <addr>");
                } else {
                    let addr = u32::from_str_radix(args[1], 16).map_err(|_| Error::new("Unable to parse breakpoint address"))?;
                    self.add_breakpoint(addr as Address);
                    println!("Breakpoint set for {:08x}", addr);
                }
            },
            "d" | "dump" => {
                if args.len() > 1 {
                    let addr = u32::from_str_radix(args[1], 16).map_err(|_| Error::new("Unable to parse address"))?;
                    let len = if args.len() > 2 { u32::from_str_radix(args[2], 16).map_err(|_| Error::new("Unable to parse length"))? } else { 0x20 };
                    system.get_bus().dump_memory(addr as Address, len as Address);
                } else {
                    system.get_bus().dump_memory(self.state.msp as Address, 0x40 as Address);
                }
            },
            "ds" | "stack" | "dumpstack" => {
                println!("Stack:");
                for addr in &self.debugger.stack_tracer.calls {
                    println!("  {:08x}", system.get_bus().read_beu32(*addr as Address)?);
                }
            },
            "dis" | "disassemble" => {
                let mut decoder = M68kDecoder::new(self.cputype, 0, 0);
                decoder.dump_disassembly(system, self.state.pc, 0x1000);
            },
            "so" | "stepout" => {
                self.debugger.step_until_return = Some(self.debugger.stack_tracer.calls.len() - 1);
                return Ok(true);
            },
            "c" | "continue" => {
                self.debugger.use_tracing = false;
                self.debugger.use_debugger = false;
                return Ok(true);
            },
            _ => { return Ok(true); },
        }
        Ok(false)
    }
}

