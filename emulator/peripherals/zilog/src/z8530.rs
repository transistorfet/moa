
use moa_core::{System, Error, ClockTime, ClockDuration, Address, Addressable, Steppable, Transmutable, warn, debug};

const DEV_NAME: &str = "z8530";

#[derive(Default)]
pub struct Z8530 {

}

impl Addressable for Z8530 {
    fn size(&self) -> usize {
        0x10
    }

    fn read(&mut self, _clock: ClockTime, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        warn!("{}: !!! unhandled read from {:0x}", DEV_NAME, addr);
        debug!("{}: read from register {:x} of {:?}", DEV_NAME, addr, data);
        Ok(())
    }

    fn write(&mut self, _clock: ClockTime, addr: Address, data: &[u8]) -> Result<(), Error> {
        debug!("{}: write to register {:x} with {:x}", DEV_NAME, addr, data[0]);
        warn!("{}: !!! unhandled write {:0x} to {:0x}", DEV_NAME, data[0], addr);
        Ok(())
    }
}

impl Steppable for Z8530 {
    fn step(&mut self, _system: &System) -> Result<ClockDuration, Error> {

        Ok(ClockDuration::from_secs(1))
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


