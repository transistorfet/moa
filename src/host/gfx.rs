
use std::sync::{Arc, Mutex};

use crate::host::traits::{WindowUpdater, BlitableSurface};


#[derive(Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub bitmap: Vec<u32>,
}

impl BlitableSurface for Frame {
    fn set_size(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.bitmap.resize((width * height) as usize, 0);
    }

    fn blit<B: Iterator<Item=u32>>(&mut self, pos_x: u32, mut pos_y: u32, mut bitmap: B, width: u32, height: u32) {
        for y in pos_y..(pos_y + height) {
            for x in pos_x..(pos_x + width) {
                match bitmap.next().unwrap() {
                    0 => { },
                    value if x < self.width && y < self.height => { self.bitmap[(x + (y * self.width)) as usize] = value; },
                    _ => { },
                }
            }
        }
    }
}


pub struct FrameSwapper {
    pub current: Frame,
    pub previous: Frame,
}

impl FrameSwapper {
    pub fn new() -> FrameSwapper {
        FrameSwapper {
            current: Frame { width: 0, height: 0, bitmap: vec![] },
            previous: Frame { width: 0, height: 0, bitmap: vec![] },
        }
    }

    pub fn new_shared() -> Arc<Mutex<FrameSwapper>> {
        Arc::new(Mutex::new(FrameSwapper::new()))
    }

    pub fn to_boxed(swapper: Arc<Mutex<FrameSwapper>>) -> Box<dyn WindowUpdater> {
        Box::new(FrameSwapperWrapper(swapper))
    }
}

impl WindowUpdater for FrameSwapper {
    fn update_frame(&mut self, width: u32, height: u32, bitmap: &mut [u32]) {
        std::mem::swap(&mut self.current, &mut self.previous);

        for y in 0..self.current.height {
            for x in 0..self.current.width {
                bitmap[(x + (y * width)) as usize] = self.current.bitmap[(x + (y * self.current.width)) as usize];
            }
        }
    }
}

pub struct FrameSwapperWrapper(Arc<Mutex<FrameSwapper>>);

impl WindowUpdater for FrameSwapperWrapper {
    fn update_frame(&mut self, width: u32, height: u32, bitmap: &mut [u32]) {
        self.0.lock().map(|mut swapper| swapper.update_frame(width, height, bitmap));
    }
}

