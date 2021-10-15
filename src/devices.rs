
use std::rc::Rc;
use std::cell::{RefCell, RefMut};

use crate::error::Error;
use crate::system::System;
use crate::memory::{Addressable};


pub type Clock = u64;

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


pub trait AddressableDevice: Addressable + Steppable { }
pub trait InterruptableDevice: Interruptable + Steppable { }

pub type AddressableDeviceBox = Rc<RefCell<Box<dyn AddressableDevice>>>;
pub type InterruptableDeviceBox = Rc<RefCell<Box<dyn InterruptableDevice>>>;

pub type AddressableDeviceRefMut<'a> = RefMut<'a, Box<dyn AddressableDevice>>;

impl<T: Addressable + Steppable> AddressableDevice for T { }
impl<T: Interruptable + Steppable> InterruptableDevice for T { }

pub fn wrap_addressable<T: AddressableDevice + 'static>(value: T) -> AddressableDeviceBox {
    Rc::new(RefCell::new(Box::new(value)))
}

pub fn wrap_interruptable<T: InterruptableDevice + 'static>(value: T) -> InterruptableDeviceBox {
    Rc::new(RefCell::new(Box::new(value)))
}


pub enum Device {
    Addressable(AddressableDeviceBox),
    Interruptable(InterruptableDeviceBox),
}


