
use std::rc::Rc;
use std::cell::{RefCell, RefMut};

use crate::error::Error;
use crate::interrupts::Signal;
use crate::memory::{Address, Addressable, Bus};
use crate::devices::{Device, Steppable, AddressableDeviceBox, InterruptableDeviceBox, Clock};


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

    pub fn change_interrupt_state(&self, state: bool, priority: u8, number: u8) -> Result<(), Error> {
        // TODO how does this find the specific device it's connected to?

        // TODO for the time being, this will find the first device to handle it or fail
        println!("system: interrupt state changed to {} ({})", state, priority);
        for dev in &self.devices {
            match dev {
                Device::Interruptable(dev) => {
                    return dev.borrow_mut().interrupt_state_change(&self, state, priority, number);
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

