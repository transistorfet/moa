
use std::rc::Rc;
use std::cell::{RefCell, RefMut};

use crate::error::Error;
use crate::memory::{self, Address, Addressable, Bus};


pub type Clock = u64;
pub type DeviceNumber = u8;


pub trait Steppable {
    fn step(&mut self, system: &System) -> Result<Clock, Error>;
}

pub trait Interruptable {
    fn handle_interrupt(&mut self, system: &System, number: u8) -> Result<(), Error>;
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


pub struct System {
    pub clock: Clock,
    pub addressable_devices: Vec<AddressableDeviceBox>,
    pub bus: RefCell<Bus>,
}

impl System {
    pub fn new() -> System {
        System {
            clock: 0,
            addressable_devices: vec![],
            bus: RefCell::new(Bus::new()),
        }
    }

    pub fn get_bus(&self) -> RefMut<'_, Bus> {
        self.bus.borrow_mut()
    }

    pub fn add_addressable_device(&mut self, addr: Address, device: AddressableDeviceBox) -> Result<(), Error> {
        let length = device.borrow().len();
        self.bus.borrow_mut().insert(addr, length, device.clone());
        self.addressable_devices.push(device);
        Ok(())
    }

    pub fn step(&mut self) -> Result<(), Error> {
        self.clock += 1;
        for dev in &self.addressable_devices {
            dev.borrow_mut().step(&self)?;
        }
        Ok(())
    }
}

