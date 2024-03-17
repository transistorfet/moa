use std::rc::Rc;
use std::cell::{Cell, RefCell};
use femtos::Instant;

use moa_core::{Bus, Error, Address, Addressable, Transmutable};
use moa_signals::Signal;

const DEV_NAME: &str = "coprocessor";

pub struct CoprocessorCoordinator {
    bus_request: Signal<bool>,
    reset: Signal<bool>,
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
    fn size(&self) -> usize {
        0x4000
    }

    fn read(&mut self, _clock: Instant, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        match addr {
            0x100 => {
                data[0] = if self.bus_request.get() && self.reset.get() {
                    0x01
                } else {
                    0x00
                };
            },
            _ => {
                log::warn!("{}: !!! unhandled read from {:0x}", DEV_NAME, addr);
            },
        }
        log::info!("{}: read from register {:x} of {:?}", DEV_NAME, addr, data);
        Ok(())
    }

    fn write(&mut self, _clock: Instant, addr: Address, data: &[u8]) -> Result<(), Error> {
        log::info!("{}: write to register {:x} with {:x}", DEV_NAME, addr, data[0]);
        match addr {
            0x000 => { /* ROM vs DRAM mode */ },
            0x100 => {
                self.bus_request.set(data[0] != 0);
            },
            0x200 => {
                self.reset.set(data[0] == 0);
            },
            _ => {
                log::warn!("{}: !!! unhandled write {:0x} to {:0x}", DEV_NAME, data[0], addr);
            },
        }
        Ok(())
    }
}

impl Transmutable for CoprocessorCoordinator {
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        Some(self)
    }
}


type CoprocessorRegister = Rc<Cell<Address>>;

pub struct CoprocessorBankRegister {
    base: CoprocessorRegister,
}

impl Addressable for CoprocessorBankRegister {
    fn size(&self) -> usize {
        0x01
    }

    fn read(&mut self, _clock: Instant, _addr: Address, _data: &mut [u8]) -> Result<(), Error> {
        Ok(())
    }

    fn write(&mut self, _clock: Instant, _addr: Address, data: &[u8]) -> Result<(), Error> {
        let value = ((self.base.get() >> 1) | ((data[0] as Address) << 23)) & 0xFF8000;
        //let value = ((self.base.get() << 1) | ((data[0] as Address) << 15)) & 0xFF8000;
        println!("New base is {:x}", value);
        self.base.set(value);
        Ok(())
    }
}

impl Transmutable for CoprocessorBankRegister {
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        Some(self)
    }
}


pub struct CoprocessorBankArea {
    base: CoprocessorRegister,
    bus: Rc<RefCell<Bus>>,
}

impl CoprocessorBankArea {
    pub fn new(bus: Rc<RefCell<Bus>>) -> (Self, CoprocessorBankRegister) {
        let base = Rc::new(Cell::new(0));
        let register = CoprocessorBankRegister {
            base: base.clone(),
        };
        let bank = Self {
            base,
            bus,
        };
        (bank, register)
    }
}

impl Addressable for CoprocessorBankArea {
    fn size(&self) -> usize {
        0x8000
    }

    fn read(&mut self, clock: Instant, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        self.bus.borrow_mut().read(clock, self.base.get() + addr, data)
    }

    fn write(&mut self, clock: Instant, addr: Address, data: &[u8]) -> Result<(), Error> {
        self.bus.borrow_mut().write(clock, self.base.get() + addr, data)
    }
}

impl Transmutable for CoprocessorBankArea {
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        Some(self)
    }
}
