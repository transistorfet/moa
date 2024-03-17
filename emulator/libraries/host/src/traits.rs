use std::fmt;
use std::error::Error;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use femtos::Instant;

use crate::gfx::FrameReceiver;
use crate::audio::Sample;
use crate::keys::KeyEvent;
use crate::controllers::ControllerEvent;
use crate::mouse::MouseEvent;
use crate::input::EventSender;

#[derive(Clone, Debug, thiserror::Error)]
pub enum HostError<E> {
    TTYNotSupported,
    VideoSourceNotSupported,
    AudioSourceNotSupported,
    ControllerNotSupported,
    KeyboardNotSupported,
    MouseNotSupported,
    #[from(E)]
    Specific(E),
}


impl<E> fmt::Display for HostError<E>
where
    E: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            HostError::TTYNotSupported => write!(f, "This frontend doesn't support PTYs"),
            HostError::VideoSourceNotSupported => write!(f, "This frontend doesn't support windows"),
            HostError::AudioSourceNotSupported => write!(f, "This frontend doesn't support the sound"),
            HostError::ControllerNotSupported => write!(f, "This frontend doesn't support game controllers"),
            HostError::KeyboardNotSupported => write!(f, "This frontend doesn't support the keyboard"),
            HostError::MouseNotSupported => write!(f, "This frontend doesn't support the mouse"),
            HostError::Specific(err) => write!(f, "{}", err),
        }
    }
}

pub trait Host {
    type Error: Error;

    fn add_pty(&self) -> Result<Box<dyn Tty>, HostError<Self::Error>> {
        Err(HostError::TTYNotSupported)
    }

    fn add_video_source(&mut self, _receiver: FrameReceiver) -> Result<(), HostError<Self::Error>> {
        Err(HostError::VideoSourceNotSupported)
    }

    fn add_audio_source(&mut self) -> Result<Box<dyn Audio>, HostError<Self::Error>> {
        Err(HostError::AudioSourceNotSupported)
    }

    fn register_controllers(&mut self, _sender: EventSender<ControllerEvent>) -> Result<(), HostError<Self::Error>> {
        Err(HostError::ControllerNotSupported)
    }

    fn register_keyboard(&mut self, _sender: EventSender<KeyEvent>) -> Result<(), HostError<Self::Error>> {
        Err(HostError::KeyboardNotSupported)
    }

    fn register_mouse(&mut self, _sender: EventSender<MouseEvent>) -> Result<(), HostError<Self::Error>> {
        Err(HostError::MouseNotSupported)
    }
}


pub trait Tty {
    fn device_name(&self) -> String;
    fn read(&mut self) -> Option<u8>;
    fn write(&mut self, output: u8) -> bool;
}

pub trait Audio {
    fn samples_per_second(&self) -> usize;
    fn write_samples(&mut self, clock: Instant, buffer: &[Sample]);
}


#[derive(Clone, Default)]
pub struct ClockedQueue<T>(Arc<Mutex<VecDeque<(Instant, T)>>>, usize);

impl<T: Clone> ClockedQueue<T> {
    pub fn new(max: usize) -> Self {
        Self(Arc::new(Mutex::new(VecDeque::new())), max)
    }

    pub fn push(&self, clock: Instant, data: T) {
        let mut queue = self.0.lock().unwrap();
        if queue.len() > self.1 {
            //log::warn!("dropping data from queue due to limit of {} items", self.1);
            queue.pop_front();
        }
        queue.push_back((clock, data));
    }

    pub fn pop_next(&self) -> Option<(Instant, T)> {
        self.0.lock().unwrap().pop_front()
    }

    pub fn pop_latest(&self) -> Option<(Instant, T)> {
        self.0.lock().unwrap().drain(..).last()
    }

    pub fn put_back(&self, clock: Instant, data: T) {
        self.0.lock().unwrap().push_front((clock, data));
    }

    pub fn peek_clock(&self) -> Option<Instant> {
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

    fn write_samples(&mut self, _clock: Instant, _buffer: &[Sample]) {}
}
