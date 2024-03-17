use std::rc::Rc;
use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
use femtos::{Instant, Duration};

use crate::{Bus, Error, InterruptController, Address, Device};


pub struct System {
    pub clock: Instant,
    pub devices: HashMap<String, Device>,
    pub event_queue: Vec<NextStep>,

    pub debuggables: Vec<Device>,

    pub bus: Rc<RefCell<Bus>>,
    pub buses: HashMap<String, Rc<RefCell<Bus>>>,
    pub interrupt_controller: RefCell<InterruptController>,
}

impl Default for System {
    fn default() -> Self {
        Self {
            clock: Instant::START,
            devices: HashMap::new(),
            event_queue: vec![],

            debuggables: Vec::new(),

            bus: Rc::new(RefCell::new(Bus::default())),
            buses: HashMap::new(),
            interrupt_controller: RefCell::new(InterruptController::default()),
        }
    }
}

impl System {
    pub fn get_bus(&self) -> RefMut<'_, Bus> {
        self.bus.borrow_mut()
    }

    pub fn get_interrupt_controller(&self) -> RefMut<'_, InterruptController> {
        self.interrupt_controller.borrow_mut()
    }

    pub fn get_device(&self, name: &str) -> Result<Device, Error> {
        self.devices
            .get(name)
            .cloned()
            .ok_or_else(|| Error::new(format!("system: no device named {}", name)))
    }

    pub fn add_device(&mut self, name: &str, device: Device) -> Result<(), Error> {
        self.try_add_debuggable(device.clone());
        self.try_queue_device(device.clone());
        self.devices.insert(name.to_string(), device);
        Ok(())
    }

    pub fn add_addressable_device(&mut self, addr: Address, device: Device) -> Result<(), Error> {
        self.add_peripheral(&format!("mem{:x}", addr), addr, device)
    }

    pub fn add_peripheral(&mut self, name: &str, addr: Address, device: Device) -> Result<(), Error> {
        self.bus.borrow_mut().insert(addr, device.clone());
        self.try_add_debuggable(device.clone());
        self.try_queue_device(device.clone());
        self.devices.insert(name.to_string(), device);
        Ok(())
    }

    pub fn add_interruptable_device(&mut self, name: &str, device: Device) -> Result<(), Error> {
        self.try_add_debuggable(device.clone());
        self.try_queue_device(device.clone());
        self.devices.insert(name.to_string(), device);
        Ok(())
    }

    fn process_one_event(&mut self) -> Result<(), Error> {
        let mut event_device = self.event_queue.pop().unwrap();
        self.clock = event_device.next_clock;
        let result = match event_device.device.borrow_mut().as_steppable().unwrap().step(self) {
            Ok(diff) => {
                event_device.next_clock = self.clock.checked_add(diff).unwrap();
                Ok(())
            },
            Err(err) => Err(err),
        };
        self.queue_device(event_device);
        result
    }

    /// Step the simulation one event exactly
    pub fn step(&mut self) -> Result<(), Error> {
        match self.process_one_event() {
            Ok(()) => {},
            Err(err @ Error::Breakpoint(_)) => {
                return Err(err);
            },
            Err(err) => {
                self.exit_error();
                log::error!("{:?}", err);
                return Err(err);
            },
        }
        Ok(())
    }

    /// Step through the simulation until the next event is for the given device
    pub fn step_until_device(&mut self, device: Device) -> Result<(), Error> {
        loop {
            self.step()?;

            if self.get_next_event_device().id() == device.id() {
                break;
            }
        }
        Ok(())
    }

    /// Step through the simulation until the next event scheduled is for a debuggable device
    pub fn step_until_debuggable(&mut self) -> Result<(), Error> {
        loop {
            self.step()?;

            if self.get_next_event_device().borrow_mut().as_debuggable().is_some() {
                break;
            }
        }
        Ok(())
    }

    /// Run the simulation until the given simulation clock time has been reached
    pub fn run_until_clock(&mut self, clock: Instant) -> Result<(), Error> {
        while self.clock < clock {
            self.step()?;
        }
        Ok(())
    }

    /// Run the simulation for `elapsed` amount of simulation time
    pub fn run_for_duration(&mut self, elapsed: Duration) -> Result<(), Error> {
        let target = self.clock + elapsed;

        while self.clock < target {
            self.step()?;
        }
        Ok(())
    }

    /// Run the simulation forever, or until there is an error
    pub fn run_forever(&mut self) -> Result<(), Error> {
        self.run_until_clock(Instant::FOREVER)
    }

    pub fn exit_error(&mut self) {
        for (_, dev) in self.devices.iter() {
            if let Some(dev) = dev.borrow_mut().as_steppable() {
                dev.on_error(self);
            }
        }
    }

    pub fn get_next_event_device(&self) -> Device {
        self.event_queue[self.event_queue.len() - 1].device.clone()
    }

    pub fn get_next_debuggable_device(&self) -> Option<Device> {
        for event in self.event_queue.iter().rev() {
            if event.device.borrow_mut().as_debuggable().is_some() {
                return Some(event.device.clone());
            }
        }
        None
    }

    fn try_add_debuggable(&mut self, device: Device) {
        if device.borrow_mut().as_debuggable().is_some() {
            self.debuggables.push(device);
        }
    }

    fn try_queue_device(&mut self, device: Device) {
        if device.borrow_mut().as_steppable().is_some() {
            self.queue_device(NextStep::new(device));
        }
    }

    fn queue_device(&mut self, device_step: NextStep) {
        for (i, event) in self.event_queue.iter().enumerate().rev() {
            if event.next_clock > device_step.next_clock {
                self.event_queue.insert(i + 1, device_step);
                return;
            }
        }
        self.event_queue.insert(0, device_step);
    }
}


pub struct NextStep {
    pub next_clock: Instant,
    pub device: Device,
}

impl NextStep {
    pub fn new(device: Device) -> Self {
        Self {
            next_clock: Instant::START,
            device,
        }
    }
}

/*
use emulator_hal::bus::{BusType, BusAccess};

impl BusType for System {
    type Address = u64;
    type Error = Error;
    type Instant = Instant;
}

impl BusAccess for System {
    fn read(&mut self, _now: Instant, addr: u64, data: &mut [u8]) -> Result<usize, Self::Error> {
        let addr = addr as usize;
        data.copy_from_slice(&self.0[addr..addr + data.len()]);
        Ok(data.len())
    }

    fn write(&mut self, _now: Instant, addr: u64, data: &[u8]) -> Result<usize, Self::Error> {
        let addr = addr as usize;
        self.0[addr..addr + data.len()].copy_from_slice(data);
        Ok(data.len())
    }
}
*/
