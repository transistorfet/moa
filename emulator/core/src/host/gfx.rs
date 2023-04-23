use std::sync::{Arc, Mutex};

use crate::host::traits::{BlitableSurface, ClockedQueue, WindowUpdater};
use crate::ClockTime;
use crate::Error;

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
            PixelEncoding::RGBA =>
                ((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | (a as u32),
            PixelEncoding::ARGB =>
                ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32),
            PixelEncoding::ABGR =>
                ((a as u32) << 24) | ((b as u32) << 16) | ((g as u32) << 8) | (r as u32),
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
}

impl BlitableSurface for Frame {
    fn set_size(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.bitmap.resize((width * height) as usize, 0);
    }

    fn set_pixel(&mut self, pos_x: u32, pos_y: u32, pixel: Pixel) {
        match pixel {
            Pixel::Mask => {}
            value if pos_x < self.width && pos_y < self.height => {
                self.bitmap[(pos_x + (pos_y * self.width)) as usize] = value.encode(self.encoding);
            }
            _ => {}
        }
    }

    #[inline]
    fn set_encoded_pixel(&mut self, pos_x: u32, pos_y: u32, pixel: u32) {
        match pixel {
            MASK_COLOUR => { },
            value if pos_x < self.width && pos_y < self.height => {
                self.bitmap[(pos_x + (pos_y * self.width)) as usize] = value;
            },
            _ => { },
        }
    }

    fn blit<B: Iterator<Item = Pixel>>(&mut self, pos_x: u32, pos_y: u32, mut bitmap: B, width: u32, height: u32) {
        for y in pos_y..(pos_y + height) {
            for x in pos_x..(pos_x + width) {
                match bitmap.next().unwrap() {
                    Pixel::Mask => {}
                    value if x < self.width && y < self.height => {
                        self.bitmap[(x + (y * self.width)) as usize] = value.encode(self.encoding);
                    }
                    _ => {}
                }
            }
        }
    }

    fn clear(&mut self, value: Pixel) {
        let value = value.encode(self.encoding);
        self.bitmap.iter_mut().for_each(|pixel| *pixel = value);
    }
}

#[derive(Clone)]
pub struct FrameQueue {
    max_size: (u32, u32),
    encoding: Arc<Mutex<PixelEncoding>>,
    queue: ClockedQueue<Frame>,
}

impl FrameQueue {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            max_size: (width, height),
            encoding: Arc::new(Mutex::new(PixelEncoding::RGBA)),
            queue: Default::default(),
        }
    }

    pub fn encoding(&mut self) -> PixelEncoding {
        *self.encoding.lock().unwrap()
    }

    pub fn add(&self, clock: ClockTime, frame: Frame) {
        self.queue.push(clock, frame);
    }

    pub fn latest(&self) -> Option<(ClockTime, Frame)> {
        self.queue.pop_latest()
    }
}

impl WindowUpdater for FrameQueue {
    fn max_size(&self) -> (u32, u32) {
        self.max_size
    }

    fn request_encoding(&mut self, encoding: PixelEncoding) {
        *self.encoding.lock().unwrap() = encoding;
    }

    fn take_frame(&mut self) -> Result<Frame, Error> {
        self.latest()
            .map(|(_, f)| f)
            .ok_or_else(|| Error::new("No frame available"))
    }
}
