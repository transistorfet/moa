
use moa_core::Error;
use moa_core::host::{Host, Tty, WindowUpdater};

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

