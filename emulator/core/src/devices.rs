use std::rc::Rc;
use std::cell::{RefCell, RefMut, BorrowMutError};
use std::sync::atomic::{AtomicUsize, Ordering};
use femtos::{Duration, Instant};

use crate::{Error, System};


/// A universal memory address used by the Addressable trait
pub type Address = u64;


/// A device that can change state over time.  The `step()` method will be called
/// by the containing `System` when the system clock advances.  If an error occurs
/// with any device, the `on_error()` method will be called to display any state
/// information that might be helpful for debugging.
pub trait Steppable {
    fn step(&mut self, system: &System) -> Result<Duration, Error>;
    fn on_error(&mut self, _system: &System) {}
}

/// A device that can receive an interrupt.  The `interrupt_state_change()` method
/// will be called whenever an interrupt signal changes goes high or low.
pub trait Interruptable {
    //fn interrupt_state_change(&mut self, state: bool, priority: u8, number: u8) -> Result<(), Error>;
}

/// A device that can be addressed to read data from or write data to the device.
pub trait Addressable {
    fn size(&self) -> usize;
    fn read(&mut self, clock: Instant, addr: Address, data: &mut [u8]) -> Result<(), Error>;
    fn write(&mut self, clock: Instant, addr: Address, data: &[u8]) -> Result<(), Error>;

    #[inline]
    fn read_u8(&mut self, clock: Instant, addr: Address) -> Result<u8, Error> {
        let mut data = [0; 1];
        self.read(clock, addr, &mut data)?;
        Ok(data[0])
    }

    #[inline]
    fn read_beu16(&mut self, clock: Instant, addr: Address) -> Result<u16, Error> {
        let mut data = [0; 2];
        self.read(clock, addr, &mut data)?;
        Ok(read_beu16(&data))
    }

    #[inline]
    fn read_leu16(&mut self, clock: Instant, addr: Address) -> Result<u16, Error> {
        let mut data = [0; 2];
        self.read(clock, addr, &mut data)?;
        Ok(read_leu16(&data))
    }

    #[inline]
    fn read_beu32(&mut self, clock: Instant, addr: Address) -> Result<u32, Error> {
        let mut data = [0; 4];
        self.read(clock, addr, &mut data)?;
        Ok(read_beu32(&data))
    }

    #[inline]
    fn read_leu32(&mut self, clock: Instant, addr: Address) -> Result<u32, Error> {
        let mut data = [0; 4];
        self.read(clock, addr, &mut data)?;
        Ok(read_leu32(&data))
    }

    #[inline]
    fn write_u8(&mut self, clock: Instant, addr: Address, value: u8) -> Result<(), Error> {
        let data = [value];
        self.write(clock, addr, &data)
    }

    #[inline]
    fn write_beu16(&mut self, clock: Instant, addr: Address, value: u16) -> Result<(), Error> {
        let mut data = [0; 2];
        write_beu16(&mut data, value);
        self.write(clock, addr, &data)
    }

    #[inline]
    fn write_leu16(&mut self, clock: Instant, addr: Address, value: u16) -> Result<(), Error> {
        let mut data = [0; 2];
        write_leu16(&mut data, value);
        self.write(clock, addr, &data)
    }

    #[inline]
    fn write_beu32(&mut self, clock: Instant, addr: Address, value: u32) -> Result<(), Error> {
        let mut data = [0; 4];
        write_beu32(&mut data, value);
        self.write(clock, addr, &data)
    }

    #[inline]
    fn write_leu32(&mut self, clock: Instant, addr: Address, value: u32) -> Result<(), Error> {
        let mut data = [0; 4];
        write_leu32(&mut data, value);
        self.write(clock, addr, &data)
    }
}

#[inline]
pub fn read_beu16(data: &[u8]) -> u16 {
    (data[0] as u16) << 8 | (data[1] as u16)
}

#[inline]
pub fn read_leu16(data: &[u8]) -> u16 {
    (data[1] as u16) << 8 | (data[0] as u16)
}

#[inline]
pub fn read_beu32(data: &[u8]) -> u32 {
    (data[0] as u32) << 24 | (data[1] as u32) << 16 | (data[2] as u32) << 8 | (data[3] as u32)
}

#[inline]
pub fn read_leu32(data: &[u8]) -> u32 {
    (data[3] as u32) << 24 | (data[2] as u32) << 16 | (data[1] as u32) << 8 | (data[0] as u32)
}



#[inline]
pub fn write_beu16(data: &mut [u8], value: u16) -> &mut [u8] {
    data[0] = (value >> 8) as u8;
    data[1] = value as u8;
    data
}

#[inline]
pub fn write_leu16(data: &mut [u8], value: u16) -> &mut [u8] {
    data[0] = value as u8;
    data[1] = (value >> 8) as u8;
    data
}

#[inline]
pub fn write_beu32(data: &mut [u8], value: u32) -> &mut [u8] {
    data[0] = (value >> 24) as u8;
    data[1] = (value >> 16) as u8;
    data[2] = (value >> 8) as u8;
    data[3] = value as u8;
    data
}

#[inline]
pub fn write_leu32(data: &mut [u8], value: u32) -> &mut [u8] {
    data[0] = value as u8;
    data[1] = (value >> 8) as u8;
    data[2] = (value >> 16) as u8;
    data[3] = (value >> 24) as u8;
    data
}


/// A device (cpu) that can debugged using the built-in debugger
pub trait Debuggable {
    fn add_breakpoint(&mut self, addr: Address);
    fn remove_breakpoint(&mut self, addr: Address);

    fn print_current_step(&mut self, system: &System) -> Result<(), Error>;
    fn print_disassembly(&mut self, system: &System, addr: Address, count: usize);
    fn run_command(&mut self, system: &System, args: &[&str]) -> Result<bool, Error>;
}

/// A device (peripheral) that can inspected using the built-in debugger
pub trait Inspectable {
    fn inspect(&mut self, system: &System, args: &[&str]) -> Result<(), Error>;
}


pub trait Transmutable {
    #[inline]
    fn as_steppable(&mut self) -> Option<&mut dyn Steppable> {
        None
    }

    #[inline]
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        None
    }

    #[inline]
    fn as_interruptable(&mut self) -> Option<&mut dyn Interruptable> {
        None
    }

    #[inline]
    fn as_debuggable(&mut self) -> Option<&mut dyn Debuggable> {
        None
    }

    #[inline]
    fn as_inspectable(&mut self) -> Option<&mut dyn Inspectable> {
        None
    }
}

pub type TransmutableBox = Rc<RefCell<Box<dyn Transmutable>>>;

pub fn wrap_transmutable<T: Transmutable + 'static>(value: T) -> TransmutableBox {
    Rc::new(RefCell::new(Box::new(value)))
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

#[derive(Clone)]
pub struct Device(DeviceId, TransmutableBox);

impl Device {
    pub fn new<T>(value: T) -> Self
    where
        T: Transmutable + 'static,
    {
        Self(DeviceId::new(), wrap_transmutable(value))
    }

    pub fn id(&self) -> DeviceId {
        self.0
    }

    pub fn borrow_mut(&self) -> RefMut<'_, Box<dyn Transmutable>> {
        self.1.borrow_mut()
    }

    pub fn try_borrow_mut(&self) -> Result<RefMut<'_, Box<dyn Transmutable>>, BorrowMutError> {
        self.1.try_borrow_mut()
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
