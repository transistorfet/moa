
use std::iter;

use crate::error::Error;
use crate::devices::TransmutableBox;


pub struct InterruptController {
    pub target: Option<TransmutableBox>,
    pub interrupts: Vec<(bool, u8)>,
    pub highest: u8,
}

impl InterruptController {
    pub fn new() -> InterruptController {
        InterruptController {
            target: None,
            interrupts: vec![(false, 0); 7],
            highest: 0,
        }
    }

    pub fn set_target(&mut self, dev: TransmutableBox) -> Result<(), Error> {
        if self.target.is_some() {
            return Err(Error::new("Interruptable device already set, and interrupt controller only supports one receiver"));
        }

        self.target = Some(dev);
        Ok(())
    }

    pub fn set(&mut self, state: bool, priority: u8, number: u8) -> Result<(), Error> {
        self.interrupts[priority as usize].0 = state;
        self.interrupts[priority as usize].1 = number;
        if state && priority > self.highest {
            self.highest = priority;
        }
        Ok(())
    }

    pub fn check(&mut self) -> (bool, u8) {
        if self.highest > 0 {
            (true, self.highest)
        } else {
            (false, 0)
        }
    }

    pub fn acknowledge(&mut self, priority: u8) -> Result<u8, Error> {
        let acknowledge = self.interrupts[priority as usize].1;
        self.interrupts[priority as usize].0 = false;
        while self.highest > 0 && !self.interrupts[self.highest as usize].0 {
            self.highest -= 1;
        }
        Ok(acknowledge)
    }
}

