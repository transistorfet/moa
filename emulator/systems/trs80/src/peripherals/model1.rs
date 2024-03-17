use femtos::{Instant, Duration};

use moa_core::{System, Error, Address, Addressable, Steppable, Transmutable};
use moa_host::{self, Host, HostError, Frame, FrameSender, KeyEvent, EventReceiver};

use super::keymap;
use super::charset::CharacterGenerator;


const DEV_NAME: &str = "model1";
const SCREEN_SIZE: (u32, u32) = (384, 128);


pub struct Model1Keyboard {
    receiver: EventReceiver<KeyEvent>,
    keyboard_mem: [u8; 8],
}

impl Model1Keyboard {
    pub fn new<H, E>(host: &mut H) -> Result<Self, HostError<E>>
    where
        H: Host<Error = E>,
    {
        let (sender, receiver) = moa_host::event_queue();
        host.register_keyboard(sender)?;

        Ok(Self {
            receiver,
            keyboard_mem: [0; 8],
        })
    }
}

impl Addressable for Model1Keyboard {
    fn size(&self) -> usize {
        0x420
    }

    fn read(&mut self, _clock: Instant, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        if (0x20..=0xA0).contains(&addr) {
            let offset = addr - 0x20;
            data[0] = 0;
            if (offset & 0x01) != 0 {
                data[0] |= self.keyboard_mem[0];
            }
            if (offset & 0x02) != 0 {
                data[0] |= self.keyboard_mem[1];
            }
            if (offset & 0x04) != 0 {
                data[0] |= self.keyboard_mem[2];
            }
            if (offset & 0x08) != 0 {
                data[0] |= self.keyboard_mem[3];
            }
            if (offset & 0x10) != 0 {
                data[0] |= self.keyboard_mem[4];
            }
            if (offset & 0x20) != 0 {
                data[0] |= self.keyboard_mem[5];
            }
            if (offset & 0x40) != 0 {
                data[0] |= self.keyboard_mem[6];
            }
            if (offset & 0x80) != 0 {
                data[0] |= self.keyboard_mem[7];
            }
            //info!("{}: read from keyboard {:x} of {:?}", DEV_NAME, addr, data);
        } else {
            log::warn!("{}: !!! unhandled read from {:0x}", DEV_NAME, addr);
        }
        log::debug!("{}: read from register {:x} of {:?}", DEV_NAME, addr, data);
        Ok(())
    }

    fn write(&mut self, _clock: Instant, addr: Address, data: &[u8]) -> Result<(), Error> {
        log::warn!("{}: !!! unhandled write {:0x} to {:0x}", DEV_NAME, data[0], addr);
        Ok(())
    }
}

impl Steppable for Model1Keyboard {
    fn step(&mut self, _system: &System) -> Result<Duration, Error> {
        while let Some(event) = self.receiver.receive() {
            keymap::record_key_press(&mut self.keyboard_mem, event.key, event.state);
        }

        Ok(Duration::from_millis(1))
    }
}

impl Transmutable for Model1Keyboard {
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        Some(self)
    }

    fn as_steppable(&mut self) -> Option<&mut dyn Steppable> {
        Some(self)
    }
}

pub struct Model1Video {
    frame_sender: FrameSender,
    video_mem: [u8; 1024],
}

impl Model1Video {
    pub fn new<H, E>(host: &mut H) -> Result<Self, HostError<E>>
    where
        H: Host<Error = E>,
    {
        let (frame_sender, frame_receiver) = moa_host::frame_queue(SCREEN_SIZE.0, SCREEN_SIZE.1);

        host.add_video_source(frame_receiver)?;

        Ok(Self {
            frame_sender,
            video_mem: [0x20; 1024],
        })
    }
}

impl Steppable for Model1Video {
    fn step(&mut self, system: &System) -> Result<Duration, Error> {
        let mut frame = Frame::new(SCREEN_SIZE.0, SCREEN_SIZE.1, self.frame_sender.encoding());
        for y in 0..16 {
            for x in 0..64 {
                let ch = self.video_mem[x + (y * 64)];
                let iter = CharacterGenerator::new(ch.saturating_sub(0x20) % 64);
                frame.blit((x * 6) as u32, (y * 8) as u32, iter, 6, 8);
            }
        }
        self.frame_sender.add(system.clock, frame);

        Ok(Duration::from_micros(16_630))
    }
}

impl Addressable for Model1Video {
    fn size(&self) -> usize {
        0x400
    }

    fn read(&mut self, _clock: Instant, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        data[0] = self.video_mem[addr as usize];
        log::debug!("{}: read from register {:x} of {:?}", DEV_NAME, addr, data);
        Ok(())
    }

    fn write(&mut self, _clock: Instant, addr: Address, data: &[u8]) -> Result<(), Error> {
        log::debug!("{}: write to register {:x} with {:x}", DEV_NAME, addr, data[0]);
        self.video_mem[addr as usize] = data[0];
        Ok(())
    }
}

impl Transmutable for Model1Video {
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        Some(self)
    }

    fn as_steppable(&mut self) -> Option<&mut dyn Steppable> {
        Some(self)
    }
}
