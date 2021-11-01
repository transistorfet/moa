
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

pub struct Debugger;


impl Debugger {
    pub fn run_debugger(system: &System, target: TransmutableBox) -> Result<(), Error> {
        let mut target = target.borrow_mut();
        let debug_obj = target.as_debuggable().unwrap();
        debug_obj.print_current_step(system)?;

        loop {
            let mut buffer = String::new();
            std::io::stdin().read_line(&mut buffer).unwrap();
            let args: Vec<&str> = buffer.split_whitespace().collect();
            match Debugger::run_debugger_command(system, debug_obj, &args) {
                Ok(true) => return Ok(()),
                Ok(false) => { },
                Err(err) => {
                    println!("Error: {}", err.msg);
                },
            }
        }
    }

    pub fn run_debugger_command(system: &System, debug_obj: &mut dyn Debuggable, args: &[&str]) -> Result<bool, Error> {
        if args.len() == 0 {
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
            "c" | "continue" => {
                system.disable_debugging();
                return Ok(true);
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
                    return Ok(true);
                }
            },
        }
        Ok(false)
    }
}

