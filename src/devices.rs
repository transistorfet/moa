
use std::rc::Rc;
use std::cell::RefCell;

use crate::error::Error;
use crate::system::System;


pub const MAX_READ: usize = 4;

pub type Clock = u64;
pub type Address = u64;


/// A device that can change state over time.  The `step()` method will be called
/// by the containing `System` when the system clock advances.  If an error occurs
/// with any device, the `on_error()` method will be called to display any state
/// information that might be helpful for debugging.
pub trait Steppable {
    fn step(&mut self, system: &System) -> Result<Clock, Error>;
    fn on_error(&mut self, _system: &System) { }
    fn on_debug(&mut self) { }
}


/// A device that can receive an interrupt.  The `interrupt_state_change()` method
/// will be called whenever an interrupt signal changes goes high or low.
pub trait Interruptable {
    fn interrupt_state_change(&mut self, state: bool, priority: u8, number: u8) -> Result<(), Error>;
}


/// A device that can be addressed to read data from or write data to the device.
pub trait Addressable {
    fn len(&self) -> usize;
    fn read(&mut self, addr: Address, count: usize) -> Result<[u8; MAX_READ], Error>;
    fn write(&mut self, addr: Address, data: &[u8]) -> Result<(), Error>;

    fn read_u8(&mut self, addr: Address) -> Result<u8, Error> {
        Ok(self.read(addr, 1)?[0])
    }

    fn read_beu16(&mut self, addr: Address) -> Result<u16, Error> {
        Ok(read_beu16(&self.read(addr, 2)?))
    }

    fn read_beu32(&mut self, addr: Address) -> Result<u32, Error> {
        Ok(read_beu32(&self.read(addr, 4)?))
    }

    fn write_u8(&mut self, addr: Address, value: u8) -> Result<(), Error> {
        let data = [value];
        self.write(addr, &data)
    }

    fn write_beu16(&mut self, addr: Address, value: u16) -> Result<(), Error> {
        let data = write_beu16(value);
        self.write(addr, &data)
    }

    fn write_beu32(&mut self, addr: Address, value: u32) -> Result<(), Error> {
        let data = write_beu32(value);
        self.write(addr, &data)
    }
}

#[inline(always)]
pub fn read_beu16(data: &[u8]) -> u16 {
    (data[0] as u16) << 8 |
    (data[1] as u16)
}

#[inline(always)]
pub fn read_beu32(data: &[u8]) -> u32 {
    (data[0] as u32) << 24 |
    (data[1] as u32) << 16 |
    (data[2] as u32) << 8 |
    (data[3] as u32)
}

#[inline(always)]
pub fn write_beu16(value: u16) -> [u8; 2] {
    [
        (value >> 8) as u8,
        value as u8,
    ]
}

#[inline(always)]
pub fn write_beu32(value: u32) -> [u8; 4] {
    [
        (value >> 24) as u8,
        (value >> 16) as u8,
        (value >> 8) as u8,
        value as u8,
    ]
}

pub trait Transmutable {
    fn as_steppable(&mut self) -> Option<&mut dyn Steppable> {
        None
    }

    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        None
    }

    fn as_interruptable(&mut self) -> Option<&mut dyn Interruptable> {
        None
    }
}

pub type TransmutableBox = Rc<RefCell<Box<dyn Transmutable>>>;

pub fn wrap_transmutable<T: Transmutable + 'static>(value: T) -> TransmutableBox {
    Rc::new(RefCell::new(Box::new(value)))
}

