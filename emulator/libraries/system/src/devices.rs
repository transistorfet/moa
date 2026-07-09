use std::fmt;
use std::rc::Rc;
use std::cell::{RefCell, RefMut, BorrowMutError};
use std::sync::atomic::{AtomicUsize, Ordering};
use femtos::{Duration, Instant};
use emulator_hal::{BusAccess, Step, Inspect, Debug};

use crate::{Error, System};


/// A universal memory address used by the Addressable trait
pub type Address = u64;

pub type MoaBus = dyn BusAccess<u64, Instant = Instant, Error = Error>;
pub type MoaStep = dyn Step<u64, MoaBus, Error = Error>;
//pub type MoaInspect<'a> = dyn Inspect<u64, &'a mut MoaBus, String>;
//pub type MoaDebug<'a> = dyn Debug<u64, &'a mut MoaBus, String>;

pub trait DeviceInterface {
    #[inline]
    fn as_bus_access(&mut self) -> Option<&mut MoaBus> {
        None
    }

    #[inline]
    fn as_step(&mut self) -> Option<&mut MoaStep> {
        None
    }

    /*
    #[inline]
    fn as_inspect(&mut self) -> Option<&mut MoaInspect> {
        None
    }

    #[inline]
    fn as_debug(&mut self) -> Option<&mut MoaDebug> {
        None
    }
    */
}






static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeviceId(usize);

impl DeviceId {
    pub fn new() -> Self {
        let next = NEXT_ID.load(Ordering::Acquire);
        NEXT_ID.store(next + 1, Ordering::Release);
        Self(next)
    }
}

impl Default for DeviceId {
    fn default() -> Self {
        Self::new()
    }
}

pub type BoxedInterface = Rc<RefCell<Box<dyn DeviceInterface>>>;

#[derive(Clone)]
pub struct Device(DeviceId, BoxedInterface);

impl Device {
    pub fn new<T>(value: T) -> Self
    where
        T: DeviceInterface + 'static,
    {
        Self(DeviceId::new(), Rc::new(RefCell::new(Box::new(value))))
    }

    pub fn id(&self) -> DeviceId {
        self.0
    }

    pub fn borrow_mut(&self) -> RefMut<'_, Box<dyn DeviceInterface>> {
        self.1.borrow_mut()
    }

    pub fn try_borrow_mut(&self) -> Result<RefMut<'_, Box<dyn DeviceInterface>>, BorrowMutError> {
        self.1.try_borrow_mut()
    }
}

impl fmt::Debug for Device {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{:?}", self.0)
    }
}


/*
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeviceId(usize);

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Interrupt {
    Number(usize),
}

pub enum InterruptPriority {
    NonMaskable,
    Number(usize),
}

struct InterruptPort {
    id: usize,
    controller: TransmutableBox,
}

impl InterruptPort {
    fn check_pending(&self) -> Option<Interrupt> {
        self.controller.borrow_mut().as_interrupt_controller().check_pending(self.id)
    }

    fn acknowledge(&self, interrupt: Interrupt) -> Result<(), Error> {
        self.controller.borrow_mut().as_interrupt_controller().acknowledge(self.id, interrupt)
    }
}

//pub trait InterruptPort {
//    fn check_pending(&mut self, id: DeviceId) -> Option<Interrupt>;
//    fn acknowledge(&mut self, id: DeviceId, interrupt: Interrupt) -> Result<(), Error>;
//}

//pub trait Interrupter {
//    fn trigger(&mut self, id: DeviceId, interrupt: Interrupt) -> Result<(), Error>;
//}

struct Interrupter {
    input_id: usize,
    interrupt: Interrupt,
    controller: Rc<RefCell<TransmutableBox>>,
}

pub trait InterruptController {
    fn connect(&mut self, priority: InterruptPriority) -> Result<InterruptPort, Error>;
    fn check_pending(&mut self, id: usize) -> Option<Interrupt>;
    fn acknowledge(&mut self, id: usize, interrupt: Interrupt) -> Result<(), Error>;
}
*/
