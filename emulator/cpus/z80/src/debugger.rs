use moa_core::{System, Error, Address, Debuggable};

use crate::state::{Z80, Z80Error};
use crate::decode::Z80Decoder;
use crate::instructions::Register;


#[derive(Clone, Default)]
pub struct Z80Debugger {
    pub(crate) skip_breakpoint: usize,
    pub(crate) breakpoints: Vec<u16>,
}

impl Debuggable for Z80 {
    fn add_breakpoint(&mut self, addr: Address) {
        self.debugger.breakpoints.push(addr as u16);
    }

    fn remove_breakpoint(&mut self, addr: Address) {
        if let Some(index) = self.debugger.breakpoints.iter().position(|a| *a == addr as u16) {
            self.debugger.breakpoints.remove(index);
        }
    }

    fn print_current_step(&mut self, system: &System) -> Result<(), Error> {
        self.decoder.decode_at(&mut self.port, system.clock, self.state.pc)?;
        self.decoder.dump_decoded(&mut self.port);
        self.dump_state(system.clock);
        Ok(())
    }

    fn print_disassembly(&mut self, _system: &System, addr: Address, count: usize) {
        let mut decoder = Z80Decoder::default();
        decoder.dump_disassembly(&mut self.port, addr as u16, count as u16);
    }

    fn run_command(&mut self, _system: &System, args: &[&str]) -> Result<bool, Error> {
        match args[0] {
            "l" => self.state.reg[Register::L as usize] = 0x05,
            _ => {
                return Ok(true);
            },
        }
        Ok(false)
    }
}

impl Z80 {
    pub fn check_breakpoints(&mut self) -> Result<(), Z80Error> {
        for breakpoint in &self.debugger.breakpoints {
            if *breakpoint == self.state.pc {
                if self.debugger.skip_breakpoint > 0 {
                    self.debugger.skip_breakpoint -= 1;
                    return Ok(());
                } else {
                    self.debugger.skip_breakpoint = 1;
                    return Err(Z80Error::Breakpoint);
                }
            }
        }
        Ok(())
    }
}
