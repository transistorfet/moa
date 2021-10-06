
use std::cell::{RefCell, RefMut};

use crate::error::Error;
use crate::memory::{self, Address, Addressable, Bus};


pub type Clock = u64;
pub type DeviceNumber = u8;

pub type DeviceRefMut<'a> = RefMut<'a, Box<dyn AddressableDevice>>;

pub trait Device: Addressable {
    fn step(&mut self, system: &System) -> Result<Clock, Error>;
}

pub trait AddressableDevice: Device + Addressable { }

impl<T: Device + Addressable> AddressableDevice for T { }


pub struct System {
    pub clock: Clock,
    pub devices: Vec<RefCell<Box<dyn AddressableDevice>>>,
    pub bus: Bus,
}

impl System {
    pub fn new() -> System {
        System {
            clock: 0,
            devices: vec![],
            bus: Bus::new(),
        }
    }

    pub fn add_device(&mut self, addr: Address, device: Box<dyn AddressableDevice>) -> Result<(), Error> {
        self.bus.insert(addr, device.len(), self.devices.len() as DeviceNumber);
        self.devices.push(RefCell::new(device));
        Ok(())
    }


    pub fn step(&mut self) -> Result<(), Error> {
        self.clock += 1;
        for dev in &self.devices {
            dev.borrow_mut().step(&self)?;
        }
        Ok(())
    }

    pub fn get_device_in_range(&self, addr: Address, count: usize) -> Result<(DeviceRefMut<'_>, Address), Error> {
        let (dev, relative_addr) = self.bus.get_device_at(addr, count)?;
        Ok((self.devices[dev as usize].borrow_mut(), relative_addr))
    }


    pub fn read(&self, addr: Address, count: usize) -> Result<Vec<u8>, Error> {
        let (dev, relative_addr) = self.bus.get_device_at(addr, count)?;
        self.devices[dev as usize].borrow_mut().read(relative_addr, count)
    }

    pub fn write(&self, addr: Address, data: &[u8]) -> Result<(), Error> {
        let (dev, relative_addr) = self.bus.get_device_at(addr, data.len())?;
        self.devices[dev as usize].borrow_mut().write(relative_addr, data)
    }


    pub fn read_u8(&self, addr: Address) -> Result<u8, Error> {
        Ok(self.read(addr, 1)?[0])
    }

    pub fn read_beu16(&self, addr: Address) -> Result<u16, Error> {
        Ok(memory::read_beu16(&self.read(addr, 2)?))
    }

    pub fn read_beu32(&self, addr: Address) -> Result<u32, Error> {
        Ok(memory::read_beu32(&self.read(addr, 4)?))
    }

    pub fn write_u8(&self, addr: Address, value: u8) -> Result<(), Error> {
        let data = [value];
        self.write(addr, &data)
    }

    pub fn write_beu16(&self, addr: Address, value: u16) -> Result<(), Error> {
        let data = memory::write_beu16(value);
        self.write(addr, &data)
    }

    pub fn write_beu32(&self, addr: Address, value: u32) -> Result<(), Error> {
        let data = memory::write_beu32(value);
        self.write(addr, &data)
    }


    pub fn dump_memory(&self, mut addr: Address, mut count: Address) {
        while count > 0 {
            let mut line = format!("{:#010x}: ", addr);

            let to = if count < 16 { count / 2 } else { 8 };
            for _ in 0..to {
                let word = self.read_beu16(addr);
                if word.is_err() {
                    println!("{}", line);
                    return;
                }
                line += &format!("{:#06x} ", word.unwrap());
                addr += 2;
                count -= 2;
            }
            println!("{}", line);
        }
    }
}

