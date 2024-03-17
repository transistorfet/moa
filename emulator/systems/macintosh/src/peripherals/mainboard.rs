use std::rc::Rc;
use std::cell::RefCell;
use femtos::{Instant, Duration};

use moa_core::{System, Bus, Error, Address, Addressable, AddressRepeater, Steppable, Transmutable, Device};
use moa_signals::Observable;

use moa_peripherals_mos::Mos6522;
use moa_peripherals_zilog::Z8530;
use crate::peripherals::iwm::IWM;

const DEV_NAME: &str = "mac";


pub struct Mainboard {
    lower_bus: Rc<RefCell<Bus>>,
    scc1: Z8530,
    scc2: Z8530,
    iwm: IWM,
    via: Mos6522,
    phase_read: PhaseRead,
    last_sec: Instant,
}

impl Mainboard {
    pub fn new(ram: Device, rom: Device) -> Result<Self, Error> {
        let scc1 = Z8530::default();
        let scc2 = Z8530::default();
        let iwm = IWM::default();
        let via = Mos6522::default();
        let phase_read = PhaseRead::default();

        let lower_bus = Rc::new(RefCell::new(Bus::default()));

        let mainboard = Self {
            lower_bus: lower_bus.clone(),
            scc1,
            scc2,
            iwm,
            via,
            phase_read,
            last_sec: Instant::START,
        };

        mainboard.via.port_a.set_observer(move |port| {
            if (port.data & 0x10) == 0 {
                println!("{}: overlay is 0 (normal)", DEV_NAME);
                lower_bus.borrow_mut().clear_all_bus_devices();
                lower_bus
                    .borrow_mut()
                    .insert(0x000000, Device::new(AddressRepeater::new(ram.clone(), 0x400000)));
                lower_bus
                    .borrow_mut()
                    .insert(0x400000, Device::new(AddressRepeater::new(rom.clone(), 0x100000)));
                lower_bus
                    .borrow_mut()
                    .insert(0x600000, Device::new(AddressRepeater::new(rom.clone(), 0x100000)));
            } else {
                println!("{}: overlay is 1 (startup)", DEV_NAME);
                lower_bus.borrow_mut().clear_all_bus_devices();
                lower_bus
                    .borrow_mut()
                    .insert(0x000000, Device::new(AddressRepeater::new(rom.clone(), 0x100000)));
                lower_bus
                    .borrow_mut()
                    .insert(0x200000, Device::new(AddressRepeater::new(rom.clone(), 0x100000)));
                lower_bus
                    .borrow_mut()
                    .insert(0x400000, Device::new(AddressRepeater::new(rom.clone(), 0x100000)));
                lower_bus
                    .borrow_mut()
                    .insert(0x600000, Device::new(AddressRepeater::new(ram.clone(), 0x200000)));
            }
        });

        mainboard.via.port_a.notify();

        Ok(mainboard)
    }
}

impl Addressable for Mainboard {
    fn size(&self) -> usize {
        0x01000000
    }

    fn read(&mut self, clock: Instant, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        if addr < 0x800000 {
            self.lower_bus.borrow_mut().read(clock, addr, data)
        } else if (0x900000..0xA00000).contains(&addr) {
            self.scc1.read(clock, (addr >> 9) & 0x0F, data)
        } else if (0xB00000..0xC00000).contains(&addr) {
            self.scc2.read(clock, (addr >> 9) & 0x0F, data)
        } else if (0xD00000..0xE00000).contains(&addr) {
            self.iwm.read(clock, (addr >> 9) & 0x0F, data)
        } else if (0xE80000..0xF00000).contains(&addr) {
            self.via.read(clock, (addr >> 9) & 0x0F, data)
        } else if (0xF00000..0xF80000).contains(&addr) {
            self.phase_read.read(clock, addr, data)
        } else if (0xF80000..0xF80010).contains(&addr) {
            // Debugger
            Ok(())
        } else {
            Err(Error::new(format!("Error reading address {:#010x}", addr)))
        }
    }

    fn write(&mut self, clock: Instant, addr: Address, data: &[u8]) -> Result<(), Error> {
        if addr < 0x800000 {
            self.lower_bus.borrow_mut().write(clock, addr, data)
        } else if (0x900000..0xA00000).contains(&addr) {
            self.scc1.write(clock, (addr >> 9) & 0x0F, data)
        } else if (0xB00000..0xC00000).contains(&addr) {
            self.scc2.write(clock, (addr >> 9) & 0x0F, data)
        } else if (0xD00000..0xE00000).contains(&addr) {
            self.iwm.write(clock, (addr >> 9) & 0x0F, data)
        } else if (0xE80000..0xF00000).contains(&addr) {
            self.via.write(clock, (addr >> 9) & 0x0F, data)
        } else if (0xF00000..0xF80000).contains(&addr) {
            self.phase_read.write(clock, addr, data)
        } else {
            Err(Error::new(format!("Error writing address {:#010x}", addr)))
        }
    }
}

impl Steppable for Mainboard {
    fn step(&mut self, system: &System) -> Result<Duration, Error> {
        let elapsed = self.via.step(system)?;

        // TODO should this be 1 second, or a multiple of 979_200, which is an 8th of the CPU clock
        if self.last_sec + Duration::from_secs(1) > system.clock {
            self.last_sec += Duration::from_secs(1);
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



#[derive(Default)]
pub struct PhaseRead {}

impl Addressable for PhaseRead {
    fn size(&self) -> usize {
        0x80000
    }

    fn read(&mut self, _clock: Instant, _addr: Address, data: &mut [u8]) -> Result<(), Error> {
        // TODO I'm not sure how this is supposed to work
        data[0] = 0x00;
        Ok(())
    }

    fn write(&mut self, _clock: Instant, _addr: Address, _data: &[u8]) -> Result<(), Error> {
        // TODO I'm not sure how this is supposed to work
        Ok(())
    }
}
