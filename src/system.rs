
use std::rc::Rc;
use std::cell::{Cell, RefCell, RefMut};
use std::collections::HashMap;

use crate::memory::Bus;
use crate::debugger::Debugger;
use crate::error::{Error, ErrorType};
use crate::interrupts::InterruptController;
use crate::devices::{Clock, ClockElapsed, Address, TransmutableBox};


pub struct System {
    pub clock: Clock,
    pub devices: HashMap<String, TransmutableBox>,
    pub event_queue: Vec<NextStep>,

    pub debug_enabled: Cell<bool>,
    pub debugger: RefCell<Debugger>,

    pub bus: Rc<RefCell<Bus>>,
    pub interrupt_controller: RefCell<InterruptController>,
}

impl System {
    pub fn new() -> System {
        System {
            clock: 0,
            devices: HashMap::new(),
            event_queue: vec![],

            debug_enabled: Cell::new(false),
            debugger: RefCell::new(Debugger::new()),

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

    pub fn add_device(&mut self, name: &str, device: TransmutableBox) -> Result<(), Error> {
        self.try_queue_device(device.clone());
        self.devices.insert(name.to_string(), device);
        Ok(())
    }

    pub fn add_addressable_device(&mut self, addr: Address, device: TransmutableBox) -> Result<(), Error> {
        self.add_peripheral(&format!("mem{:x}", addr), addr, device)
    }

    pub fn add_peripheral(&mut self, name: &str, addr: Address, device: TransmutableBox) -> Result<(), Error> {
        self.bus.borrow_mut().insert(addr, device.clone());
        self.try_queue_device(device.clone());
        self.devices.insert(name.to_string(), device);
        Ok(())
    }

    pub fn add_interruptable_device(&mut self, name: &str, device: TransmutableBox) -> Result<(), Error> {
        self.interrupt_controller.borrow_mut().set_target(device.clone())?;
        self.try_queue_device(device.clone());
        self.devices.insert(name.to_string(), device);
        Ok(())
    }

    pub fn enable_debugging(&self) {
        self.debug_enabled.set(true);
        self.debugger.borrow_mut().breakpoint_occurred();
    }

    pub fn disable_debugging(&self) {
        self.debug_enabled.set(false);
    }

    pub fn step(&mut self) -> Result<(), Error> {
        let mut event_device = self.event_queue.pop().unwrap();
        self.clock = event_device.next_clock;
        let result = match event_device.device.borrow_mut().as_steppable().unwrap().step(&self) {
            Ok(diff) => {
                event_device.next_clock = self.clock + diff;
                Ok(())
            },
            Err(err) => Err(err),
        };
        self.queue_device(event_device);
        result
    }

    pub fn run_for(&mut self, elapsed: ClockElapsed) -> Result<(), Error> {
        let target = self.clock + elapsed;

        while self.clock < target {
            if self.debug_enabled.get() && self.event_queue[self.event_queue.len() - 1].device.borrow_mut().as_debuggable().is_some() {
                self.debugger.borrow_mut().run_debugger(&self, self.event_queue[self.event_queue.len() - 1].device.clone());
            }

            match self.step() {
                Ok(()) => { }
                Err(err) if err.err == ErrorType::Breakpoint => {
                    println!("Breakpoint reached: {}", err.msg);
                    self.enable_debugging();
                },
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
        self.run_for(u64::MAX).unwrap();
    }

    pub fn exit_error(&mut self) {
        for (_, dev) in self.devices.iter() {
            match dev.borrow_mut().as_steppable() {
                Some(dev) => dev.on_error(&self),
                None => { },
            }
        }
    }


    fn try_queue_device(&mut self, device: TransmutableBox) {
        if device.borrow_mut().as_steppable().is_some() {
            self.queue_device(NextStep::new(device));
        }
    }

    fn queue_device(&mut self, device_step: NextStep) {
        for i in (0..self.event_queue.len()).rev() {
            if self.event_queue[i].next_clock > device_step.next_clock {
                self.event_queue.insert(i + 1, device_step);
                return;
            }
        }
        self.event_queue.insert(0, device_step);
    }
}


pub struct NextStep {
    pub next_clock: Clock,
    pub device: TransmutableBox,
}

impl NextStep {
    pub fn new(device: TransmutableBox) -> Self {
        Self {
            next_clock: 0,
            device,
        }
    }
}

