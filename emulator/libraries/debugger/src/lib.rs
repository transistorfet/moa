use moa_core::{Error, System, Address, Addressable};


#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DebugControl {
    /// Wait for the next user command
    Wait,
    /// Continue looping without accepting input
    Continue,
    /// Exit the debugger and return to normal operation
    Exit,
}


#[derive(Default)]
pub struct Debugger {
    repeat_command: Option<(u32, String)>,
    trace_only: bool,
}


impl Debugger {
    pub fn breakpoint_occurred(&mut self) {
        self.trace_only = false;
    }

    pub fn print_step(&mut self, system: &mut System) -> Result<(), Error> {
        println!("@ {} ns", system.clock.as_duration().as_nanos());
        if let Some(device) = system.get_next_debuggable_device() {
            device.borrow_mut().as_debuggable().unwrap().print_current_step(system)?;
        }
        Ok(())
    }

    pub fn check_auto_command(&mut self, system: &mut System) -> Result<DebugControl, Error> {
        if self.trace_only {
            return Ok(DebugControl::Continue);
        }

        if let Some((count, command)) = self.repeat_command.take() {
            self.run_command(system, &command)?;
            let next_count = count - 1;
            if next_count == 0 {
                self.repeat_command = None;
            } else {
                self.repeat_command = Some((next_count, command));
            }
            return Ok(DebugControl::Continue);
        }

        Ok(DebugControl::Wait)
    }

    pub fn run_command(&mut self, system: &mut System, command: &str) -> Result<DebugControl, Error> {
        let args: Vec<&str> = command.split_whitespace().collect();

        // If no command given, then run the `step` command
        let args = if args.is_empty() { vec!["step"] } else { args };

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
                            if let Some(device) = system.get_next_debuggable_device() {
                                device.borrow_mut().as_debuggable().unwrap().add_breakpoint(addr);
                                println!("Breakpoint set for {:08x}", addr);
                            }
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
                            if let Some(device) = system.get_next_debuggable_device() {
                                device.borrow_mut().as_debuggable().unwrap().remove_breakpoint(addr);
                                println!("Breakpoint removed for {:08x}", addr);
                            }
                        },
                    }
                }
            },
            "w" | "watch" => {
                if args.len() != 2 {
                    println!("Usage: watch <addr>");
                } else {
                    let addr = Address::from_str_radix(args[1], 16).map_err(|_| Error::new("Unable to parse address"))?;
                    system.get_bus().add_watcher(addr);
                }
            },
            "rw" | "rwatch" | "remove_watch" => {
                if args.len() != 2 {
                    println!("Usage: remove_watch <addr>");
                } else {
                    let addr = Address::from_str_radix(args[1], 16).map_err(|_| Error::new("Unable to parse address"))?;
                    system.get_bus().remove_watcher(addr);
                }
            },

            "d" | "dump" => {
                if args.len() > 1 {
                    let addr = u32::from_str_radix(args[1], 16).map_err(|_| Error::new("Unable to parse address"))?;
                    let len = if args.len() > 2 {
                        u32::from_str_radix(args[2], 16).map_err(|_| Error::new("Unable to parse length"))?
                    } else {
                        0x20
                    };
                    system.get_bus().dump_memory(system.clock, addr as Address, len as Address);
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
                    device
                        .borrow_mut()
                        .as_inspectable()
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

                if let Some(device) = system.get_next_debuggable_device() {
                    device
                        .borrow_mut()
                        .as_debuggable()
                        .unwrap()
                        .print_disassembly(system, addr, count);
                }
            },
            "c" | "continue" => {
                self.check_repeat_arg(&args)?;
                return Ok(DebugControl::Exit);
            },
            "s" | "step" => {
                self.check_repeat_arg(&args)?;
                system.step_until_debuggable()?;
                return Ok(DebugControl::Wait);
            },
            "t" | "trace" => {
                self.trace_only = true;
                system.step_until_debuggable()?;
                return Ok(DebugControl::Continue);
            },
            "setb" | "setw" | "setl" => {
                if args.len() != 3 {
                    println!("Usage: set[b|w|l] <addr> <data>");
                } else {
                    let addr = u64::from_str_radix(args[1], 16).map_err(|_| Error::new("Unable to parse set address"))?;
                    let data = u32::from_str_radix(args[2], 16).map_err(|_| Error::new("Unable to parse data"))?;
                    match args[0] {
                        "setb" => system.get_bus().write_u8(system.clock, addr, data as u8)?,
                        "setw" => system.get_bus().write_beu16(system.clock, addr, data as u16)?,
                        "setl" => system.get_bus().write_beu32(system.clock, addr, data)?,
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
                if let Some(device) = system.get_next_debuggable_device() {
                    if device.borrow_mut().as_debuggable().unwrap().run_command(system, &args)? {
                        println!("Error: unknown command {}", args[0]);
                    }
                }
            },
        }
        Ok(DebugControl::Wait)
    }

    fn check_repeat_arg(&mut self, args: &[&str]) -> Result<(), Error> {
        if args.len() > 1 {
            let count = args[1]
                .parse::<u32>()
                .map_err(|_| Error::new("Unable to parse repeat number"))?;
            self.repeat_command = Some((count, args[0].to_string()));
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
