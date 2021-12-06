
use crate::error::Error;
use crate::system::System;
use crate::devices::{ClockElapsed, Address, Addressable, Steppable, Transmutable};

const DEV_NAME: &'static str = "sn76489";

pub struct SN76489 {

}

impl SN76489 {
    pub fn new() -> Self {
        Self {

        }
    }
}

impl Addressable for SN76489 {
    fn len(&self) -> usize {
        0x01
    }

    fn read(&mut self, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        match addr {
            _ => {
                warning!("{}: !!! unhandled read from {:0x}", DEV_NAME, addr);
            },
        }
        debug!("{}: read from register {:x} of {:?}", DEV_NAME, addr, data);
        Ok(())
    }

    fn write(&mut self, addr: Address, data: &[u8]) -> Result<(), Error> {
        debug!("{}: write to register {:x} with {:x}", DEV_NAME, addr, data[0]);
        match addr {
            _ => {
                warning!("{}: !!! unhandled write {:0x} to {:0x}", DEV_NAME, data[0], addr);
            },
        }
        Ok(())
    }
}


impl Transmutable for SN76489 {
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        Some(self)
    }

    //fn as_steppable(&mut self) -> Option<&mut dyn Steppable> {
    //    Some(self)
    //}
}


