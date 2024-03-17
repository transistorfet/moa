use femtos::{Instant, Duration};

use moa_core::{System, Error, Address, Addressable, Steppable, Transmutable};
use moa_host::{self, Host, HostError, ControllerDevice, ControllerInput, ControllerEvent, EventReceiver};
use moa_signals::{Signal};

const REG_VERSION: Address = 0x01;
const REG_DATA1: Address = 0x03;
const REG_DATA2: Address = 0x05;
const REG_DATA3: Address = 0x07;
const REG_CTRL1: Address = 0x09;
const REG_CTRL2: Address = 0x0B;
const REG_CTRL3: Address = 0x0D;
const REG_S_CTRL1: Address = 0x13;
const REG_S_CTRL2: Address = 0x19;
const REG_S_CTRL3: Address = 0x1F;


const DEV_NAME: &str = "genesis_controller";

pub struct GenesisControllerPort {
    /// Data contains bits:
    /// 11 | 10 | 9 |    8 |     7 | 6 | 5 | 4 |     3 |    2 |    1 |  0
    ///  X |  Y | Z | MODE | START | A | C | B | RIGHT | LEFT | DOWN | UP
    buttons: u16,

    ctrl: u8,
    outputs: u8,
    th_count: u8,

    s_ctrl: u8,
}

impl Default for GenesisControllerPort {
    fn default() -> Self {
        Self {
            buttons: 0xffff,
            ctrl: 0,
            outputs: 0,
            th_count: 0,
            s_ctrl: 0,
        }
    }
}

impl GenesisControllerPort {
    pub fn get_data(&mut self) -> u8 {
        let inputs = self.buttons;
        let th_state = (self.outputs & 0x40) != 0;

        match (th_state, self.th_count) {
            (true, 0) => self.outputs | ((inputs & 0x003F) as u8),
            (false, 1) => self.outputs | (((inputs & 0x00C0) >> 2) as u8) | ((inputs & 0x0003) as u8),
            (true, 1) => self.outputs | ((inputs & 0x003F) as u8),
            (false, 2) => self.outputs | (((inputs & 0x00C0) >> 2) as u8),
            (true, 2) => self.outputs | ((inputs & 0x0030) as u8) | (((inputs & 0x0F00) >> 8) as u8),
            (false, 3) => self.outputs | (((inputs & 0x00C0) >> 2) as u8) | 0x0F,
            (true, 3) => self.outputs | ((inputs & 0x003F) as u8),
            (false, 0) => self.outputs | (((inputs & 0x00C0) >> 2) as u8) | ((inputs & 0x0003) as u8),
            _ => 0,
        }
    }

    pub fn set_data(&mut self, outputs: u8) {
        let prev_th = self.outputs & 0x40;
        self.outputs = outputs;

        if ((outputs & 0x40) ^ prev_th) != 0 && (outputs & 0x40) == 0 {
            // TH bit was toggled

            self.th_count += 1;
            if self.th_count > 3 {
                self.th_count = 0;
            }
        }
    }

    pub fn set_ctrl(&mut self, ctrl: u8) {
        self.ctrl = ctrl;
        self.th_count = 0;
    }

    pub fn reset_count(&mut self) {
        self.th_count = 0;
    }
}


pub struct GenesisControllers {
    receiver: EventReceiver<ControllerEvent>,
    port_1: GenesisControllerPort,
    port_2: GenesisControllerPort,
    expansion: GenesisControllerPort,
    interrupt: Signal<bool>,
    reset_timer: Duration,
}

impl GenesisControllers {
    pub fn new<H, E>(host: &mut H) -> Result<Self, HostError<E>>
    where
        H: Host<Error = E>,
    {
        let (sender, receiver) = moa_host::event_queue();
        host.register_controllers(sender)?;

        Ok(Self {
            receiver,
            port_1: GenesisControllerPort::default(),
            port_2: GenesisControllerPort::default(),
            expansion: GenesisControllerPort::default(),
            interrupt: Signal::new(false),
            reset_timer: Duration::ZERO,
        })
    }

    pub fn get_interrupt_signal(&self) -> Signal<bool> {
        self.interrupt.clone()
    }

    fn process_event(&mut self, event: ControllerEvent) {
        let (mask, state) = match event.input {
            ControllerInput::ButtonA(state) => (0x0040, state),
            ControllerInput::ButtonB(state) => (0x0010, state),
            ControllerInput::ButtonC(state) => (0x0020, state),
            ControllerInput::DpadUp(state) => (0x0001, state),
            ControllerInput::DpadDown(state) => (0x0002, state),
            ControllerInput::DpadLeft(state) => (0x0004, state),
            ControllerInput::DpadRight(state) => (0x0008, state),
            ControllerInput::Start(state) => (0x0080, state),
            ControllerInput::Mode(state) => (0x0100, state),
            _ => (0x0000, false),
        };

        let prev_state = match event.device {
            ControllerDevice::A => &mut self.port_1.buttons,
            ControllerDevice::B => &mut self.port_2.buttons,
            _ => return,
        };

        *prev_state = (*prev_state & !mask) | (if !state { mask } else { 0 });
        if *prev_state != 0 {
            self.interrupt.set(true);
        }
    }
}

impl Addressable for GenesisControllers {
    fn size(&self) -> usize {
        0x30
    }

    fn read(&mut self, _clock: Instant, mut addr: Address, data: &mut [u8]) -> Result<(), Error> {
        // If the address is even, only the second byte (odd byte) will be meaningful
        let mut i = 0;
        if (addr % 2) == 0 {
            addr += 1;
            i += 1;
        }

        match addr {
            REG_VERSION => {
                data[i] = 0xA0;
            }, // Overseas Version, NTSC, No Expansion
            REG_DATA1 => {
                data[i] = self.port_1.get_data();
            },
            REG_DATA2 => {
                data[i] = self.port_2.get_data();
            },
            REG_DATA3 => {
                data[i] = self.expansion.get_data();
            },
            REG_CTRL1 => {
                data[i] = self.port_1.ctrl;
            },
            REG_CTRL2 => {
                data[i] = self.port_2.ctrl;
            },
            REG_CTRL3 => {
                data[i] = self.expansion.ctrl;
            },
            REG_S_CTRL1 => {
                data[i] = self.port_1.s_ctrl | 0x02;
            },
            REG_S_CTRL2 => {
                data[i] = self.port_2.s_ctrl | 0x02;
            },
            REG_S_CTRL3 => {
                data[i] = self.expansion.s_ctrl | 0x02;
            },
            _ => {
                log::warn!("{}: !!! unhandled reading from {:0x}", DEV_NAME, addr);
            },
        }
        log::info!("{}: read from register {:x} the value {:x}", DEV_NAME, addr, data[0]);
        Ok(())
    }

    fn write(&mut self, _clock: Instant, addr: Address, data: &[u8]) -> Result<(), Error> {
        self.reset_timer = Duration::ZERO;

        log::info!("{}: write to register {:x} with {:x}", DEV_NAME, addr, data[0]);
        match addr {
            REG_DATA1 => {
                self.port_1.set_data(data[0]);
            },
            REG_DATA2 => {
                self.port_2.set_data(data[0]);
            },
            REG_DATA3 => {
                self.expansion.set_data(data[0]);
            },
            REG_CTRL1 => {
                self.port_1.set_ctrl(data[0]);
            },
            REG_CTRL2 => {
                self.port_2.set_ctrl(data[0]);
            },
            REG_CTRL3 => {
                self.expansion.set_ctrl(data[0]);
            },
            REG_S_CTRL1 => {
                self.port_1.s_ctrl = data[0] & 0xF8;
            },
            REG_S_CTRL2 => {
                self.port_2.s_ctrl = data[0] & 0xF8;
            },
            REG_S_CTRL3 => {
                self.expansion.s_ctrl = data[0] & 0xF8;
            },
            _ => {
                log::warn!("{}: !!! unhandled write of {:0x} to {:0x}", DEV_NAME, data[0], addr);
            },
        }
        Ok(())
    }
}

impl Steppable for GenesisControllers {
    fn step(&mut self, _system: &System) -> Result<Duration, Error> {
        let duration = Duration::from_micros(100); // Update every 100us

        while let Some(event) = self.receiver.receive() {
            self.process_event(event);
        }

        self.reset_timer += duration;
        if self.reset_timer >= Duration::from_micros(1_500) {
            self.port_1.reset_count();
            self.port_2.reset_count();
            self.expansion.reset_count();
        }
        Ok(duration)
    }
}

impl Transmutable for GenesisControllers {
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        Some(self)
    }

    fn as_steppable(&mut self) -> Option<&mut dyn Steppable> {
        Some(self)
    }
}
