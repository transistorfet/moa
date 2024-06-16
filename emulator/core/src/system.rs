use std::rc::Rc;
use std::cell::{RefCell, RefMut};
use std::collections::{BTreeMap, HashMap};
use femtos::{Instant, Duration};

use crate::devices::{downcast_rc_refc, get_next_device_id, Device, DeviceId, DynDevice};
use crate::{wrap_device, Address, Bus, Error, InterruptController};


pub struct System {
    pub clock: Instant,
    pub devices: BTreeMap<DeviceId, Device>,
    pub event_queue: Vec<NextStep>,
    pub id_to_name: HashMap<DeviceId, String>,

    pub debuggables: Vec<DeviceId>,

    pub bus: Rc<RefCell<Bus>>,
    pub buses: HashMap<String, Rc<RefCell<Bus>>>,
    pub interrupt_controller: RefCell<InterruptController>,
}


impl Default for System {
    fn default() -> Self {
        Self {
            clock: Instant::START,
            devices: BTreeMap::new(),
            event_queue: vec![],
            id_to_name: HashMap::new(),

            debuggables: Vec::new(),

            bus: Rc::new(RefCell::new(Bus::default())),
            buses: HashMap::new(),
            interrupt_controller: RefCell::new(InterruptController::default()),
        }
    }
}

pub struct DeviceSettings {
    pub name: Option<String>,
    pub address: Option<Address>,
    pub debuggable: bool,
    pub queue: bool,
}

impl Default for DeviceSettings {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceSettings {
    fn new() -> Self {
        Self {
            name: None,
            address: None,
            debuggable: false,
            queue: false,
        }
    }

    fn with_address(&mut self, addr: Address) -> &mut Self {
        self.address = Some(addr);
        self
    }

    fn with_name(&mut self, name: String) -> &mut Self {
        self.name = Some(name);
        self
    }

    fn debuggable(&mut self, debuggable: bool) -> &mut Self {
        self.debuggable = debuggable;
        self
    }

    fn queue(&mut self, queue: bool) -> &mut Self {
        self.queue = queue;
        self
    }
}

impl System {
    pub fn get_bus(&self) -> RefMut<'_, Bus> {
        self.bus.borrow_mut()
    }

    pub fn get_interrupt_controller(&self) -> RefMut<'_, InterruptController> {
        self.interrupt_controller.borrow_mut()
    }

    pub fn get_id_from_name(&self, name: &str) -> Option<DeviceId> {
        self.id_to_name.iter().find_map(|(key, &ref val)| if val == name { Some(*key)} else { None })
    }

    pub fn get_device<T: DynDevice>(&self, device: DeviceId) -> Result<Rc<RefCell<T>>, Error> {
        self.devices
            .get(&device)
            .and_then(|rc| downcast_rc_refc::<T>(rc).inspect_err(|e| panic!("{:?}", e)).ok())
            .cloned()
            .ok_or_else(|| Error::new(format!("system: bad device id {}", device)))
    }

    pub fn get_device_by_name<T: DynDevice>(&self, name: &str) -> Result<Rc<RefCell<T>>, Error> {
        if let Some(id) = self.get_id_from_name(name) {
            self.get_device(id)
        } else {
            Err(Error::new(format!("system: could not find device  {}", name)))
        }
    }

    pub fn get_dyn_device(&self, device: DeviceId) -> Result<Device, Error> {
        self.devices
            .get(&device)
            .cloned()
            .ok_or_else(|| Error::new(format!("system: bad device id {}", device)))
    }

    pub fn get_dyn_device_by_name(&self, name: &str) -> Result<Device, Error> {
        if let Some(id) = self.get_id_from_name(name) {
            self.get_dyn_device(id)
        } else {
            Err(Error::new(format!("system: could not find device  {}", name)))
        }
    }

    pub fn add_device<T: DynDevice>(&mut self, device: T, settings: DeviceSettings) -> Result<DeviceId, Error> {
        self.add_device_rc_dyn(wrap_device(device), settings)
    }

    pub fn add_device_rc_dyn(&mut self, device: Device, settings: DeviceSettings) -> Result<DeviceId, Error> {
        let id = get_next_device_id();
        self.id_to_name.insert(id, settings.name.unwrap_or_default());

        self.devices.insert(id, device.clone());

        if settings.debuggable && device.borrow_mut().as_debuggable().is_some() {
            self.debuggables.push(id);
        }
        if settings.queue && device.borrow_mut().as_steppable().is_some() {
            self.queue_device(NextStep::new(id));
        }
        if let Some(addr) = settings.address {
            self.bus.borrow_mut().insert(addr, device.clone());
        }
        Ok(id)
    }

    pub fn add_named_device<T: DynDevice>(&mut self, name: &str, device: T) -> Result<DeviceId, Error> {
        self.add_named_device_rc_dyn(name, wrap_device(device))
    }

    pub fn add_named_device_rc_dyn(&mut self, name: &str, device: Device) -> Result<DeviceId, Error> {
        self.add_device_rc_dyn(device, DeviceSettings {
            name: Some(name.to_owned()),
            queue: true,
            ..Default::default()
        })
    }

    pub fn add_addressable_device<T: DynDevice>(&mut self, addr: Address, device: T) -> Result<DeviceId, Error> {
        self.add_addressable_device_rc_dyn(addr, wrap_device(device))
    }

    pub fn add_addressable_device_rc_dyn(&mut self, addr: Address, device: Device) -> Result<DeviceId, Error> {
        self.add_device_rc_dyn(device, DeviceSettings {
            name: Some(format!("mem{:x}", addr)),
            address: Some(addr),
            queue: true,
            ..Default::default()
        })
    }
    

    pub fn add_peripheral<T: DynDevice>(&mut self, name: &str, addr: Address, device: T) -> Result<DeviceId, Error> {
        self.add_peripheral_rc_dyn(name, addr, wrap_device(device))
    }

    pub fn add_peripheral_rc_dyn(&mut self, name: &str, addr: Address, device: Device) -> Result<DeviceId, Error> {
        self.add_device_rc_dyn(device, DeviceSettings {
            name: Some(name.to_owned()),
            address: Some(addr),
            queue: true,
            ..Default::default()
        })
    }

    pub fn add_interruptable_device<T: DynDevice>(&mut self, name: &str, device: T) -> Result<DeviceId, Error> {
        self.add_interruptable_device_rc_dyn(name, wrap_device(device))
    }

    pub fn add_interruptable_device_rc_dyn(&mut self, name: &str, device: Device) -> Result<DeviceId, Error> {
        self.add_device_rc_dyn(device, DeviceSettings {
            name: Some(name.to_owned()),
            queue: true,
            ..Default::default()
        })
    }

    fn process_one_event(&mut self) -> Result<(), Error> {
        let mut event_device = self.event_queue.pop().unwrap();
        self.clock = event_device.next_clock;
        let result = match self.get_dyn_device(event_device.device).unwrap().borrow_mut().as_steppable().unwrap().step(self) {
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
    pub fn step_until_device(&mut self, device: DeviceId) -> Result<(), Error> {
        loop {
            self.step()?;

            if self.get_next_event_device_id() == device {
                break;
            }
        }
        Ok(())
    }

    /// Step through the simulation until the next event scheduled is for a debuggable device
    pub fn step_until_debuggable(&mut self) -> Result<(), Error> {
        loop {
            self.step()?;

            if self.get_dyn_device(self.get_next_event_device_id()).unwrap().borrow_mut().as_debuggable().is_some() {
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

    pub fn get_next_event_device_id(&self) -> DeviceId {
        self.event_queue[self.event_queue.len() - 1].device
    }

    pub fn get_next_debuggable_device(&self) -> Option<DeviceId> {
        for event in self.event_queue.iter().rev() {
            if self.get_dyn_device(event.device).unwrap().borrow_mut().as_debuggable().is_some() {
                return Some(event.device);
            }
        }
        None
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
    pub device: DeviceId,
}

impl NextStep {
    pub fn new(device: DeviceId) -> Self {
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
