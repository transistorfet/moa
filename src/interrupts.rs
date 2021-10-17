
use std::iter;

use crate::error::Error;
use crate::devices::TransmutableBox;


pub struct Signal {
    pub current: bool,
    pub previous: bool,
}

impl Signal {
    pub fn new() -> Signal {
        Signal {
            current: false,
            previous: false,
        }
    }

    pub fn has_changed(&mut self) -> Option<bool> {
        if self.current != self.previous {
            self.previous = self.current;
            Some(self.current)
        } else {
            None
        }
    }

    pub fn set(&mut self, value: bool) {
        self.current = value;
    }
}



pub struct InterruptController {
    pub target: Option<TransmutableBox>,
    pub priority: Vec<Signal>,
}

impl InterruptController {
    pub fn new() -> InterruptController {
        InterruptController {
            target: None,
            priority: iter::repeat_with(|| Signal::new()).take(7).collect::<Vec<_>>(), //vec![Signal::new(); 7],
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
        let signal = &mut self.priority[priority as usize];
        signal.set(state);
        match signal.has_changed() {
            Some(value) => self.notify_interrupt_state(value, priority, number)?,
            None => { },
        }
        Ok(())
    }

    fn notify_interrupt_state(&mut self, state: bool, priority: u8, number: u8) -> Result<(), Error> {
        // TODO how does this find the specific device it's connected to?
        // TODO for the time being, this will find the first device to handle it or fail

        debug!("interrupts: priority {} state changed to {}", priority, state);
        match &self.target {
            Some(dev) => {
                Ok(dev.borrow_mut().as_interruptable().unwrap().interrupt_state_change(state, priority, number)?)
            },
            None => {
                Err(Error::new(&format!("unhandled interrupt: {:x}", number)))
            },
        }
    }
}

