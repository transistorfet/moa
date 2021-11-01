
use std::rc::Rc;
use std::cell::RefCell;

use crate::error::Error;
use crate::system::System;


pub const MAX_READ: usize = 4;

/// The time in nanoseconds that have elapsed since the start of the simulation
pub type Clock = u64;

/// The time in nanoseconds until the `step()` method should be called again
pub type ClockElapsed = u64;

/// A universal memory address used by the Addressable trait
pub type Address = u64;


/// A device that can change state over time.  The `step()` method will be called
/// by the containing `System` when the system clock advances.  If an error occurs
/// with any device, the `on_error()` method will be called to display any state
/// information that might be helpful for debugging.
pub trait Steppable {
    fn step(&mut self, system: &System) -> Result<ClockElapsed, Error>;
    fn on_error(&mut self, _system: &System) { }
}

/// A device that can receive an interrupt.  The `interrupt_state_change()` method
/// will be called whenever an interrupt signal changes goes high or low.
pub trait Interruptable {
    //fn interrupt_state_change(&mut self, state: bool, priority: u8, number: u8) -> Result<(), Error>;
}

/// A device that can debugged using the built-in debugger
pub trait Debuggable {
    fn add_breakpoint(&mut self, addr: Address);
    fn remove_breakpoint(&mut self, addr: Address);

    fn print_current_step(&mut self, system: &System) -> Result<(), Error>;
    fn print_disassembly(&mut self, addr: Address, count: usize);
    fn execute_command(&mut self, system: &System, args: &[&str]) -> Result<bool, Error>;
}

/// A device that can be addressed to read data from or write data to the device.
pub trait Addressable {
    fn len(&self) -> usize;
    fn read(&mut self, addr: Address, data: &mut [u8]) -> Result<(), Error>;
    fn write(&mut self, addr: Address, data: &[u8]) -> Result<(), Error>;

    fn read_u8(&mut self, addr: Address) -> Result<u8, Error> {
        let mut data = [0; 1];
        self.read(addr, &mut data)?;
        Ok(data[0])
    }

    fn read_beu16(&mut self, addr: Address) -> Result<u16, Error> {
        let mut data = [0; 2];
        self.read(addr, &mut data)?;
        Ok(read_beu16(&data))
    }

    fn read_beu32(&mut self, addr: Address) -> Result<u32, Error> {
        let mut data = [0; 4];
        self.read(addr, &mut data)?;
        Ok(read_beu32(&data))
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

    fn as_debuggable(&mut self) -> Option<&mut dyn Debuggable> {
        None
    }
}

pub type TransmutableBox = Rc<RefCell<Box<dyn Transmutable>>>;

pub fn wrap_transmutable<T: Transmutable + 'static>(value: T) -> TransmutableBox {
    Rc::new(RefCell::new(Box::new(value)))
}

