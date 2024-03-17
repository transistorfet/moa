use femtos::{Instant, Duration};

use moa_core::{System, Error, Address, Addressable, Steppable, Transmutable};


//const CA0: u8           = 0x01;
//const CA1: u8           = 0x02;
//const CA2: u8           = 0x04;
//const LSTRB: u8         = 0x08;
const ENABLE: u8 = 0x10;
//const SELECT: u8        = 0x20;
const Q6: u8 = 0x40;
const Q7: u8 = 0x80;

const DEV_NAME: &str = "iwm";

#[derive(Default)]
pub struct IWM {
    state: u8,
    mode: u8,
    handshake: u8,
}

impl IWM {
    pub fn flip_switches(&mut self, addr: Address) {
        let mask = 1 << (addr >> 1);

        if (addr & 0x01) != 0 {
            self.state |= mask;
        } else {
            self.state &= !mask;
        }
        log::info!("{}: state is now {:x}", DEV_NAME, self.state);
    }
}

impl Addressable for IWM {
    fn size(&self) -> usize {
        0x10
    }

    fn read(&mut self, _clock: Instant, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        self.flip_switches(addr);

        if (addr & 0x01) != 0 {
            data[0] = 0xFF;
            return Ok(());
        }

        let i = data.len() - 1;
        match self.state & (Q7 | Q6) {
            0 => {
                // read data register
                data[0] = 0xFF;
            },
            Q6 => {
                // read "status" register
                data[i] = (self.mode & 0x1F) | if (self.state & ENABLE) != 0 { 0x20 } else { 0x00 };
            },
            Q7 => {
                // read "write-handshake" register
                data[i] = 0x3F | self.handshake;
            },
            b if b == (Q7 | Q6) => {
                panic!("");
            },
            _ => {
                log::warn!("{}: !!! unhandled read of {:0x} with state {:x}", DEV_NAME, addr, self.state);
            },
        }
        log::info!("{}: read from register {:x} of {:?}", DEV_NAME, addr, data);
        Ok(())
    }

    fn write(&mut self, _clock: Instant, addr: Address, data: &[u8]) -> Result<(), Error> {
        self.flip_switches(addr);

        log::info!("{}: write to register {:x} with {:x}", DEV_NAME, addr, data[0]);

        let i = data.len() - 1;
        match self.state & (Q7 | Q6 | ENABLE) {
            b if b == (Q7 | Q6 | ENABLE) => {
                self.handshake &= !0x80;
            },
            b if b == (Q7 | Q6) => {
                // write the mode register
                self.mode = data[i] & 0x1f;
            },
            _ => {
                log::warn!("{}: !!! unhandled write {:0x} to {:0x}", DEV_NAME, data[0], addr);
            },
        }
        Ok(())
    }
}

impl Steppable for IWM {
    fn step(&mut self, _system: &System) -> Result<Duration, Error> {
        // TODO implement
        Ok(Duration::from_secs(1))
    }
}

impl Transmutable for IWM {
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        Some(self)
    }

    fn as_steppable(&mut self) -> Option<&mut dyn Steppable> {
        Some(self)
    }
}
