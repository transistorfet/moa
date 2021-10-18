
use crate::error::Error;
use crate::system::System;
use crate::devices::{Clock, Address, Steppable, Addressable, Transmutable, MAX_READ};

use crate::host::ttys::{SimplePty, SharedSimplePty};


const REG_MR1A_MR2A: Address = 0x01;
const REG_SRA_RD: Address = 0x03;
const REG_CSRA_WR: Address = 0x03;
const REG_CRA_WR: Address = 0x05;
const REG_TBA_WR: Address = 0x07;
const REG_RBA_RD: Address = 0x07;

const REG_MR1B_MR2B: Address = 0x11;
const REG_SRB_RD: Address = 0x13;
const REG_CSRB_WR: Address = 0x13;
const REG_CRB_WR: Address = 0x15;
const REG_TBB_WR: Address = 0x17;
const REG_RBB_RD: Address = 0x17;

const REG_ACR_WR: Address = 0x09;

const REG_CTUR_WR: Address = 0x0D;
const REG_CTLR_WR: Address = 0x0F;
const REG_START_RD: Address = 0x1D;
const REG_STOP_RD: Address = 0x1F;

const REG_IPCR_RD: Address = 0x09;
const REG_OPCR_WR: Address = 0x1B;
const REG_INPUT_RD: Address = 0x1B;
const REG_OUT_SET: Address = 0x1D;
const REG_OUT_RESET: Address = 0x1F;

const REG_ISR_RD: Address = 0x0B;
const REG_IMR_WR: Address = 0x0B;
const REG_IVR_WR: Address = 0x19;


// Status Register Bits (SRA/SRB)
#[allow(dead_code)]
const SR_RECEIVED_BREAK: u8 = 0x80;
#[allow(dead_code)]
const SR_FRAMING_ERROR: u8 = 0x40;
#[allow(dead_code)]
const SR_PARITY_ERROR: u8 = 0x20;
#[allow(dead_code)]
const SR_OVERRUN_ERROR: u8 = 0x10;
#[allow(dead_code)]
const SR_TX_EMPTY: u8 = 0x08;
#[allow(dead_code)]
const SR_TX_READY: u8 = 0x04;
#[allow(dead_code)]
const SR_RX_FULL: u8 = 0x02;
#[allow(dead_code)]
const SR_RX_READY: u8 = 0x01;


// Interrupt Status/Mask Bits (ISR/IVR)
const ISR_INPUT_CHANGE: u8 = 0x80;
const ISR_CH_B_BREAK_CHANGE: u8 = 0x40;
const ISR_CH_B_RX_READY_FULL: u8 = 0x20;
const ISR_CH_B_TX_READY: u8 = 0x10;
const ISR_TIMER_CHANGE: u8 = 0x08;
const ISR_CH_A_BREAK_CHANGE: u8 = 0x04;
const ISR_CH_A_RX_READY_FULL: u8 = 0x02;
const ISR_CH_A_TX_READY: u8 = 0x01;


const DEV_NAME: &'static str = "mc68681";

pub struct MC68681Port {
    pub tty: Option<SharedSimplePty>,
    pub status: u8,

    pub tx_enabled: bool,

    pub rx_enabled: bool,
    pub input: u8,
}

impl MC68681Port {
    pub fn new() -> MC68681Port {
        MC68681Port {
            tty: None,
            status: 0,

            tx_enabled: false,

            rx_enabled: false,
            input: 0,
        }
    }

    pub fn open(&mut self) -> Result<String, Error> {
        let pty = SimplePty::open()?;
        let name = pty.lock().unwrap().name.clone();
        println!("{}: opening pts {}", DEV_NAME, name);
        self.tty = Some(pty);
        Ok(name)
    }

    pub fn send_byte(&mut self, data: u8) {
        self.tty.as_mut().map(|tty| tty.lock().unwrap().write(data));
        self.set_tx_status(false);
    }

    pub fn set_tx_status(&mut self, value: bool) {
        match value {
            true => { self.status |= SR_TX_READY | SR_TX_EMPTY; },
            false => { self.status &= !(SR_TX_READY | SR_TX_EMPTY); },
        }
    }

    pub fn set_rx_status(&mut self, value: bool) {
        match value {
            true => { self.status |= SR_RX_READY; },
            false => { self.status &= !SR_RX_READY; },
        }
    }

    pub fn check_rx(&mut self) -> Result<bool, Error> {
        if self.rx_enabled && (self.status & SR_RX_READY) == 0 && self.tty.is_some() {
            let tty = self.tty.as_mut().unwrap();
            let result = tty.lock().unwrap().read();
            match result {
                Some(input) => {
                    self.input = input;
                    self.set_rx_status(true);
                    return Ok(true);
                },
                None => { },
            }
        }
        Ok(false)
    }

    pub fn check_tx(&mut self) -> bool {
        self.set_tx_status(self.tx_enabled);
        self.tx_enabled
    }

    pub fn handle_command(&mut self, data: u8) -> Option<bool> {
        let rx_cmd = data& 0x03;
        if rx_cmd == 0b01 {
            self.rx_enabled = true;
        } else if rx_cmd == 0b10 {
            self.rx_enabled = false;
        }

        let tx_cmd = (data & 0x0C) >> 2;
        if tx_cmd == 0b01 {
            self.tx_enabled = true;
            self.set_tx_status(true);
            return Some(true);
        } else if tx_cmd == 0b10 {
            self.tx_enabled = false;
            self.set_tx_status(false);
            return Some(false);
        }

        None
    }
}

pub struct MC68681 {
    pub acr: u8,
    pub port_a: MC68681Port,
    pub port_b: MC68681Port,

    pub int_mask: u8,
    pub int_status: u8,
    pub int_vector: u8,

    pub timer_preload: u16,
    pub timer_count: u16,
    pub is_timing: bool,
    pub timer_divider: u16,

    pub input_pin_change: u8,
    pub input_state: u8,
    pub output_conf: u8,
    pub output_state: u8,
}

impl MC68681 {
    pub fn new() -> Self {
        MC68681 {
            acr: 0,
            port_a: MC68681Port::new(),
            port_b: MC68681Port::new(),

            int_mask: 0,
            int_status: 0,
            int_vector: 0,

            timer_preload: 0,
            timer_count: 0,
            is_timing: true,
            timer_divider: 0,

            input_pin_change: 0,
            input_state: 0,
            output_conf: 0,
            output_state: 0,
        }
    }

    pub fn step_internal(&mut self, system: &System) -> Result<(), Error> {
        if self.port_a.check_rx()? {
            self.set_interrupt_flag(ISR_CH_A_RX_READY_FULL, true);
        }

        if self.port_b.check_rx()? {
            self.set_interrupt_flag(ISR_CH_B_RX_READY_FULL, true);
        }

        if self.is_timing {
            self.timer_divider = self.timer_divider.wrapping_sub(1);
            if self.timer_divider == 0 {
                self.timer_divider = 1;
                self.timer_count = self.timer_count.wrapping_sub(1);

                if self.timer_count == 0 {
                    self.set_interrupt_flag(ISR_TIMER_CHANGE, true);
                    if (self.acr & 0x40) == 0 {
                        self.is_timing = false;
                    } else {
                        self.timer_count = self.timer_preload;
                    }
                }
            }
        }

        self.check_interrupt_state(system)?;

        if self.port_a.check_tx() {
            self.set_interrupt_flag(ISR_CH_A_TX_READY, true);
        }

        if self.port_b.check_tx() {
            self.set_interrupt_flag(ISR_CH_B_TX_READY, true);
        }

        Ok(())
    }

    fn set_interrupt_flag(&mut self, flag: u8, value: bool) {
        self.int_status = (self.int_status & !flag) | (if value { flag } else { 0 });
    }

    fn check_interrupt_state(&mut self, system: &System) -> Result<(), Error> {
        system.get_interrupt_controller().set((self.int_status & self.int_mask) != 0, 4, self.int_vector)
    }
}

impl Addressable for MC68681 {
    fn len(&self) -> usize {
        0x30
    }

    fn read(&mut self, addr: Address, _count: usize) -> Result<[u8; MAX_READ], Error> {
        let mut data = [0; MAX_READ];

        match addr {
            REG_SRA_RD => {
                data[0] = self.port_a.status
            },
            REG_RBA_RD => {
                data[0] = self.port_a.input;
                self.port_a.set_rx_status(false);
                self.set_interrupt_flag(ISR_CH_A_RX_READY_FULL, false);
            },
            REG_SRB_RD => {
                data[0] = self.port_b.status
            },
            REG_RBB_RD => {
                data[0] = self.port_b.input;
                self.port_b.set_rx_status(false);
                self.set_interrupt_flag(ISR_CH_B_RX_READY_FULL, false);
            },
            REG_ISR_RD => {
                data[0] = self.int_status;
            },
            REG_IPCR_RD => {
                data[0] = self.input_pin_change;
            },
            REG_INPUT_RD => {
                data[0] = self.input_state;
            },
            REG_START_RD => {
                self.timer_count = self.timer_preload;
                self.is_timing = true;
            },
            REG_STOP_RD => {
                if (self.acr & 0x40) == 0 {
                    // Counter Mode
                    self.is_timing = false;
                    self.timer_count = self.timer_preload;
                } else {
                    // Timer Mode
                    // Do nothing except reset the ISR bit
                }
                self.set_interrupt_flag(ISR_TIMER_CHANGE, false);
            },
            _ => { },
        }

        if addr != REG_SRA_RD && addr != REG_SRB_RD {
            debug!("{}: read from {:0x} of {:0x}", DEV_NAME, addr, data[0]);
        }

        Ok(data)
    }

    fn write(&mut self, addr: Address, data: &[u8]) -> Result<(), Error> {
        debug!("{}: writing {:0x} to {:0x}", DEV_NAME, data[0], addr);
        match addr {
            REG_MR1A_MR2A | REG_MR1B_MR2B | REG_CSRA_WR | REG_CSRB_WR => {
                // NOTE we aren't simulating the serial speeds, so we aren't doing anything with these settings atm
            },
            REG_ACR_WR => {
                self.acr = data[0];
            }
            REG_TBA_WR => {
                debug!("{}a: write {}", DEV_NAME, data[0] as char);
                self.port_a.send_byte(data[0]);
                self.set_interrupt_flag(ISR_CH_A_TX_READY, false);
            },
            REG_CRA_WR => {
                match self.port_a.handle_command(data[0]) {
                    Some(value) => self.set_interrupt_flag(ISR_CH_A_TX_READY, value),
                    None => { },
                }
            },
            REG_TBB_WR => {
                debug!("{}b: write {:x}", DEV_NAME, data[0]);
                self.port_b.send_byte(data[0]);
                self.set_interrupt_flag(ISR_CH_B_TX_READY, false);
            },
            REG_CRB_WR => {
                match self.port_b.handle_command(data[0]) {
                    Some(value) => self.set_interrupt_flag(ISR_CH_B_TX_READY, value),
                    None => { },
                }
            },
            REG_CTUR_WR => {
                self.timer_preload = (self.timer_preload & 0x00FF) | ((data[0] as u16) << 8);
            },
            REG_CTLR_WR => {
                self.timer_preload = (self.timer_preload & 0xFF00) | (data[0] as u16);
            },
            REG_IMR_WR => {
                self.int_mask = data[0];
            },
            REG_IVR_WR => {
                self.int_vector = data[0];
            },
            REG_OPCR_WR => {
                self.output_conf = data[0];
            },
            REG_OUT_SET => {
                self.output_state |= data[0];
            },
            REG_OUT_RESET => {
                self.output_state &= !data[0];
            },
            _ => { },
        }
        Ok(())
    }
}

impl Steppable for MC68681 {
    fn step(&mut self, system: &System) -> Result<Clock, Error> {
        self.step_internal(system)?;
        Ok(1)
    }
}

impl Transmutable for MC68681 {
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        Some(self)
    }

    fn as_steppable(&mut self) -> Option<&mut dyn Steppable> {
        Some(self)
    }
}
