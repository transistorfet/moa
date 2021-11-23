
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


pub struct FrameSwapper {
    pub current: Frame,
    //pub previous: Frame,
}

impl FrameSwapper {
    pub fn new(width: u32, height: u32) -> FrameSwapper {
        FrameSwapper {
            current: Frame { width, height, bitmap: vec![0; (width * height) as usize] },
            //previous: Frame { width, height, bitmap: vec![0; (width * height) as usize] },
        }
    }

    pub fn new_shared(width: u32, height: u32) -> Arc<Mutex<FrameSwapper>> {
        Arc::new(Mutex::new(FrameSwapper::new(width, height)))
    }

    pub fn to_boxed(swapper: Arc<Mutex<FrameSwapper>>) -> Box<dyn WindowUpdater> {
        Box::new(FrameSwapperWrapper(swapper))
    }
}

impl WindowUpdater for FrameSwapper {
    fn get_size(&mut self) -> (u32, u32) {
        (self.current.width, self.current.height)
    }

    fn update_frame(&mut self, width: u32, _height: u32, bitmap: &mut [u32]) {
        //std::mem::swap(&mut self.current, &mut self.previous);

        for y in 0..self.current.height {
            for x in 0..self.current.width {
                bitmap[(x + (y * width)) as usize] = self.current.bitmap[(x + (y * self.current.width)) as usize];
            }
        }
    }
}

pub struct FrameSwapperWrapper(Arc<Mutex<FrameSwapper>>);

impl WindowUpdater for FrameSwapperWrapper {
    fn get_size(&mut self) -> (u32, u32) {
        self.0.lock().map(|mut swapper| swapper.get_size()).unwrap_or((0, 0))
    }

    fn update_frame(&mut self, width: u32, height: u32, bitmap: &mut [u32]) {
        if let Ok(mut swapper) = self.0.lock() {
            swapper.update_frame(width, height, bitmap);
        }
    }
}

