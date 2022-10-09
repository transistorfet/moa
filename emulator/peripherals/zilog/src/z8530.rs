
use moa_core::{System, Error, ClockElapsed, Address, Addressable, Steppable, Transmutable, warn, debug};

const DEV_NAME: &'static str = "z8530";

pub struct Z8530 {

}

impl Z8530 {
    pub fn new() -> Self {
        Self {

        }
    }
}

impl Addressable for Z8530 {
    fn len(&self) -> usize {
        0x10
    }

    fn read(&mut self, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        match addr {
            _ => {
                warn!("{}: !!! unhandled read from {:0x}", DEV_NAME, addr);
            },
        }
        debug!("{}: read from register {:x} of {:?}", DEV_NAME, addr, data);
        Ok(())
    }

    fn write(&mut self, addr: Address, data: &[u8]) -> Result<(), Error> {
        debug!("{}: write to register {:x} with {:x}", DEV_NAME, addr, data[0]);
        match addr {
            _ => {
                warn!("{}: !!! unhandled write {:0x} to {:0x}", DEV_NAME, data[0], addr);
            },
        }
        Ok(())
    }
}

impl Steppable for Z8530 {
    fn step(&mut self, _system: &System) -> Result<ClockElapsed, Error> {

        Ok(1_000_000_00)
    }
}

impl Transmutable for Z8530 {
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        Some(self)
    }

    fn as_steppable(&mut self) -> Option<&mut dyn Steppable> {
        Some(self)
    }
}


