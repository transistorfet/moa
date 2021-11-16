
use crate::error::Error;
use crate::signals::Signal;
use crate::devices::{Address, Addressable, Transmutable};


const DEV_NAME: &'static str = "coprocessor";

pub struct CoprocessorCoordinator {
    pub bus_request: Signal<bool>,
    pub reset: Signal<bool>,
}


impl CoprocessorCoordinator {
    pub fn new(reset: Signal<bool>, bus_request: Signal<bool>) -> Self {
        Self {
            bus_request,
            reset,
        }
    }
}

impl Addressable for CoprocessorCoordinator {
    fn len(&self) -> usize {
        0x4000
    }

    fn read(&mut self, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        match addr {
            0x100 => {
                data[0] = if self.bus_request.get() && self.reset.get() { 0x01 } else { 0x00 };
            },
            _ => { warning!("{}: !!! unhandled read from {:0x}", DEV_NAME, addr); },
        }
        info!("{}: read from register {:x} of {:?}", DEV_NAME, addr, data);
        Ok(())
    }

    fn write(&mut self, addr: Address, data: &[u8]) -> Result<(), Error> {
        info!("{}: write to register {:x} with {:x}", DEV_NAME, addr, data[0]);
        match addr {
            0x000 => { /* ROM vs DRAM mode */ },
            0x100 => {
                if data[0] != 0 {
                    self.bus_request.set(true);
                } else {
                    self.bus_request.set(false);
                }
            },
            0x200 => {
                if data[0] == 0 {
                    self.reset.set(true);
                } else {
                    self.reset.set(false);
                }
            },
            _ => { warning!("{}: !!! unhandled write {:0x} to {:0x}", DEV_NAME, data[0], addr); },
        }
        Ok(())
    }
}

impl Transmutable for CoprocessorCoordinator {
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        Some(self)
    }
}


