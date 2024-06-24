use crate::state::{Z80Error, Z80Address};

#[derive(Clone, Default)]
pub struct Z80Debugger {
    pub(crate) skip_breakpoint: usize,
    pub(crate) breakpoints: Vec<u16>,
}

impl Z80Debugger {
    pub fn check_breakpoints(&mut self, pc: Z80Address) -> Result<(), Z80Error> {
        for breakpoint in &self.breakpoints {
            if *breakpoint == pc {
                if self.skip_breakpoint > 0 {
                    self.skip_breakpoint -= 1;
                    return Ok(());
                } else {
                    self.skip_breakpoint = 1;
                    return Err(Z80Error::Breakpoint);
                }
            }
        }
        Ok(())
    }
}
