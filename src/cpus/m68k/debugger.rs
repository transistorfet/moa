
use crate::error::Error;
use crate::memory::{Address, AddressSpace};

use super::execute::{MC68010};
use super::decode::{Instruction, Target, Size, Direction, Condition, ControlRegister, RegisterType};

pub struct M68kDebugger {
    pub breakpoints: Vec<u32>,
    pub use_tracing: bool,
    pub use_debugger: bool,
}


impl M68kDebugger {
    pub fn new() -> M68kDebugger {
        M68kDebugger {
            breakpoints: vec!(),
            use_tracing: false,
            use_debugger: false,
        }
    }
}

impl MC68010 {
    pub fn enable_tracing(&mut self) {
        self.debugger.use_tracing = true;
    }

    pub fn add_breakpoint(&mut self, addr: Address) {
        self.debugger.breakpoints.push(addr as u32);
    }

    pub fn check_breakpoints(&mut self) {
        for breakpoint in &self.debugger.breakpoints {
            if *breakpoint == self.state.pc {
                self.debugger.use_tracing = true;
                self.debugger.use_debugger = true;
                break;
            }
        }
    }

    pub fn run_debugger(&mut self, space: &mut AddressSpace) {
        self.dump_state(space);
        let mut buffer = String::new();

        loop {
            std::io::stdin().read_line(&mut buffer).unwrap();
            match buffer.as_ref() {
                "dump\n" => space.dump_memory(self.state.msp as Address, (0x200000 - self.state.msp) as Address),
                "continue\n" => {
                    self.debugger.use_debugger = false;
                    return;
                },
                _ => { return; },
            }
        }
    }
}

