
use crate::error::Error;
use crate::devices::{Address, Addressable, Transmutable};
use crate::host::controllers::{ControllerDevice, ControllerEvent};
use crate::host::traits::{Host, ControllerUpdater, SharedData};


const REG_VERSION: Address      = 0x01;
const REG_DATA1: Address        = 0x03;
const REG_DATA2: Address        = 0x05;
const REG_DATA3: Address        = 0x07;
const REG_CTRL1: Address        = 0x09;
const REG_CTRL2: Address        = 0x0B;
const REG_CTRL3: Address        = 0x0D;
const REG_S_CTRL1: Address      = 0x13;
const REG_S_CTRL2: Address      = 0x19;
const REG_S_CTRL3: Address      = 0x1F;


const DEV_NAME: &'static str = "genesis_controller";

pub struct GenesisControllerPort {
    /// Data contains bits:
    /// 11 | 10 | 9 |    8 |     7 | 6 | 5 | 4 |     3 |    2 |    1 |  0
    ///  X |  Y | Z | MODE | START | A | C | B | RIGHT | LEFT | DOWN | UP
    pub data: SharedData<u16>,

    pub ctrl: u8,
    pub th_count: u8,
    pub next_read: u8,

    pub s_ctrl: u8,
}

impl GenesisControllerPort {
    pub fn new() -> Self {
        Self {
            data: SharedData::new(0xffff),
            ctrl: 0,
            th_count: 0,
            next_read: 0,
            s_ctrl: 0,
        }
    }

    pub fn set_data(&mut self, outputs: u8) {
        let prev_th = self.next_read & 0x40;
        self.next_read = outputs & self.ctrl;

        if ((self.next_read & 0x40) ^ prev_th) != 0 {
            // TH bit was toggled
            let inputs = self.data.get();
            self.next_read = match self.th_count {
                0 => self.next_read | ((inputs & 0x003F) as u8),
                1 => self.next_read | (((inputs & 0x00C0) >> 2) as u8) | ((inputs & 0x0003) as u8),
                2 => self.next_read | ((inputs & 0x003F) as u8),
                3 => self.next_read | (((inputs & 0x00C0) >> 2) as u8),
                4 => self.next_read | ((inputs & 0x0030) as u8) | (((inputs & 0x0F00) >> 8) as u8),
                5 => self.next_read | (((inputs & 0x00C0) >> 2) as u8) | 0x0F,
                6 => self.next_read | ((inputs & 0x003F) as u8),
                7 => self.next_read | (((inputs & 0x00C0) >> 2) as u8) | ((inputs & 0x0003) as u8),
                _ => 0,
            };

            self.th_count += 1;
            if self.th_count > 7 {
                self.th_count = 0;
            }
        }
    }

    pub fn set_ctrl(&mut self, ctrl: u8) {
        self.ctrl = ctrl;
        self.th_count = 0;
    }
}

pub struct GenesisControllerUpdater(SharedData<u16>, SharedData<bool>);

impl ControllerUpdater for GenesisControllerUpdater {
    fn update_controller(&mut self, event: ControllerEvent) {
        let (mask, state) = match event {
            ControllerEvent::ButtonA(state) => (0x0040, state),
            ControllerEvent::ButtonB(state) => (0x0010, state),
            ControllerEvent::ButtonC(state) => (0x0020, state),
            ControllerEvent::DpadUp(state) => (0x0001, state),
            ControllerEvent::DpadDown(state) => (0x0002, state),
            ControllerEvent::DpadLeft(state) => (0x0004, state),
            ControllerEvent::DpadRight(state) => (0x0008, state),
            ControllerEvent::Start(state) => (0x0080, state),
            ControllerEvent::Mode(state) => (0x0100, state),
            _ => (0x0000, false),
        };

        let buttons = (self.0.get() & !mask) | (if !state { mask } else { 0 });
        self.0.set(buttons);
        if buttons != 0 {
            self.1.set(true);
        }
    }
}



pub struct GenesisController {
    pub port_1: GenesisControllerPort,
    pub port_2: GenesisControllerPort,
    pub expansion: GenesisControllerPort,
    pub interrupt: SharedData<bool>,
}

impl GenesisController {
    pub fn new() -> Self {
        GenesisController {
            port_1: GenesisControllerPort::new(),
            port_2: GenesisControllerPort::new(),
            expansion: GenesisControllerPort::new(),
            interrupt: SharedData::new(false),
        }
    }

    pub fn create<H: Host>(host: &mut H) -> Result<Self, Error> {
        let controller = GenesisController::new();

        let controller1 = Box::new(GenesisControllerUpdater(controller.port_1.data.clone(), controller.interrupt.clone()));
        host.register_controller(ControllerDevice::A, controller1)?;
        let controller2 = Box::new(GenesisControllerUpdater(controller.port_2.data.clone(), controller.interrupt.clone()));
        host.register_controller(ControllerDevice::B, controller2)?;

        Ok(controller)
    }

    pub fn get_interrupt_signal(&self) -> SharedData<bool> {
        self.interrupt.clone()
    }
}

impl Addressable for GenesisController {
    fn len(&self) -> usize {
        0x30
    }

    fn read(&mut self, mut addr: Address, data: &mut [u8]) -> Result<(), Error> {
        // If the address is even, only the second byte (odd byte) will be meaningful
        let mut i = 0;
        if (addr % 2) == 0 {
            addr += 1;
            i += 1;
        }

        match addr {
            REG_VERSION => { data[i] = 0xA0; } // Overseas Version, NTSC, No Expansion
            REG_DATA1 => { data[i] = self.port_1.next_read; },
            REG_DATA2 => { data[i] = self.port_2.next_read; },
            REG_DATA3 => { data[i] = self.expansion.next_read; },
            REG_CTRL1 => { data[i] = self.port_1.ctrl; },
            REG_CTRL2 => { data[i] = self.port_2.ctrl; },
            REG_CTRL3 => { data[i] = self.expansion.ctrl; },
            REG_S_CTRL1 => { data[i] = self.port_1.s_ctrl | 0x02; },
            REG_S_CTRL2 => { data[i] = self.port_2.s_ctrl | 0x02; },
            REG_S_CTRL3 => { data[i] = self.expansion.s_ctrl | 0x02; },
            _ => { warning!("{}: !!! unhandled reading from {:0x}", DEV_NAME, addr); },
        }
        debug!("{}: read from register {:x} the value {:x}", DEV_NAME, addr, data[0]);
        Ok(())
    }

    fn write(&mut self, addr: Address, data: &[u8]) -> Result<(), Error> {
        debug!("{}: write to register {:x} with {:x}", DEV_NAME, addr, data[0]);
        match addr {
            REG_DATA1 => { self.port_1.set_data(data[0]); }
            REG_DATA2 => { self.port_2.set_data(data[0]); },
            REG_DATA3 => { self.expansion.set_data(data[0]); },
            REG_CTRL1 => { self.port_1.set_ctrl(data[0]); },
            REG_CTRL2 => { self.port_2.set_ctrl(data[0]); },
            REG_CTRL3 => { self.expansion.set_ctrl(data[0]); },
            REG_S_CTRL1 => { self.port_1.s_ctrl = data[0] & 0xF8; },
            REG_S_CTRL2 => { self.port_2.s_ctrl = data[0] & 0xF8; },
            REG_S_CTRL3 => { self.expansion.s_ctrl = data[0] & 0xF8; },
            _ => { warning!("{}: !!! unhandled write of {:0x} to {:0x}", DEV_NAME, data[0], addr); },
        }
        Ok(())
    }
}

// TODO make a step function to reset the TH count after 1.5ms

impl Transmutable for GenesisController {
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        Some(self)
    }
}


