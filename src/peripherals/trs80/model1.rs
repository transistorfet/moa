
use std::slice::Iter;
use std::sync::{Arc, Mutex};

use crate::error::Error;
use crate::system::System;
use crate::devices::{ClockElapsed, Address, Addressable, Steppable, Transmutable};

use crate::host::keys::Key;
use crate::host::gfx::{Frame, FrameSwapper};
use crate::host::traits::{Host, BlitableSurface, KeyboardUpdater};

use super::keymap;
use super::charset::CharacterGenerator;


const DEV_NAME: &'static str = "model1";

pub struct Model1Peripherals {
    pub swapper: Arc<Mutex<FrameSwapper>>,
    pub keyboard_mem: Arc<Mutex<[u8; 8]>>,
    pub video_mem: [u8; 1024],
}

impl Model1Peripherals {
    pub fn create<H: Host>(host: &mut H) -> Result<Self, Error> {
        let swapper = FrameSwapper::new_shared(384, 128);
        let keyboard_mem = Arc::new(Mutex::new([0; 8]));

        host.add_window(FrameSwapper::to_boxed(swapper.clone()))?;
        host.register_keyboard(Box::new(Model1KeyboardUpdater(keyboard_mem.clone())))?;

        Ok(Self {
            swapper,
            keyboard_mem,
            video_mem: [0; 1024],
        })
    }
}

pub struct Model1KeyboardUpdater(Arc<Mutex<[u8; 8]>>);

impl KeyboardUpdater for Model1KeyboardUpdater {
    fn update_keyboard(&mut self, key: Key, state: bool) {
        println!(">>> {:?}", key);
        keymap::record_key_press(&mut self.0.lock().unwrap(), key, state);
    }
}

impl Steppable for Model1Peripherals {
    fn step(&mut self, system: &System) -> Result<ClockElapsed, Error> {
        let mut swapper = self.swapper.lock().unwrap();
        swapper.current.clear(0);
        for y in 0..16 {
            for x in 0..64 {
                let ch = self.video_mem[x + (y * 64)];
                let iter = CharacterGenerator::new((ch - 0x20) % 64);
                swapper.current.blit((x * 6) as u32, (y * 8) as u32, iter, 6, 8);
            }
        }

        Ok(16_630_000)
    }
}

impl Addressable for Model1Peripherals {
    fn len(&self) -> usize {
        0x820
    }

    fn read(&mut self, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        if addr >= 0x20 && addr <= 0xA0 {
            let offset = addr - 0x20;
            data[0] = 0;
            if (offset & 0x01) != 0 { data[0] |= self.keyboard_mem.lock().unwrap()[0]; }
            if (offset & 0x02) != 0 { data[0] |= self.keyboard_mem.lock().unwrap()[1]; }
            if (offset & 0x04) != 0 { data[0] |= self.keyboard_mem.lock().unwrap()[2]; }
            if (offset & 0x08) != 0 { data[0] |= self.keyboard_mem.lock().unwrap()[3]; }
            if (offset & 0x10) != 0 { data[0] |= self.keyboard_mem.lock().unwrap()[4]; }
            if (offset & 0x20) != 0 { data[0] |= self.keyboard_mem.lock().unwrap()[5]; }
            if (offset & 0x40) != 0 { data[0] |= self.keyboard_mem.lock().unwrap()[6]; }
            if (offset & 0x80) != 0 { data[0] |= self.keyboard_mem.lock().unwrap()[7]; }
            //info!("{}: read from keyboard {:x} of {:?}", DEV_NAME, addr, data);
        } else if addr >= 0x420 && addr <= 0x820 {
            data[0] = self.video_mem[addr as usize - 0x420];
        } else {
            warning!("{}: !!! unhandled read from {:0x}", DEV_NAME, addr);
        }
        debug!("{}: read from register {:x} of {:?}", DEV_NAME, addr, data);
        Ok(())
    }

    fn write(&mut self, addr: Address, data: &[u8]) -> Result<(), Error> {
        debug!("{}: write to register {:x} with {:x}", DEV_NAME, addr, data[0]);
        if addr >= 0x420 && addr < 0x820 {
            self.video_mem[addr as usize - 0x420] = data[0];
        } else {
            warning!("{}: !!! unhandled write {:0x} to {:0x}", DEV_NAME, data[0], addr);
        }
        Ok(())
    }
}

impl Transmutable for Model1Peripherals {
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        Some(self)
    }

    fn as_steppable(&mut self) -> Option<&mut dyn Steppable> {
        Some(self)
    }
}

