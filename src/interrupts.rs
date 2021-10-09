
use crate::error::Error;


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

    pub fn check_change<F>(&mut self, f: F) -> Result<(), Error> where F: Fn(bool) -> Result<(), Error> {
        if self.current != self.previous {
            self.previous = self.current;
            f(self.current)
        } else {
            Ok(())
        }
    }

    pub fn set(&mut self, value: bool) {
        self.current = value;
    }
}



pub struct InterruptController {

}


