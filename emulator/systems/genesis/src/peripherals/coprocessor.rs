use std::rc::Rc;
use std::cell::{Cell, RefCell};
use std::any::Any;
use femtos::Instant;

use moa_core::{Bus, Device, Error, Address, Addressable, Signal, Transmutable};
//use moa_signals::Signal;
use moa_z80::Z80;

const DEV_NAME: &str = "coprocessor";

pub struct CoprocessorCoordinator {
    z80: Device,
    //bus_request: Signal<bool>,
    //reset: Signal<bool>,
}


impl CoprocessorCoordinator {
    pub fn new(z80: Device) -> Self {
        Self {
            z80,
            //bus_request,
            //reset,
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
                let mut device = self.z80.borrow_mut();
                let z80 = device.as_signalable().unwrap();
                data[0] = if z80.signal(Signal::BusRequest).unwrap_or(false) && z80.signal(Signal::Reset).unwrap_or(false) {
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
                let mut device = self.z80.borrow_mut();
                let z80 = device.as_signalable().unwrap();
                z80.set_signal(Signal::BusRequest, data[0] == 0)?;
            },
            0x200 => {
                let mut device = self.z80.borrow_mut();
                let z80 = device.as_signalable().unwrap();
                z80.set_signal(Signal::Reset, data[0] == 0)?;
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
