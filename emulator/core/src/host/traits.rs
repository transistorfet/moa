
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::{ClockTime, Error};
use crate::host::gfx::FrameReceiver;
use crate::host::audio::Sample;
use crate::host::keys::KeyEvent;
use crate::host::controllers::ControllerEvent;
use crate::host::mouse::MouseEvent;
use crate::host::input::EventSender;


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

    fn register_controllers(&mut self, _sender: EventSender<ControllerEvent>) -> Result<(), Error> {
        Err(Error::new("This frontend doesn't support game controllers"))
    }

    fn register_keyboard(&mut self, _sender: EventSender<KeyEvent>) -> Result<(), Error> {
        Err(Error::new("This frontend doesn't support the keyboard"))
    }

    fn register_mouse(&mut self, _sender: EventSender<MouseEvent>) -> Result<(), Error> {
        Err(Error::new("This frontend doesn't support the mouse"))
    }
}


pub trait Tty {
    fn device_name(&self) -> String;
    fn read(&mut self) -> Option<u8>;
    fn write(&mut self, output: u8) -> bool;
}

pub trait Audio {
    fn samples_per_second(&self) -> usize;
    fn write_samples(&mut self, clock: ClockTime, buffer: &[Sample]);
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
pub struct ClockedQueue<T>(Arc<Mutex<VecDeque<(ClockTime, T)>>>, usize);

impl<T: Clone> ClockedQueue<T> {
    pub fn new(max: usize) -> Self {
        Self(Arc::new(Mutex::new(VecDeque::new())), max)
    }

    pub fn push(&self, clock: ClockTime, data: T) {
        let mut queue = self.0.lock().unwrap();
        if queue.len() > self.1 {
            //log::warn!("dropping data from queue due to limit of {} items", self.1);
            queue.pop_front();
        }
        queue.push_back((clock, data));
    }

    pub fn pop_next(&self) -> Option<(ClockTime, T)> {
        self.0.lock().unwrap().pop_front()
    }

    pub fn pop_latest(&self) -> Option<(ClockTime, T)> {
        self.0.lock().unwrap().drain(..).last()
    }

    pub fn put_back(&self, clock: ClockTime, data: T) {
        self.0.lock().unwrap().push_front((clock, data));
    }

    pub fn peek_clock(&self) -> Option<ClockTime> {
        self.0.lock().unwrap().front().map(|(clock, _)| *clock)
    }

    pub fn is_empty(&self) -> bool {
        self.0.lock().unwrap().is_empty()
    }
}


pub struct DummyAudio();

impl Audio for DummyAudio {
    fn samples_per_second(&self) -> usize {
        48000
    }

    fn write_samples(&mut self, _clock: ClockTime, _buffer: &[Sample]) {}
}

