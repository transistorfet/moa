
use moa_core::{System, Error, Address, Debuggable};

use crate::state::Z80;
use crate::decode::Z80Decoder;


#[derive(Clone, Default)]
pub struct Z80Debugger {
    pub enabled: bool,
    pub breakpoints: Vec<u16>,
}

impl Debuggable for Z80 {
    fn debugging_enabled(&mut self) -> bool {
        self.debugger.enabled
    }

    fn set_debugging(&mut self, enable: bool) {
        self.debugger.enabled = enable;
    }

    fn add_breakpoint(&mut self, addr: Address) {
        self.debugger.breakpoints.push(addr as u16);
        self.debugger.enabled = true;
    }

    fn remove_breakpoint(&mut self, addr: Address) {
        if let Some(index) = self.debugger.breakpoints.iter().position(|a| *a == addr as u16) {
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
        let mut decoder = Z80Decoder::default();
        decoder.dump_disassembly(&mut self.port, addr as u16, count as u16);
    }

    fn execute_command(&mut self, _system: &System, args: &[&str]) -> Result<bool, Error> {
        match args[0] {
            "l" => {
                use super::state::Register;
                self.state.reg[Register::L as usize] = 0x05
            },
            _ => { return Ok(true); },
        }
        Ok(false)
    }
}

impl Z80 {
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

