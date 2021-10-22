
use std::cell::{RefCell, RefMut};

use crate::error::Error;
use crate::memory::Bus;
use crate::interrupts::InterruptController;
use crate::devices::{Clock, Address, TransmutableBox};


pub struct System {
    pub clock: Clock,
    pub devices: Vec<TransmutableBox>,
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


    pub fn add_addressable_device(&mut self, addr: Address, device: TransmutableBox) -> Result<(), Error> {
        let length = device.borrow_mut().as_addressable().unwrap().len();
        self.bus.borrow_mut().insert(addr, length, device.clone());
        self.devices.push(device);
        Ok(())
    }

    pub fn add_interruptable_device(&mut self, device: TransmutableBox) -> Result<(), Error> {
        self.interrupt_controller.borrow_mut().set_target(device.clone())?;
        self.devices.push(device);
        Ok(())
    }

    pub fn step(&mut self) -> Result<(), Error> {
        self.clock += 1;
        for dev in &self.devices {
            match dev.borrow_mut().as_steppable() {
                Some(dev) => { dev.step(&self)?; },
                None => { },
            }
        }
        Ok(())
    }

    pub fn exit_error(&mut self) {
        for dev in &self.devices {
            match dev.borrow_mut().as_steppable() {
                Some(dev) => dev.on_error(&self),
                None => { },
            }
        }
    }

    pub fn debug(&mut self) -> Result<(), Error> {
        for dev in &self.devices {
            match dev.borrow_mut().as_steppable() {
                Some(dev) => dev.on_debug(),
                None => { },
            }
        }
        Ok(())
    }

    pub fn run_loop(&mut self) {
        loop {
            match self.step() {
                Ok(()) => { },
                Err(err) => {
                    self.exit_error();
                    println!("{:?}", err);
                    break;
                },
            }
        }
    }

    pub fn run_for(&mut self, clocks: Clock) -> Result<(), Error> {
        let target = self.clock + clocks;
        while self.clock < target {
            match self.step() {
                Ok(()) => { },
                Err(err) => {
                    self.exit_error();
                    println!("{:?}", err);
                    return Err(err);
                },
            }
        }
        Ok(())
    }
}

