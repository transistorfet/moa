use crate::error::Error;


pub struct InterruptController {
    interrupts: Vec<(bool, u8)>,
    highest: u8,
}

impl Default for InterruptController {
    fn default() -> InterruptController {
        InterruptController {
            interrupts: vec![(false, 0); 7],
            highest: 0,
        }
    }
}

impl InterruptController {
    pub fn set(&mut self, state: bool, priority: u8, number: u8) -> Result<(), Error> {
        self.interrupts[priority as usize].0 = state;
        self.interrupts[priority as usize].1 = number;
        if state && priority > self.highest {
            self.highest = priority;
        }
        Ok(())
    }

    pub fn check(&mut self) -> (bool, u8, u8) {
        if self.highest > 0 {
            (true, self.highest, self.interrupts[self.highest as usize].1)
        } else {
            (false, 0, 0)
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
