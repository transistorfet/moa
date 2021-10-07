
use std::rc::Rc;
use std::cell::{RefCell, RefMut};

use crate::error::Error;
use crate::memory::{Address, Addressable, Bus};


pub type Clock = u64;


pub trait Steppable {
    fn step(&mut self, system: &System) -> Result<Clock, Error>;
    fn on_error(&mut self, _system: &System) { }
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

pub fn wrap_interruptable<T: InterruptableDevice + 'static>(value: T) -> InterruptableDeviceBox {
    Rc::new(RefCell::new(Box::new(value)))
}


pub enum Device {
    Addressable(AddressableDeviceBox),
    Interruptable(InterruptableDeviceBox),
}

pub struct System {
    pub clock: Clock,
    pub devices: Vec<Device>,
    pub bus: RefCell<Bus>,
}

impl System {
    pub fn new() -> System {
        System {
            clock: 0,
            devices: vec![],
            bus: RefCell::new(Bus::new()),
        }
    }

    pub fn get_bus(&self) -> RefMut<'_, Bus> {
        self.bus.borrow_mut()
    }

    pub fn add_addressable_device(&mut self, addr: Address, device: AddressableDeviceBox) -> Result<(), Error> {
        let length = device.borrow().len();
        self.bus.borrow_mut().insert(addr, length, device.clone());
        self.devices.push(Device::Addressable(device));
        Ok(())
    }

    pub fn add_interruptable_device(&mut self, device: InterruptableDeviceBox) -> Result<(), Error> {
        //self.bus.borrow_mut().insert(addr, length, device.clone());
        self.devices.push(Device::Interruptable(device));
        Ok(())
    }

    pub fn step(&mut self) -> Result<(), Error> {
        self.clock += 1;
        for dev in &self.devices {
            match dev {
                Device::Addressable(dev) => dev.borrow_mut().step(&self),
                Device::Interruptable(dev) => dev.borrow_mut().step(&self),
            }?;
        }
        Ok(())
    }

    pub fn trigger_interrupt(&self, number: u8) -> Result<(), Error> {
        // TODO how does this find the specific device it's connected to?

        // TODO for the time being, this will find the first device to handle it or fail
        for dev in &self.devices {
            match dev {
                Device::Interruptable(dev) => {
                    return dev.borrow_mut().handle_interrupt(&self, number);
                },
                _ => { },
            }
        }
        return Err(Error::new(&format!("unhandled interrupt: {:x}", number)));
    }

    pub fn exit_error(&mut self) {
        for dev in &self.devices {
            match dev {
                Device::Addressable(dev) => dev.borrow_mut().on_error(&self),
                Device::Interruptable(dev) => dev.borrow_mut().on_error(&self),
            }
        }
    }
}

pub struct InterruptController {
    
}


