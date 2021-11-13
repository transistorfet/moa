
use moa::error::Error;
use moa::host::traits::{Host, WindowUpdater};

pub struct ConsoleFrontend;

impl Host for ConsoleFrontend {
    fn add_window(&mut self, updater: Box<dyn WindowUpdater>) -> Result<(), Error> {
        println!("console: add_window() is not supported from the console; ignoring request...");
        Ok(())
    }
}

