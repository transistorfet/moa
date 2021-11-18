
use std::sync::{Arc, Mutex, MutexGuard};

use crate::error::Error;
use crate::host::keys::Key;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum JoystickDevice {
    A,
    B,
    C,
    D,
}

pub trait Host {
    //fn create_pty(&self) -> Result<Box<dyn Tty>, Error>;
    fn add_window(&mut self, updater: Box<dyn WindowUpdater>) -> Result<(), Error>;
    fn register_joystick(&mut self, _device: JoystickDevice, _input: Box<dyn JoystickUpdater>) -> Result<(), Error> { Err(Error::new("Not supported")) }
    fn register_keyboard(&mut self, _input: Box<dyn KeyboardUpdater>) -> Result<(), Error> { Err(Error::new("Not supported")) }
}

pub trait Tty {
    fn device_name(&self) -> String;
    fn read(&mut self) -> Option<u8>;
    fn write(&mut self, output: u8) -> bool;
}

pub trait WindowUpdater: Send {
    fn get_size(&mut self) -> (u32, u32);
    fn update_frame(&mut self, width: u32, height: u32, bitmap: &mut [u32]);
}

pub trait JoystickUpdater: Send {
    fn update_joystick(&mut self, modifiers: u16);
}

pub trait KeyboardUpdater: Send {
    fn update_keyboard(&mut self, key: Key, state: bool);
}

pub trait BlitableSurface {
    fn set_size(&mut self, width: u32, height: u32);
    fn blit<B: Iterator<Item=u32>>(&mut self, pos_x: u32, pos_y: u32, bitmap: B, width: u32, height: u32);
    fn clear(&mut self, value: u32);
}


#[derive(Clone, Debug)]
pub struct SharedData<T>(Arc<Mutex<T>>);

impl<T> SharedData<T> {
    pub fn new(init: T) -> SharedData<T> {
        SharedData(Arc::new(Mutex::new(init)))
    }

    pub fn lock(&self) -> MutexGuard<'_, T> {
        self.0.lock().unwrap()
    }
}

impl<T: Copy> SharedData<T> {
    pub fn set(&mut self, value: T) {
        *(self.0.lock().unwrap()) = value;
    }

    pub fn get(&mut self) -> T {
        *(self.0.lock().unwrap())
    }
}

