
use moa_core::Error;
use moa_core::host::{Host, Tty, WindowUpdater, ControllerDevice, ControllerUpdater, Audio};

use moa_common::audio::{AudioMixer, AudioSource};

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

    fn register_controller(&mut self, _device: ControllerDevice, _input: Box<dyn ControllerUpdater>) -> Result<(), Error> {
        println!("console: register_controller() is not supported from the console; ignoring request...");
        Ok(())
    }

    fn create_audio_source(&mut self) -> Result<Box<dyn Audio>, Error> {
        println!("console: create_audio_source() is not supported from the console; returning dummy device...");
        let source = AudioSource::new(AudioMixer::with_default_rate());
        Ok(Box::new(source))
    }
}

