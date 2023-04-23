
use moa_core::{Error, System, ClockDuration, Address, Addressable, Steppable, Transmutable, Signal, ObservableSignal, Observable, debug, warn};


const REG_OUTPUT_B: Address     = 0x00;
const REG_OUTPUT_A: Address     = 0x01;
const REG_DDR_B: Address        = 0x02;
const REG_DDR_A: Address        = 0x03;
const REG_PERIPH_CTRL: Address  = 0x0C;
const REG_INT_FLAGS: Address    = 0x0D;
const REG_INT_ENABLE: Address   = 0x0E;
const REG_OUTPUT_A_NHS: Address = 0x0F;


const DEV_NAME: &str = "mos6522";


pub struct Port {
    pub data: u8,
    pub ddr: u8,
}

impl Default for Port {
    fn default() -> Self {
        Self {
            data: 0xff,
            ddr: 0,
        }
    }
}


pub struct Mos6522 {
    pub port_a: ObservableSignal<Port>,
    pub port_b: ObservableSignal<Port>,
    pub peripheral_ctrl: u8,
    pub interrupt: Signal<bool>,
    pub interrupt_flags: u8,
    pub interrupt_enable: u8,
}

impl Default for Mos6522 {
    fn default() -> Self {
        Self {
            port_a: ObservableSignal::new(Port::default()),
            port_b: ObservableSignal::new(Port::default()),
            peripheral_ctrl: 0,
            interrupt: Signal::new(false),
            interrupt_flags: 0,
            interrupt_enable: 0,
        }
    }
}

impl Addressable for Mos6522 {
    fn len(&self) -> usize {
        0x10
    }

    fn read(&mut self, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        match addr {
            REG_OUTPUT_B => { data[0] = self.port_b.borrow_mut().data; },
            REG_OUTPUT_A => { data[0] = self.port_a.borrow_mut().data; },
            REG_DDR_B => { data[0] = self.port_b.borrow_mut().ddr; },
            REG_DDR_A => { data[0] = self.port_a.borrow_mut().ddr; },
            REG_INT_FLAGS => { data[0] = self.interrupt_flags; },
            REG_INT_ENABLE => { data[0] = self.interrupt_enable | 0x80; },
            _ => {
                warn!("{}: !!! unhandled read from {:0x}", DEV_NAME, addr);
            },
        }
        debug!("{}: read from register {:x} of {:?}", DEV_NAME, addr, data);
        Ok(())
    }

    fn write(&mut self, addr: Address, data: &[u8]) -> Result<(), Error> {
        debug!("{}: write to register {:x} with {:x}", DEV_NAME, addr, data[0]);
        match addr {
            REG_OUTPUT_B => { self.port_b.borrow_mut().data = data[0]; self.port_b.notify(); },
            REG_OUTPUT_A => { self.port_a.borrow_mut().data = data[0]; self.port_a.notify(); },
            REG_DDR_B => { self.port_b.borrow_mut().ddr = data[0]; self.port_b.notify(); },
            REG_DDR_A => { self.port_a.borrow_mut().ddr = data[0]; self.port_a.notify(); },
            REG_PERIPH_CTRL => { println!("SET TO {:?}", data[0]); self.peripheral_ctrl = data[0]; },
            REG_INT_FLAGS => { self.interrupt_flags &= !data[0] & 0x7F; },
            REG_INT_ENABLE => {
                if (data[0] & 0x80) == 0 {
                     self.interrupt_flags &= !data[0];
                } else {
                     self.interrupt_flags |= data[0];
                }
            },
            REG_OUTPUT_A_NHS => { self.port_a.borrow_mut().data = data[0]; self.port_a.notify(); },
            _ => {
                warn!("{}: !!! unhandled write {:0x} to {:0x}", DEV_NAME, data[0], addr);
            },
        }
        Ok(())
    }
}

impl Steppable for Mos6522 {
    fn step(&mut self, _system: &System) -> Result<ClockDuration, Error> {

        Ok(ClockDuration::from_micros(16_600))
    }
}


impl Transmutable for Mos6522 {
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        Some(self)
    }

    fn as_steppable(&mut self) -> Option<&mut dyn Steppable> {
        Some(self)
    }
}

