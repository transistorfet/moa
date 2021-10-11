
use std::io::{Read, Write};
use std::os::unix::io::AsRawFd;

use nix::fcntl::OFlag;
use nix::pty::{self, PtyMaster};
use nix::fcntl::{fcntl, FcntlArg};

use crate::error::Error;
use crate::system::System;
use crate::devices::{Clock, Steppable};
use crate::memory::{Address, Addressable};


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
    pub tty: Option<PtyMaster>,
    pub status: u8,
    pub input: u8,
    pub tx_enabled: bool,
    pub rx_enabled: bool,
}

impl MC68681Port {
    pub fn new() -> MC68681Port {
        MC68681Port {
            tty: None,
            status: 0,
            input: 0,
            tx_enabled: false,
            rx_enabled: false,
        }
    }

    pub fn open(&mut self) -> Result<String, Error> {
        let master = pty::posix_openpt(OFlag::O_RDWR).and_then(|master| {
            pty::grantpt(&master)?;
            pty::unlockpt(&master)?;
            fcntl(master.as_raw_fd(), FcntlArg::F_SETFL(OFlag::O_NONBLOCK))?;
            Ok(master)
        }).map_err(|_| Error::new("Error opening new pseudoterminal"))?;

        let name = unsafe { pty::ptsname(&master).map_err(|_| Error::new("Unable to get pty name"))? };
        println!("{}: opening pts {}", DEV_NAME, name);
        self.tty = Some(master);
        Ok(name)
    }

    pub fn rx_ready(&self) -> bool {
        (self.status & SR_RX_READY) != 0
    }

    pub fn check_read(&mut self) -> Result<bool, Error> {
        if self.rx_enabled && !self.rx_ready() && self.tty.is_some() {
            let mut buf = [0; 1];
            let tty = self.tty.as_mut().unwrap();
            match tty.read(&mut buf) {
                Ok(count) => {
                    println!("READ {:?}", count);
                    self.input = buf[0];
                    self.status |= SR_RX_READY;
                    return Ok(true);
                },
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => { },
                Err(err) => {
                    println!("ERROR: {:?}", err);
                }
            }
        }
        Ok(false)
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
            self.status |= SR_TX_READY | SR_TX_EMPTY;
            return Some(true);
        } else if tx_cmd == 0b10 {
            self.tx_enabled = false;
            self.status &= !(SR_TX_READY | SR_TX_EMPTY);
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

            input_pin_change: 0,
            input_state: 0,
            output_conf: 0,
            output_state: 0,
        }
    }

    pub fn step_internal(&mut self, system: &System) -> Result<(), Error> {
        if self.port_a.check_read()? {
            self.int_status |= ISR_CH_A_RX_READY_FULL;
        }

        if self.port_b.check_read()? {
            self.int_status |= ISR_CH_B_RX_READY_FULL;
        }

        if self.is_timing {
            self.timer_count = self.timer_count.wrapping_sub(1);

            if self.timer_count == 0 {
                self.int_status |= ISR_TIMER_CHANGE;
                if (self.acr & 0x40) == 0 {
                    self.is_timing = false;
                } else {
                    self.timer_count = self.timer_preload;
                }
            }
        }

        self.check_interrupt_state(system)?;

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

    fn read(&mut self, addr: Address, count: usize) -> Result<Vec<u8>, Error> {
        let mut data = vec![0; count];

        if addr != REG_SRA_RD && addr != REG_SRB_RD {
            println!("{}: reading from {:0x}", DEV_NAME, addr);
        }

        match addr {
            REG_SRA_RD => {
                data[0] = self.port_a.status
            },
            REG_RBA_RD => {
                data[0] = self.port_a.input;
                self.port_a.status &= !SR_RX_READY;
                self.int_status &= !ISR_CH_A_RX_READY_FULL;
            },
            REG_SRB_RD => {
                data[0] = self.port_b.status
            },
            REG_RBB_RD => {
                data[0] = self.port_b.input;
                self.port_b.status &= !SR_RX_READY;
                self.int_status &= !ISR_CH_B_RX_READY_FULL;
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
                self.int_status &= !ISR_TIMER_CHANGE;
                if (self.acr & 0x40) == 0 {
                    // Counter Mode
                    self.is_timing = false;
                    self.timer_count = self.timer_preload;
                } else {
                    // Timer Mode
                    // Do nothing except reset the ISR bit
                }
            },
            _ => { },
        }

        Ok(data)
    }

    fn write(&mut self, addr: Address, data: &[u8]) -> Result<(), Error> {
        println!("{}: writing {:0x} to {:0x}", DEV_NAME, data[0], addr);
        match addr {
            REG_MR1A_MR2A | REG_MR1B_MR2B | REG_CSRA_WR | REG_CSRB_WR => {
                // NOTE we aren't simulating the serial speeds, so we aren't doing anything with these settings atm
            },
            REG_ACR_WR => {
                self.acr = data[0];
            }
            REG_TBA_WR => {
                println!("{}a: write {}", DEV_NAME, data[0] as char);
                self.port_a.tty.as_mut().map(|tty| tty.write_all(&[data[0]]));
            },
            REG_CRA_WR => {
                match self.port_a.handle_command(data[0]) {
                    Some(value) => self.set_interrupt_flag(ISR_CH_A_TX_READY, value),
                    None => { },
                }
            },
            REG_TBB_WR => {
                println!("{}b: write {:x}", DEV_NAME, data[0]);
                self.port_b.tty.as_mut().map(|tty| tty.write_all(&[data[0]]));
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

