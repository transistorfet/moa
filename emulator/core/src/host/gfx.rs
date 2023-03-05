use std::mem;
use std::sync::{Arc, Mutex};

use crate::host::traits::{BlitableSurface, ClockedQueue, WindowUpdater};
use crate::Clock;
use crate::Error;

pub const MASK_COLOUR: u32 = 0xFFFFFFFF;

#[derive(Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub bitmap: Vec<u32>,
}

impl Frame {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            bitmap: vec![0; (width * height) as usize],
        }
    }

    pub fn new_shared(width: u32, height: u32) -> Arc<Mutex<Frame>> {
        Arc::new(Mutex::new(Frame::new(width, height)))
    }
}

impl BlitableSurface for Frame {
    fn set_size(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.bitmap.resize((width * height) as usize, 0);
    }

    fn set_pixel(&mut self, pos_x: u32, pos_y: u32, pixel: u32) {
        match pixel {
            MASK_COLOUR => {}
            value if pos_x < self.width && pos_y < self.height => {
                self.bitmap[(pos_x + (pos_y * self.width)) as usize] = value;
            }
            _ => {}
        }
    }

    fn blit<B: Iterator<Item = u32>>(&mut self, pos_x: u32, pos_y: u32, mut bitmap: B, width: u32, height: u32) {
        /*
                (pos_y..(pos_y + height))
                    .for_each(|y| {
                        self.bitmap[(y * self.width) as usize .. (y * self.width + self.width) as usize]
                            .iter_mut()
                            .for_each(|pixel|
                                match bitmap.next().unwrap() {
                                    MASK_COLOUR => {},
                                    value => *pixel = value,
                                }
                            )
                    });
        */

        for y in pos_y..(pos_y + height) {
            for x in pos_x..(pos_x + width) {
                match bitmap.next().unwrap() {
                    MASK_COLOUR => {}
                    value if x < self.width && y < self.height => {
                        self.bitmap[(x + (y * self.width)) as usize] = value;
                    }
                    _ => {}
                }
            }
        }
    }

    fn clear(&mut self, value: u32) {
        let value = if value == MASK_COLOUR { 0 } else { value };
        self.bitmap.iter_mut().for_each(|pixel| *pixel = value);
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

    pub fn set_size(&mut self, width: u32, height: u32) {
        self.previous.lock().unwrap().set_size(width, height);
        self.current.lock().unwrap().set_size(width, height);
    }
}

impl WindowUpdater for FrameSwapper {
    fn max_size(&mut self) -> (u32, u32) {
        let frame = self.current.lock().unwrap();
        (frame.width, frame.height)
    }

    fn take_frame(&mut self) -> Result<Frame, Error> {
        let mut previous = self.previous.lock().map_err(|_| Error::new("Lock error"))?;
        let mut frame = Frame::new(previous.width, previous.height);
        mem::swap(&mut *previous, &mut frame);
        Ok(frame)
    }
}

#[derive(Clone)]
pub struct FrameQueue {
    max_size: (u32, u32),
    queue: ClockedQueue<Frame>,
}

impl FrameQueue {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            max_size: (width, height),
            queue: ClockedQueue::new(),
        }
    }

    pub fn add(&self, clock: Clock, frame: Frame) {
        self.queue.push(clock, frame);
    }

    pub fn latest(&self) -> Option<(Clock, Frame)> {
        self.queue.pop_latest()
    }
}

impl WindowUpdater for FrameQueue {
    fn max_size(&mut self) -> (u32, u32) {
        self.max_size
    }

    fn take_frame(&mut self) -> Result<Frame, Error> {
        self.latest()
            .map(|(_, f)| f)
            .ok_or_else(|| Error::new("No frame available"))
    }
}
