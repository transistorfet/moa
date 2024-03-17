use std::sync::{Arc, Mutex};
use femtos::Instant;

use crate::traits::ClockedQueue;

pub const MASK_COLOUR: u32 = 0xFFFFFFFF;

#[derive(Copy, Clone, Default, Debug, PartialEq, Eq)]
pub enum PixelEncoding {
    #[default]
    RGBA,
    ARGB,
    ABGR,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Pixel {
    Rgb(u8, u8, u8),
    Rgba(u8, u8, u8, u8),
    Mask,
}

impl Pixel {
    #[inline]
    pub fn encode(self, encoding: PixelEncoding) -> u32 {
        let (r, g, b, a) = match self {
            Pixel::Rgb(r, g, b) => (r as u32, g as u32, b as u32, 255),
            Pixel::Rgba(r, g, b, a) => (r as u32, g as u32, b as u32, a as u32),
            Pixel::Mask => return MASK_COLOUR,
        };

        match encoding {
            PixelEncoding::RGBA => (r << 24) | (g << 16) | (b << 8) | a,
            PixelEncoding::ARGB => (a << 24) | (r << 16) | (g << 8) | b,
            PixelEncoding::ABGR => (a << 24) | (b << 16) | (g << 8) | r,
        }
    }
}

#[derive(Clone, Default)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub encoding: PixelEncoding,
    pub bitmap: Vec<u32>,
}

impl Frame {
    pub fn new(width: u32, height: u32, encoding: PixelEncoding) -> Self {
        Self {
            width,
            height,
            encoding,
            bitmap: vec![0; (width * height) as usize],
        }
    }

    pub fn new_shared(width: u32, height: u32, encoding: PixelEncoding) -> Arc<Mutex<Frame>> {
        Arc::new(Mutex::new(Frame::new(width, height, encoding)))
    }

    pub fn set_size(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.bitmap.resize((width * height) as usize, 0);
    }

    #[inline]
    pub fn set_pixel(&mut self, pos_x: u32, pos_y: u32, pixel: Pixel) {
        match pixel {
            Pixel::Mask => {},
            value if pos_x < self.width && pos_y < self.height => {
                self.bitmap[(pos_x + (pos_y * self.width)) as usize] = value.encode(self.encoding);
            },
            _ => {},
        }
    }

    #[inline]
    pub fn set_encoded_pixel(&mut self, pos_x: u32, pos_y: u32, pixel: u32) {
        match pixel {
            MASK_COLOUR => {},
            value if pos_x < self.width && pos_y < self.height => {
                self.bitmap[(pos_x + (pos_y * self.width)) as usize] = value;
            },
            _ => {},
        }
    }

    pub fn blit<B: Iterator<Item = Pixel>>(&mut self, pos_x: u32, pos_y: u32, mut bitmap: B, width: u32, height: u32) {
        for y in pos_y..(pos_y + height) {
            for x in pos_x..(pos_x + width) {
                match bitmap.next().unwrap() {
                    Pixel::Mask => {},
                    value if x < self.width && y < self.height => {
                        self.bitmap[(x + (y * self.width)) as usize] = value.encode(self.encoding);
                    },
                    _ => {},
                }
            }
        }
    }

    pub fn clear(&mut self, value: Pixel) {
        let value = value.encode(self.encoding);
        self.bitmap.iter_mut().for_each(|pixel| *pixel = value);
    }
}

pub fn frame_queue(width: u32, height: u32) -> (FrameSender, FrameReceiver) {
    let sender = FrameSender {
        encoding: Arc::new(Mutex::new(PixelEncoding::RGBA)),
        queue: ClockedQueue::new(10),
    };

    let receiver = FrameReceiver {
        max_size: (width, height),
        encoding: sender.encoding.clone(),
        queue: sender.queue.clone(),
    };

    (sender, receiver)
}

pub struct FrameSender {
    encoding: Arc<Mutex<PixelEncoding>>,
    queue: ClockedQueue<Frame>,
}

impl FrameSender {
    pub fn encoding(&self) -> PixelEncoding {
        *self.encoding.lock().unwrap()
    }

    pub fn add(&self, clock: Instant, frame: Frame) {
        self.queue.push(clock, frame);
    }
}

pub struct FrameReceiver {
    max_size: (u32, u32),
    encoding: Arc<Mutex<PixelEncoding>>,
    queue: ClockedQueue<Frame>,
}

impl FrameReceiver {
    pub fn max_size(&self) -> (u32, u32) {
        self.max_size
    }

    pub fn request_encoding(&self, encoding: PixelEncoding) {
        *self.encoding.lock().unwrap() = encoding;
    }

    pub fn latest(&self) -> Option<(Instant, Frame)> {
        self.queue.pop_latest()
    }
}
