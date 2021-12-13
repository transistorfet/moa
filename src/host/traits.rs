
use std::sync::{Arc, Mutex, MutexGuard};

use crate::error::Error;
use crate::host::keys::Key;
use crate::host::controllers::{ControllerDevice, ControllerEvent};

pub trait Host {
    fn create_pty(&self) -> Result<Box<dyn Tty>, Error> {
        Err(Error::new("This frontend doesn't support PTYs"))
    }

    fn add_window(&mut self, _updater: Box<dyn WindowUpdater>) -> Result<(), Error> {
        Err(Error::new("This frontend doesn't support windows"))
    }

    fn register_controller(&mut self, _device: ControllerDevice, _input: Box<dyn ControllerUpdater>) -> Result<(), Error> {
        Err(Error::new("This frontend doesn't support game controllers"))
    }

    fn register_keyboard(&mut self, _input: Box<dyn KeyboardUpdater>) -> Result<(), Error> {
        Err(Error::new("This frontend doesn't support the keyboard"))
    }

    fn create_audio_source(&mut self) -> Result<Box<dyn Audio>, Error> {
        Err(Error::new("This frontend doesn't support the sound"))
    }
}


pub trait Tty {
    fn device_name(&self) -> String;
    fn read(&mut self) -> Option<u8>;
    fn write(&mut self, output: u8) -> bool;
}

pub trait WindowUpdater: Send {
    fn get_size(&mut self) -> (u32, u32);
    fn update_frame(&mut self, width: u32, height: u32, bitmap: &mut [u32]);
    //fn update_frame(&mut self, draw_buffer: &mut dyn FnMut(u32, u32, &[u32]));
}

pub trait ControllerUpdater: Send {
    fn update_controller(&mut self, event: ControllerEvent);
}

pub trait KeyboardUpdater: Send {
    fn update_keyboard(&mut self, key: Key, state: bool);
}

pub trait Audio {
    fn samples_per_second(&self) -> usize;
    fn write_samples(&mut self, samples: usize, iter: &mut dyn Iterator<Item=f32>);
}

pub trait BlitableSurface {
    fn set_size(&mut self, width: u32, height: u32);
    fn blit<B: Iterator<Item=u32>>(&mut self, pos_x: u32, pos_y: u32, bitmap: B, width: u32, height: u32);
    fn clear(&mut self, value: u32);
}


#[derive(Clone, Debug)]
pub struct HostData<T>(Arc<Mutex<T>>);

impl<T> HostData<T> {
    pub fn new(init: T) -> HostData<T> {
        HostData(Arc::new(Mutex::new(init)))
    }

    pub fn lock(&self) -> MutexGuard<'_, T> {
        self.0.lock().unwrap()
    }
}

impl<T: Copy> HostData<T> {
    pub fn set(&mut self, value: T) {
        *(self.0.lock().unwrap()) = value;
    }

    pub fn get(&mut self) -> T {
        *(self.0.lock().unwrap())
    }
}

