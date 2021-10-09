
use std::rc::Rc;
use std::cell::{RefCell, RefMut};

use crate::error::Error;
use crate::system::System;
use crate::memory::{Addressable};


pub type Clock = u64;


pub trait Steppable {
    fn step(&mut self, system: &System) -> Result<Clock, Error>;
    fn on_error(&mut self, _system: &System) { }
}

pub trait Interruptable {
    fn interrupt_state_change(&mut self, system: &System, state: bool, priority: u8, number: u8) -> Result<(), Error>;
}

pub trait AddressableDevice: Addressable + Steppable { }
pub trait InterruptableDevice: Interruptable + Steppable { }

impl<T: Addressable + Steppable> AddressableDevice for T { }
impl<T: Interruptable + Steppable> InterruptableDevice for T { }

pub type AddressableDeviceBox = Rc<RefCell<Box<dyn AddressableDevice>>>;
pub type InterruptableDeviceBox = Rc<RefCell<Box<dyn InterruptableDevice>>>;

pub type AddressableDeviceRefMut<'a> = RefMut<'a, Box<dyn AddressableDevice>>;

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


