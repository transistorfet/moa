
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::{ClockTime, Error};
use crate::host::gfx::{PixelEncoding, Pixel, Frame, FrameReceiver};
use crate::host::keys::KeyEvent;
use crate::host::controllers::{ControllerDevice, ControllerEvent};
use crate::host::mouse::MouseEvent;

pub trait Host {
    fn add_pty(&self) -> Result<Box<dyn Tty>, Error> {
        Err(Error::new("This frontend doesn't support PTYs"))
    }

    fn add_video_source(&mut self, _receiver: FrameReceiver) -> Result<(), Error> {
        Err(Error::new("This frontend doesn't support windows"))
    }

    fn add_audio_source(&mut self) -> Result<Box<dyn Audio>, Error> {
        Err(Error::new("This frontend doesn't support the sound"))
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
}


pub trait Tty {
    fn device_name(&self) -> String;
    fn read(&mut self) -> Option<u8>;
    fn write(&mut self, output: u8) -> bool;
}

pub trait ControllerUpdater: Send {
    fn update_controller(&self, event: ControllerEvent);
}

pub trait KeyboardUpdater: Send {
    fn update_keyboard(&self, event: KeyEvent);
}

pub trait MouseUpdater: Send {
    fn update_mouse(&self, event: MouseEvent);
}

pub trait Audio {
    fn samples_per_second(&self) -> usize;
    fn space_available(&self) -> usize;
    fn write_samples(&mut self, clock: ClockTime, buffer: &[f32]);
    fn flush(&mut self);
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

#[derive(Clone, Default)]
pub struct ClockedQueue<T>(Arc<Mutex<VecDeque<(ClockTime, T)>>>);

impl<T: Clone> ClockedQueue<T> {
    pub fn push(&self, clock: ClockTime, data: T) {
        self.0.lock().unwrap().push_back((clock, data));
    }

    pub fn pop_next(&self) -> Option<(ClockTime, T)> {
        self.0.lock().unwrap().pop_front()
    }

    pub fn pop_latest(&self) -> Option<(ClockTime, T)> {
        self.0.lock().unwrap().drain(..).last()
    }

    pub fn unpop(&mut self, clock: ClockTime, data: T) {
        self.0.lock().unwrap().push_front((clock, data));
    }

    pub fn peek_clock(&self) -> Option<ClockTime> {
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

    fn write_samples(&mut self, _clock: ClockTime, _buffer: &[f32]) {}

    fn flush(&mut self) {}
}

