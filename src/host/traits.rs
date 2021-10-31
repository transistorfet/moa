
use std::sync::{Arc, Mutex};

use crate::error::Error;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum JoystickDevice {
    A,
    B,
    C,
    D,
}

pub trait Host {
    //fn create_pty(&self) -> Result<Box<dyn Tty>, Error>;
    fn add_window(&self, updater: Box<dyn WindowUpdater>) -> Result<(), Error>;
    fn register_joystick(&self, device: JoystickDevice, input: Box<dyn JoystickUpdater>) -> Result<(), Error> { Err(Error::new("Not supported")) }
}

pub trait Tty {
    fn device_name(&self) -> String;
    fn read(&mut self) -> Option<u8>;
    fn write(&mut self, output: u8) -> bool;
}

pub trait WindowUpdater: Send {
    fn update_frame(&mut self, width: u32, height: u32, bitmap: &mut [u32]);
}

pub trait JoystickUpdater: Send {
    fn update_joystick(&mut self, modifiers: u16);
}

pub trait BlitableSurface {
    fn set_size(&mut self, width: u32, height: u32);
    fn blit<B: Iterator<Item=u32>>(&mut self, pos_x: u32, pos_y: u32, bitmap: B, width: u32, height: u32);
}


