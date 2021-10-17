
use std::cell::{RefCell, RefMut};

use crate::error::Error;
use crate::memory::Bus;
use crate::interrupts::InterruptController;
use crate::devices::{Address, Device, AddressableDeviceBox, InterruptableDeviceBox, Clock};


pub struct System {
    pub clock: Clock,
    pub devices: Vec<Device>,
    pub bus: RefCell<Bus>,
    pub interrupt_controller: RefCell<InterruptController>,
}

impl System {
    pub fn new() -> System {
        System {
            clock: 0,
            devices: vec![],
            bus: RefCell::new(Bus::new()),
            interrupt_controller: RefCell::new(InterruptController::new()),
        }
    }

    pub fn get_bus(&self) -> RefMut<'_, Bus> {
        self.bus.borrow_mut()
    }

    pub fn get_interrupt_controller(&self) -> RefMut<'_, InterruptController> {
        self.interrupt_controller.borrow_mut()
    }


    pub fn add_addressable_device(&mut self, addr: Address, device: AddressableDeviceBox) -> Result<(), Error> {
        let length = device.borrow().len();
        self.bus.borrow_mut().insert(addr, length, device.clone());
        self.devices.push(Device::Addressable(device));
        Ok(())
    }

    pub fn add_interruptable_device(&mut self, device: InterruptableDeviceBox) -> Result<(), Error> {
        self.interrupt_controller.borrow_mut().set_target(device.clone())?;
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

    pub fn exit_error(&mut self) {
        for dev in &self.devices {
            match dev {
                Device::Addressable(dev) => dev.borrow_mut().on_error(&self),
                Device::Interruptable(dev) => dev.borrow_mut().on_error(&self),
            }
        }
    }

    pub fn debug(&mut self) -> Result<(), Error> {
        for dev in &self.devices {
            match dev {
                Device::Addressable(dev) => dev.borrow_mut().on_debug(),
                Device::Interruptable(dev) => dev.borrow_mut().on_debug(),
            }
        }
        Ok(())
    }
}

