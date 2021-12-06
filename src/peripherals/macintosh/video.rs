
use std::sync::{Arc, Mutex};

use crate::error::Error;
use crate::system::System;
use crate::devices::{ClockElapsed, Address, Addressable, Steppable, Transmutable};

use crate::host::gfx::FrameSwapper;
use crate::host::traits::{Host, BlitableSurface};


const SCRN_BASE: u32    = 0x07A700;

pub struct MacVideo {
    pub swapper: Arc<Mutex<FrameSwapper>>,
}

impl MacVideo {
    pub fn create<H: Host>(host: &mut H) -> Result<Self, Error> {
        let swapper = FrameSwapper::new_shared(512, 342);

        host.add_window(FrameSwapper::to_boxed(swapper.clone()))?;

        Ok(Self {
            swapper,
        })
    }
}

pub struct BitIter {
    pub bit: i8,
    pub data: u16,
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
        let mut swapper = self.swapper.lock().unwrap();
        for y in 0..342 {
            for x in 0..(512 / 16) {
                let word = memory.read_beu16((SCRN_BASE + (x * 2) + (y * (512 / 8))) as Address)?;
                swapper.current.blit(x * 16, y, BitIter::new(word), 16, 1);
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

