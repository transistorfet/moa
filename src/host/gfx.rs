
use std::sync::{Arc, Mutex};

use crate::host::traits::{WindowUpdater, BlitableSurface};


#[derive(Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub bitmap: Vec<u32>,
}

impl Frame {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height, bitmap: vec![0; (width * height) as usize] }
    }

    pub fn new_shared(width: u32, height: u32) -> Arc<Mutex<Frame>> {
        Arc::new(Mutex::new(Frame::new(width, height)))
    }

    pub fn new_updater(frame: Arc<Mutex<Frame>>) -> Box<dyn WindowUpdater> {
        Box::new(FrameUpdateWrapper(frame))
    }
}


impl BlitableSurface for Frame {
    fn set_size(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.bitmap.resize((width * height) as usize, 0);
    }

    fn blit<B: Iterator<Item=u32>>(&mut self, pos_x: u32, pos_y: u32, mut bitmap: B, width: u32, height: u32) {
        for y in pos_y..(pos_y + height) {
            for x in pos_x..(pos_x + width) {
                match bitmap.next().unwrap() {
                    0xFFFFFFFF => { },
                    value if x < self.width && y < self.height => { self.bitmap[(x + (y * self.width)) as usize] = value; },
                    _ => { },
                }
            }
        }
    }

    fn clear(&mut self, value: u32) {
        let value = if value == 0xFFFFFFFF { 0 } else { value };
        for i in 0..((self.width as usize) * (self.height as usize)) {
            self.bitmap[i] = value;
        }
    }
}

pub struct FrameUpdateWrapper(Arc<Mutex<Frame>>);

impl WindowUpdater for FrameUpdateWrapper {
    fn get_size(&mut self) -> (u32, u32) {
        match self.0.lock() {
            Ok(frame) => (frame.width, frame.height),
            _ => (0, 0),
        }
    }

    fn update_frame(&mut self, width: u32, _height: u32, bitmap: &mut [u32]) {
        if let Ok(frame) = self.0.lock() {
            for y in 0..frame.height {
                for x in 0..frame.width {
                    bitmap[(x + (y * width)) as usize] = frame.bitmap[(x + (y * frame.width)) as usize];
                }
            }
        }
    }
}


#[derive(Clone)]
pub struct FrameSwapper {
    pub current: Arc<Mutex<Frame>>,
    pub previous: Arc<Mutex<Frame>>,
}

impl FrameSwapper {
    pub fn new(width: u32, height: u32) -> FrameSwapper {
        FrameSwapper {
            current: Arc::new(Mutex::new(Frame::new(width, height))),
            previous: Arc::new(Mutex::new(Frame::new(width, height))),
        }
    }

    pub fn to_boxed(swapper: FrameSwapper) -> Box<dyn WindowUpdater> {
        Box::new(swapper)
    }

    pub fn swap(&mut self) {
        std::mem::swap(&mut self.current.lock().unwrap().bitmap, &mut self.previous.lock().unwrap().bitmap);
    }
}

impl WindowUpdater for FrameSwapper {
    fn get_size(&mut self) -> (u32, u32) {
        if let Ok(frame) = self.current.lock() {
            (frame.width, frame.height)
        } else {
            (0, 0)
        }
    }

    fn update_frame(&mut self, width: u32, _height: u32, bitmap: &mut [u32]) {
        if let Ok(frame) = self.previous.lock() {
            for y in 0..frame.height {
                for x in 0..frame.width {
                    bitmap[(x + (y * width)) as usize] = frame.bitmap[(x + (y * frame.width)) as usize];
                }
            }
        }
    }
}

