use femtos::Duration;

use moa_core::{System, Error, Address, Addressable, Steppable, Transmutable};
use moa_host::{self, Host, HostError, Frame, FrameSender, Pixel};


const SCRN_BASE: u32 = 0x07A700;
const SCRN_SIZE: (u32, u32) = (512, 342);

pub struct MacVideo {
    frame_sender: FrameSender,
}

impl MacVideo {
    pub fn new<H, E>(host: &mut H) -> Result<Self, HostError<E>>
    where
        H: Host<Error = E>,
    {
        let (frame_sender, frame_receiver) = moa_host::frame_queue(SCRN_SIZE.0, SCRN_SIZE.1);

        host.add_video_source(frame_receiver)?;

        Ok(Self {
            frame_sender,
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
    type Item = Pixel;

    fn next(&mut self) -> Option<Self::Item> {
        if self.bit < 0 {
            None
        } else {
            let bit = (self.data & (1 << self.bit)) != 0;
            self.bit -= 1;

            if bit {
                Some(Pixel::Rgb(0xC0, 0xC0, 0xC0))
            } else {
                Some(Pixel::Rgb(0, 0, 0))
            }
        }
    }
}

impl Steppable for MacVideo {
    fn step(&mut self, system: &System) -> Result<Duration, Error> {
        let mut memory = system.get_bus();
        let mut frame = Frame::new(SCRN_SIZE.0, SCRN_SIZE.1, self.frame_sender.encoding());
        for y in 0..SCRN_SIZE.1 {
            for x in 0..(SCRN_SIZE.0 / 16) {
                let word = memory.read_beu16(system.clock, (SCRN_BASE + (x * 2) + (y * (SCRN_SIZE.0 / 8))) as Address)?;
                frame.blit(x * 16, y, BitIter::new(word), 16, 1);
            }
        }

        self.frame_sender.add(system.clock, frame);
        Ok(Duration::from_micros(16_600))
    }
}

impl Transmutable for MacVideo {
    fn as_steppable(&mut self) -> Option<&mut dyn Steppable> {
        Some(self)
    }
}
