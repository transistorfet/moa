
use moa::error::Error;
use moa::host::traits::{Host, Tty, WindowUpdater};

pub struct ConsoleFrontend;

impl Host for ConsoleFrontend {
    fn create_pty(&self) -> Result<Box<dyn Tty>, Error> {
        use moa_common::tty::SimplePty;
        Ok(Box::new(SimplePty::open()?))
    }

    fn add_window(&mut self, _updater: Box<dyn WindowUpdater>) -> Result<(), Error> {
        println!("console: add_window() is not supported from the console; ignoring request...");
        Ok(())
    }
}

