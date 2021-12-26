
use std::io::Write;

use crate::error::Error;
use crate::system::System;
use crate::devices::{Address, Addressable, Debuggable, TransmutableBox};


pub struct Debugger {
    last_command: Option<String>,
    repeat: u32,
    trace_only: bool,
}


impl Debugger {
    pub fn new() -> Self {
        Self {
            last_command: None,
            repeat: 0,
            trace_only: false,
        }
    }

    pub fn breakpoint_occurred(&mut self) {
        self.trace_only = false;
    }

    pub fn run_debugger(&mut self, system: &System, target: TransmutableBox) -> Result<(), Error> {
        let mut target = target.borrow_mut();
        let debug_obj = target.as_debuggable().unwrap();
        println!("@ {} ns", system.clock);
        debug_obj.print_current_step(system)?;

        if self.trace_only {
            return Ok(());
        }

        if self.repeat > 0 {
            self.repeat -= 1;
            let last_command = self.last_command.clone().unwrap();
            let args: Vec<&str> = vec![&last_command];
            self.run_debugger_command(system, debug_obj, &args)?;
            return Ok(());
        }

        loop {
            let mut buffer = String::new();
            std::io::stdout().write_all(b"> ").unwrap();
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
                    let (name, addr) = parse_address(args[1])?;
                    match name {
                        Some(name) => {
                            let target = system.get_device(name)?;
                            target.borrow_mut().as_debuggable().unwrap().add_breakpoint(addr);
                            println!("Breakpoint set for devices {:?} at {:08x}", name, addr);
                        },
                        None => {
                            debug_obj.add_breakpoint(addr);
                            println!("Breakpoint set for {:08x}", addr);
                        },
                    }
                }
            },
            "r" | "remove" => {
                if args.len() != 2 {
                    println!("Usage: remove <addr>");
                } else {
                    let (name, addr) = parse_address(args[1])?;
                    match name {
                        Some(name) => {
                            let target = system.get_device(name)?;
                            target.borrow_mut().as_debuggable().unwrap().remove_breakpoint(addr);
                            println!("Breakpoint removed for devices {:?} at {:08x}", name, addr);
                        },
                        None => {
                            debug_obj.remove_breakpoint(addr);
                            println!("Breakpoint removed for {:08x}", addr);
                        },
                    }
                }
            },
            "d" | "dump" => {
                if args.len() > 1 {
                    let addr = u32::from_str_radix(args[1], 16).map_err(|_| Error::new("Unable to parse address"))?;
                    let len = if args.len() > 2 { u32::from_str_radix(args[2], 16).map_err(|_| Error::new("Unable to parse length"))? } else { 0x20 };
                    system.get_bus().dump_memory(addr as Address, len as Address);
                } else {
                    //self.port.dump_memory(self.state.ssp as Address, 0x40 as Address);
                }
            },
            "i" | "inspect" => {
                if args.len() < 2 {
                    println!("Usage: inspect <device_name> [<device specific arguments>]");
                } else {
                    let device = system.get_device(args[1])?;
                    let subargs = if args.len() > 2 { &args[2..] } else { &[""] };
                    device.borrow_mut().as_inspectable()
                        .ok_or_else(|| Error::new("That device is not inspectable"))?
                        .inspect(system, subargs)?;
                }
            },
            "dis" | "disassemble" => {
                let addr = if args.len() > 1 {
                    Address::from_str_radix(args[1], 16).map_err(|_| Error::new("Unable to parse address"))?
                } else {
                    0
                };

                let count = if args.len() > 2 {
                    usize::from_str_radix(args[2], 16).map_err(|_| Error::new("Unable to parse address"))?
                } else {
                    0x1000
                };

                debug_obj.print_disassembly(addr, count);
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
            "t" | "trace" => {
                self.trace_only = true;
                return Ok(true);
            }
            "setb" | "setw" | "setl" => {
                if args.len() != 3 {
                    println!("Usage: set[b|w|l] <addr> <data>");
                } else {
                    let addr = u64::from_str_radix(args[1], 16).map_err(|_| Error::new("Unable to parse set address"))?;
                    let data = u32::from_str_radix(args[2], 16).map_err(|_| Error::new("Unable to parse data"))?;
                    match args[0] {
                        "setb" => system.get_bus().write_u8(addr, data as u8)?,
                        "setw" => system.get_bus().write_beu16(addr, data as u16)?,
                        "setl" => system.get_bus().write_beu32(addr, data)?,
                        _ => panic!("Unimplemented: {:?}", args[0]),
                    }
                }
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
            self.last_command = Some(args[0].to_string());
        }
        Ok(())
    }
}

fn parse_address(arg: &str) -> Result<(Option<&str>, Address), Error> {
    let (name, addrstr) = match arg.find(':') {
        Some(index) => {
            let (name, addrstr) = arg.split_at(index);
            (Some(name), &addrstr[1..])
        },
        None => (None, arg),
    };

    let addr = Address::from_str_radix(addrstr, 16).map_err(|_| Error::new("Unable to parse address"))?;
    Ok((name, addr))
}

