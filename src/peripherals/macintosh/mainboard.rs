
use std::rc::Rc;
use std::cell::RefCell;

use crate::memory::Bus;
use crate::error::Error;
use crate::system::System;
use crate::signals::Observable;
use crate::devices::{Clock, ClockElapsed, Address, Addressable, Steppable, Transmutable, TransmutableBox, wrap_transmutable};

use crate::peripherals::z8530::Z8530;
use crate::peripherals::mos6522::Mos6522;
use crate::peripherals::macintosh::iwm::IWM;

const DEV_NAME: &'static str = "mac";


pub struct Mainboard {
    lower_bus: Rc<RefCell<Bus>>,
    scc1: Z8530,
    scc2: Z8530,
    iwm: IWM,
    via: Mos6522,
    phase_read: PhaseRead,
    last_sec: Clock,
}

impl Mainboard {
    pub fn create(ram: TransmutableBox, rom: TransmutableBox) -> Result<Self, Error> {
        let scc1 = Z8530::new();
        let scc2 = Z8530::new();
        let iwm = IWM::new();
        let via = Mos6522::new();
        let phase_read = PhaseRead::new();

        let lower_bus = Rc::new(RefCell::new(Bus::new()));

        let mainboard = Self {
            lower_bus: lower_bus.clone(),
            scc1,
            scc2,
            iwm,
            via,
            phase_read,
            last_sec: 0,
        };

        mainboard.via.port_a.set_observer(move |port| {
            if (port.data & 0x10) == 0 {
                println!("{}: overlay is 0 (normal)", DEV_NAME);
                lower_bus.borrow_mut().clear_all_bus_devices();
                lower_bus.borrow_mut().insert(0x000000, wrap_transmutable(AddressRepeater::new(ram.clone(), 32)));
                lower_bus.borrow_mut().insert(0x400000, wrap_transmutable(AddressRepeater::new(rom.clone(), 16)));
                lower_bus.borrow_mut().insert(0x600000, wrap_transmutable(AddressRepeater::new(rom.clone(), 16)));
            } else {
                println!("{}: overlay is 1 (startup)", DEV_NAME);
                lower_bus.borrow_mut().clear_all_bus_devices();
                lower_bus.borrow_mut().insert(0x000000, wrap_transmutable(AddressRepeater::new(rom.clone(), 16)));
                lower_bus.borrow_mut().insert(0x200000, wrap_transmutable(AddressRepeater::new(rom.clone(), 16)));
                lower_bus.borrow_mut().insert(0x400000, wrap_transmutable(AddressRepeater::new(rom.clone(), 16)));
                lower_bus.borrow_mut().insert(0x600000, wrap_transmutable(AddressRepeater::new(ram.clone(), 16)));
            }
        });

        mainboard.via.port_a.notify();

        Ok(mainboard)
    }
}

impl Addressable for Mainboard {
    fn len(&self) -> usize {
        0x01000000
    }

    fn read(&mut self, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        if addr < 0x800000 {
            self.lower_bus.borrow_mut().read(addr, data)
        } else if addr >= 0x900000 && addr < 0xA00000 {
            self.scc1.read((addr >> 9) & 0x0F, data)
        } else if addr >= 0xB00000 && addr < 0xC00000 {
            self.scc2.read((addr >> 9) & 0x0F, data)
        } else if addr >= 0xD00000 && addr < 0xE00000 {
            self.iwm.read((addr >> 9) & 0x0F, data)
        } else if addr >= 0xE80000 && addr < 0xF00000 {
            self.via.read((addr >> 9) & 0x0F, data)
        } else if addr >= 0xF00000 && addr < 0xF80000 {
            self.phase_read.read(addr, data)
        } else if addr >= 0xF80000 && addr < 0xF80010 {
            // Debugger
            Ok(())
        } else {
            Err(Error::new(&format!("Error reading address {:#010x}", addr)))
        }
    }

    fn write(&mut self, addr: Address, data: &[u8]) -> Result<(), Error> {
        if addr < 0x800000 {
            self.lower_bus.borrow_mut().write(addr, data)
        } else if addr >= 0x900000 && addr < 0xA00000 {
            self.scc1.write((addr >> 9) & 0x0F, data)
        } else if addr >= 0xB00000 && addr < 0xC00000 {
            self.scc2.write((addr >> 9) & 0x0F, data)
        } else if addr >= 0xD00000 && addr < 0xE00000 {
            self.iwm.write((addr >> 9) & 0x0F, data)
        } else if addr >= 0xE80000 && addr < 0xF00000 {
            self.via.write((addr >> 9) & 0x0F, data)
        } else if addr >= 0xF00000 && addr < 0xF80000 {
            self.phase_read.write(addr, data)
        } else {
            Err(Error::new(&format!("Error writing address {:#010x}", addr)))
        }
    }
}

impl Steppable for Mainboard {
    fn step(&mut self, system: &System) -> Result<ClockElapsed, Error> {
        let elapsed = self.via.step(system)?;

        // TODO should this be 1 second, or a multiple of 979_200, which is an 8th of the CPU clock
        if self.last_sec + 1_000_000_000 > system.clock {
            self.last_sec += 1_000_000_000;
            //let port_a = self.via.port_a.borrow_mut();
            // TODO how will the ca1/ca2 cb1/cb2 pins work in the via
            system.get_interrupt_controller().set(true, 1, 25)?;
        }
        Ok(elapsed)
    }
}

impl Transmutable for Mainboard {
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        Some(self)
    }

    fn as_steppable(&mut self) -> Option<&mut dyn Steppable> {
        Some(self)
    }
}



pub struct PhaseRead {

}

impl PhaseRead {
    pub fn new() -> Self {
        Self {
        }
    }
}

impl Addressable for PhaseRead {
    fn len(&self) -> usize {
        0x80000
    }

    fn read(&mut self, _addr: Address, data: &mut [u8]) -> Result<(), Error> {
        // TODO I'm not sure how this is supposed to work
        data[0] = 0x00;
        Ok(())
    }

    fn write(&mut self, _addr: Address, _data: &[u8]) -> Result<(), Error> {
        // TODO I'm not sure how this is supposed to work
        Ok(())
    }
}



pub struct AddressRepeater {
    subdevice: TransmutableBox,
    repeat: u8,
}

impl AddressRepeater {
    pub fn new(subdevice: TransmutableBox, repeat: u8) -> Self {
        Self {
            subdevice,
            repeat,
        }
    }
}

impl Addressable for AddressRepeater {
    fn len(&self) -> usize {
        let len = self.subdevice.borrow_mut().as_addressable().unwrap().len();
        len * self.repeat as usize
    }

    fn read(&mut self, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        let len = self.subdevice.borrow_mut().as_addressable().unwrap().len() as Address;
        self.subdevice.borrow_mut().as_addressable().unwrap().read(addr % len, data)
    }

    fn write(&mut self, addr: Address, data: &[u8]) -> Result<(), Error> {
        let len = self.subdevice.borrow_mut().as_addressable().unwrap().len() as Address;
        self.subdevice.borrow_mut().as_addressable().unwrap().write(addr % len, data)
    }
}

impl Transmutable for AddressRepeater {
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        Some(self)
    }
}

