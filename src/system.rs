
use std::rc::Rc;
use std::cell::{RefCell, RefMut};
use std::collections::HashMap;

use crate::error::Error;
use crate::memory::Bus;
use crate::interrupts::InterruptController;
use crate::devices::{Clock, ClockElapsed, Address, TransmutableBox};


pub struct System {
    pub clock: Clock,
    pub devices: Vec<TransmutableBox>,
    pub event_queue: Vec<SteppableDevice>,
    pub bus: Rc<RefCell<Bus>>,
    pub interrupt_controller: RefCell<InterruptController>,
}

impl System {
    pub fn new() -> System {
        System {
            clock: 0,
            devices: vec![],
            event_queue: vec![],
            bus: Rc::new(RefCell::new(Bus::new())),
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
        self.try_queue_device(device.clone());
        self.devices.push(device);
        Ok(())
    }

    pub fn add_interruptable_device(&mut self, device: TransmutableBox) -> Result<(), Error> {
        self.interrupt_controller.borrow_mut().set_target(device.clone())?;
        self.try_queue_device(device.clone());
        self.devices.push(device);
        Ok(())
    }

    pub fn step(&mut self) -> Result<(), Error> {
        let mut event_device = self.event_queue.pop().unwrap();
        self.clock = event_device.next_clock;
        event_device.next_clock = self.clock + event_device.device.borrow_mut().as_steppable().unwrap().step(&self)?;
        self.queue_device(event_device);
        Ok(())
    }

    pub fn run_for(&mut self, clocks: Clock) -> Result<(), Error> {
        let target = self.clock + clocks;

        while self.clock < target {
            match self.step() {
                Ok(()) => { }
                Err(err) => {
                    self.exit_error();
                    println!("{:?}", err);
                    return Err(err);
                },
            }
        }
        Ok(())
    }

    pub fn run_loop(&mut self) {
        self.run_for(u64::MAX);
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


    fn try_queue_device(&mut self, device: TransmutableBox) {
        if device.borrow_mut().as_steppable().is_some() {
            self.queue_device(SteppableDevice::new(device));
        }
    }

    fn queue_device(&mut self, event_device: SteppableDevice) {
        for i in (0..self.event_queue.len()).rev() {
            if self.event_queue[i].next_clock > event_device.next_clock {
                self.event_queue.insert(i + 1, event_device);
                return;
            }
        }
        self.event_queue.insert(0, event_device);
    }
}


pub struct SteppableDevice {
    pub next_clock: Clock,
    pub device: TransmutableBox,
}

impl SteppableDevice {
    pub fn new(device: TransmutableBox) -> Self {
        Self {
            next_clock: 0,
            device,
        }
    }
}

