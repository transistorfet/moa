
use std::io::Write;

use crate::error::Error;
use crate::system::System;
use crate::devices::{Address, Debuggable, TransmutableBox};


/*
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
*/

pub struct Debugger {
    pub last_command: Option<String>,
    pub repeat: u32,
}


impl Debugger {
    pub fn new() -> Self {
        Self {
            last_command: None,
            repeat: 0,
        }
    }

    pub fn run_debugger(&mut self, system: &System, target: TransmutableBox) -> Result<(), Error> {
        let mut target = target.borrow_mut();
        let debug_obj = target.as_debuggable().unwrap();
        debug_obj.print_current_step(system)?;

        if self.repeat > 0 {
            self.repeat -= 1;
            let last_command = self.last_command.clone().unwrap();
            let args: Vec<&str> = vec![&last_command];
            self.run_debugger_command(system, debug_obj, &args)?;
            return Ok(());
        }

        loop {
            let mut buffer = String::new();
            std::io::stdout().write_all(b"> ");
            std::io::stdin().read_line(&mut buffer).unwrap();
            let args: Vec<&str> = buffer.split_whitespace().collect();
            match self.run_debugger_command(system, debug_obj, &args) {
                Ok(true) => return Ok(()),
                Ok(false) => { },
                Err(err) => {
                    println!("Error: {}", err.msg);
                },
            }
        }
    }

    pub fn run_debugger_command(&mut self, system: &System, debug_obj: &mut dyn Debuggable, args: &[&str]) -> Result<bool, Error> {
        if args.len() == 0 {
            // The Default Command
            return Ok(true);
        }

        match args[0] {
            "b" | "break" | "breakpoint" => {
                if args.len() != 2 {
                    println!("Usage: breakpoint <addr>");
                } else {
                    let addr = u32::from_str_radix(args[1], 16).map_err(|_| Error::new("Unable to parse breakpoint address"))?;
                    debug_obj.add_breakpoint(addr as Address);
                    println!("Breakpoint set for {:08x}", addr);
                }
            },
            "r" | "remove" => {
                if args.len() != 2 {
                    println!("Usage: breakpoint <addr>");
                } else {
                    let addr = u32::from_str_radix(args[1], 16).map_err(|_| Error::new("Unable to parse breakpoint address"))?;
                    debug_obj.remove_breakpoint(addr as Address);
                    println!("Breakpoint removed for {:08x}", addr);
                }
            },
            "d" | "dump" => {
                if args.len() > 1 {
                    let addr = u32::from_str_radix(args[1], 16).map_err(|_| Error::new("Unable to parse address"))?;
                    let len = if args.len() > 2 { u32::from_str_radix(args[2], 16).map_err(|_| Error::new("Unable to parse length"))? } else { 0x20 };
                    system.get_bus().dump_memory(addr as Address, len as Address);
                } else {
                    //self.port.dump_memory(self.state.msp as Address, 0x40 as Address);
                }
            },
            "dis" | "disassemble" => {
                debug_obj.print_disassembly(0, 0);
            },
            "c" | "continue" => {
                self.check_repeat_arg(args)?;
                system.disable_debugging();
                return Ok(true);
            },
            "s" | "step" => {
                self.check_repeat_arg(args)?;
                return Ok(true);
            },
            //"ds" | "stack" | "dumpstack" => {
            //    println!("Stack:");
            //    for addr in &self.debugger.stack_tracer.calls {
            //        println!("  {:08x}", self.port.read_beu32(*addr as Address)?);
            //    }
            //},
            //"so" | "stepout" => {
            //    self.debugger.step_until_return = Some(self.debugger.stack_tracer.calls.len() - 1);
            //    return Ok(true);
            //},
            _ => {
                if debug_obj.execute_command(system, args)? {
                    println!("Error: unknown command {}", args[0]);
                }
            },
        }
        Ok(false)
    }

    fn check_repeat_arg(&mut self, args: &[&str]) -> Result<(), Error> {
        if args.len() > 1 {
            self.repeat = u32::from_str_radix(args[1], 10).map_err(|_| Error::new("Unable to parse repeat number"))?;
            self.last_command = Some("c".to_string());
        }
        Ok(())
    }
}

