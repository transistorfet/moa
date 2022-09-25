
use std::sync::{Arc, Mutex};

use moa_core::{System, Error, ClockElapsed, Address, Addressable, Steppable, Transmutable};
use moa_core::host::gfx::Frame;
use moa_core::host::{Host, BlitableSurface};


const SCRN_BASE: u32    = 0x07A700;

pub struct MacVideo {
    frame: Arc<Mutex<Frame>>,
}

impl MacVideo {
    pub fn create<H: Host>(host: &mut H) -> Result<Self, Error> {
        let frame = Frame::new_shared(512, 342);

        host.add_window(Frame::new_updater(frame.clone()))?;

        Ok(Self {
            frame,
        })
    }
}

pub struct BitIter {
    bit: i8,
    data: u16,
}

impl BitIter {
    pub fn new(data: u16) -> Self {
        Self {
            bit: 15,
            data,
        }
    }
}

impl Iterator for BitIter {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.bit < 0 {
            None
        } else {
            let bit = (self.data & (1 << self.bit)) != 0;
            self.bit -= 1;

            if bit {
                Some(0xC0C0C0)
            } else {
                Some(0)
            }
        }
    }
}

impl Steppable for MacVideo {
    fn step(&mut self, system: &System) -> Result<ClockElapsed, Error> {
        let mut memory = system.get_bus();
        let mut frame = self.frame.lock().unwrap();
        for y in 0..342 {
            for x in 0..(512 / 16) {
                let word = memory.read_beu16((SCRN_BASE + (x * 2) + (y * (512 / 8))) as Address)?;
                frame.blit(x * 16, y, BitIter::new(word), 16, 1);
            }
        }
        Ok(16_600_000)
    }
}

impl Transmutable for MacVideo {
    fn as_steppable(&mut self) -> Option<&mut dyn Steppable> {
        Some(self)
    }
}

