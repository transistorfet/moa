
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::{Clock, Error};
use crate::host::gfx::Frame;
use crate::host::keys::KeyEvent;
use crate::host::controllers::{ControllerDevice, ControllerEvent};
use crate::host::mouse::MouseEvent;

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

    fn register_mouse(&mut self, _input: Box<dyn MouseUpdater>) -> Result<(), Error> {
        Err(Error::new("This frontend doesn't support the mouse"))
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
    fn max_size(&mut self) -> (u32, u32);
    fn take_frame(&mut self) -> Result<Frame, Error>;

    fn update_frame(&mut self, width: u32, _height: u32, bitmap: &mut [u32]) {
        if let Ok(frame) = self.take_frame() {
            for y in 0..frame.height {
                for x in 0..frame.width {
                    bitmap[(x + (y * width)) as usize] = frame.bitmap[(x + (y * frame.width)) as usize];
                }
            }
        }
    }
}

pub trait ControllerUpdater: Send {
    fn update_controller(&mut self, event: ControllerEvent);
}

pub trait KeyboardUpdater: Send {
    fn update_keyboard(&mut self, event: KeyEvent);
}

pub trait MouseUpdater: Send {
    fn update_mouse(&mut self, event: MouseEvent);
}

pub trait Audio {
    fn samples_per_second(&self) -> usize;
    fn space_available(&self) -> usize;
    fn write_samples(&mut self, clock: Clock, buffer: &[f32]);
    fn flush(&mut self);
}

pub trait BlitableSurface {
    fn set_size(&mut self, width: u32, height: u32);
    fn set_pixel(&mut self, pos_x: u32, pos_y: u32, pixel: u32);
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

#[derive(Clone)]
pub struct ClockedQueue<T>(Arc<Mutex<VecDeque<(Clock, T)>>>);

impl<T: Clone> ClockedQueue<T> {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(VecDeque::new())))
    }

    pub fn push(&self, clock: Clock, data: T) {
        self.0.lock().unwrap().push_back((clock, data));
    }

    pub fn pop_next(&self) -> Option<(Clock, T)> {
        self.0.lock().unwrap().pop_front()
    }

    pub fn pop_latest(&self) -> Option<(Clock, T)> {
        self.0.lock().unwrap().drain(..).last()
    }

    pub fn unpop(&mut self, clock: Clock, data: T) {
        self.0.lock().unwrap().push_front((clock, data));
    }

    pub fn peek_clock(&self) -> Option<Clock> {
        self.0.lock().unwrap().front().map(|(clock, _)| *clock)
    }
}


pub struct DummyAudio();

impl Audio for DummyAudio {
    fn samples_per_second(&self) -> usize {
        48000
    }

    fn space_available(&self) -> usize {
        4800
    }

    fn write_samples(&mut self, _clock: Clock, _buffer: &[f32]) {}

    fn flush(&mut self) {}
}

