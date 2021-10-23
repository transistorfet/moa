
use std::sync::{Arc, Mutex};

use crate::host::traits::WindowUpdater;


#[derive(Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub bitmap: Vec<u32>,
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
        if self.current.width != width || self.current.height != height {
            self.current.width = width;
            self.current.height = height;
            self.current.bitmap.resize((width * height) as usize, 0);
            self.previous = self.current.clone();
            return;
        }

        for i in 0..(width as usize * height as usize) {
            bitmap[i] = self.current.bitmap[i];
        }
    }
}

pub struct FrameSwapperWrapper(Arc<Mutex<FrameSwapper>>);

impl WindowUpdater for FrameSwapperWrapper {
    fn update_frame(&mut self, width: u32, height: u32, bitmap: &mut [u32]) {
        self.0.lock().map(|mut swapper| swapper.update_frame(width, height, bitmap));
    }
}

