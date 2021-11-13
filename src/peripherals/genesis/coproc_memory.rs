
use crate::error::Error;
use crate::devices::{Address, Addressable, Transmutable};


const DEV_NAME: &'static str = "coprocessor";

pub struct CoprocessorMemory {
    pub bus_request: bool,
    pub reset: bool,
}


impl CoprocessorMemory {
    pub fn new() -> Self {
        CoprocessorMemory {
            bus_request: false,
            reset: false,
        }
    }
}

impl Addressable for CoprocessorMemory {
    fn len(&self) -> usize {
        0x4000
    }

    fn read(&mut self, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        match addr {
            0x100 => {
                data[0] = if self.bus_request && self.reset { 0x01 } else { 0x00 };
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
                    self.bus_request = true;
                } else {
                    self.bus_request = false;
                }
            },
            0x200 => {
                if data[0] == 0 {
                    self.reset = true;
                } else {
                    self.reset = false;
                }
            },
            _ => { warning!("{}: !!! unhandled write {:0x} to {:0x}", DEV_NAME, data[0], addr); },
        }
        Ok(())
    }
}

impl Transmutable for CoprocessorMemory {
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        Some(self)
    }
}


