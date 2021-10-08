
use std::process::Command;
use std::io::{Read, Write};
use std::os::unix::io::AsRawFd;

use nix::pty::{self, PtyMaster};
use nix::fcntl::OFlag;
use nix::unistd::sleep;
use nix::fcntl::{fcntl, FcntlArg};

use crate::error::Error;
use crate::system::{Clock, Steppable, System};
use crate::memory::{Address, Addressable};


const REG_MR1A_MR2A: Address = 0x01;
const REG_SRA_RD: Address = 0x03;
const REG_CSRA_WR: Address = 0x03;
const REG_CRA_WR: Address = 0x05;
const REG_TBA_WR: Address = 0x07;
const REG_RBA_RD: Address = 0x07;

const REG_MR1B_MR2B: Address = 0x700011;
const REG_SRB_RD: Address = 0x700013;
const REG_CSRB_WR: Address = 0x700013;
const REG_CRB_WR: Address = 0x700015;
const REG_TBB_WR: Address = 0x700017;
const REG_RBB_RD: Address = 0x700017;

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
const SR_RECEIVED_BREAK: u8 = 0x80;
const SR_FRAMING_ERROR: u8 = 0x40;
const SR_PARITY_ERROR: u8 = 0x20;
const SR_OVERRUN_ERROR: u8 = 0x10;
const SR_TX_EMPTY: u8 = 0x08;
const SR_TX_READY: u8 = 0x04;
const SR_RX_FULL: u8 = 0x02;
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

pub struct MC68681 {
    pub tty: Option<PtyMaster>,
    pub acr: u8,
    pub status_a: u8,
    pub input_a: u8,
    pub tx_a_enabled: bool,
    pub rx_a_enabled: bool,
    pub status_b: u8,
    pub input_b: u8,
    pub tx_b_enabled: bool,
    pub rx_b_enabled: bool,
    pub int_mask: u8,
    pub int_status: u8,
    pub int_vector: u8,
    pub is_interrupt: bool,
    pub timer_preload: u16,
    pub timer_count: u16,
    pub is_timing: bool,
}

impl MC68681 {
    pub fn new() -> Self {
        MC68681 {
            tty: None,
            acr: 0,

            status_a: 0,
            input_a: 0,
            tx_a_enabled: false,
            rx_a_enabled: false,
            status_b: 0,
            input_b: 0,
            tx_b_enabled: false,
            rx_b_enabled: false,

            int_mask: 0,
            int_status: 0,
            int_vector: 0,
            is_interrupt: false,

            timer_preload: 0,
            timer_count: 0,
            is_timing: true,
        }
    }

    pub fn open(&mut self) -> Result<(), Error> {
        let result = pty::posix_openpt(OFlag::O_RDWR).and_then(|master| {
            pty::grantpt(&master).and_then(|_| pty::unlockpt(&master)).and_then(|_| Ok(master))
        });

        match result {
            Ok(master) => {
                let name = unsafe { pty::ptsname(&master).map_err(|_| Error::new("Unable to get pty name"))? };
                println!("Open {}", name);
                fcntl(master.as_raw_fd(), FcntlArg::F_SETFL(OFlag::O_NONBLOCK)).unwrap();
                self.tty = Some(master);

                Command::new("x-terminal-emulator").arg("-e").arg(&format!("pyserial-miniterm {}", name)).spawn().unwrap();
                sleep(1);
                Ok(())
            },
            Err(_) => Err(Error::new("Error opening new pseudoterminal")),
        }
    }

    pub fn step_internal(&mut self, system: &System) -> Result<(), Error> {
        if self.rx_a_enabled && !self.rx_ready() && self.tty.is_some() {
            let mut buf = [0; 1];
            let tty = self.tty.as_mut().unwrap();
            match tty.read(&mut buf) {
                Ok(count) => {
                    println!("READ {:?}", count);
                    self.input_a = buf[0];
                    self.status_a |= SR_RX_READY;
                    self.int_status |= ISR_CH_A_RX_READY_FULL;
                },
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => { },
                Err(err) => {
                    println!("ERROR: {:?}", err);
                }
            }
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

    fn check_interrupt_state(&mut self, system: &System) -> Result<(), Error> {
        if !self.is_interrupt && (self.int_status & self.int_mask) != 0 {
            self.is_interrupt = true;
            system.change_interrupt_state(self.is_interrupt, 4, self.int_vector)?;
        }

        if self.is_interrupt && (self.int_status & self.int_mask) == 0 {
            self.is_interrupt = false;
            system.change_interrupt_state(self.is_interrupt, 4, self.int_vector)?;
        }
        Ok(())
    }

    pub fn rx_ready(&self) -> bool {
        (self.status_a & SR_RX_READY) != 0
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
                data[0] = self.status_a
            },
            REG_RBA_RD => {
                data[0] = self.input_a;
                self.status_a &= !SR_RX_READY;
                self.int_status &= !ISR_CH_A_RX_READY_FULL;
            },
            REG_SRB_RD => {
                data[0] = self.status_b
            },
            REG_RBB_RD => {
                data[0] = self.input_b;
                self.status_b &= !SR_RX_READY;
                self.int_status &= !ISR_CH_B_RX_READY_FULL;
            },
            REG_ISR_RD => {
                data[0] = self.int_status;
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
            //REG_MR1A_MR2A | REG_ACR_WR => {
            //    // TODO config
            //},
            REG_ACR_WR => {
                self.acr = data[0];
            }
            REG_TBA_WR => {
                println!("{}a: write {}", DEV_NAME, data[0] as char);
                self.tty.as_mut().map(|tty| tty.write_all(&[data[0]]));
            },
            REG_CRA_WR => {
                let rx_cmd = (data[0] & 0x03);
                if rx_cmd == 0b01 {
                    self.rx_a_enabled = true;
                } else if rx_cmd == 0b10 {
                    self.rx_a_enabled = false;
                }

                let tx_cmd = ((data[0] & 0x0C) >> 2);
                if tx_cmd == 0b01 {
                    self.tx_a_enabled = true;
                    self.status_a |= SR_TX_READY | SR_TX_EMPTY;
                    self.int_status |= ISR_CH_A_TX_READY;
                } else if tx_cmd == 0b10 {
                    self.tx_a_enabled = false;
                    self.status_a &= !(SR_TX_READY | SR_TX_EMPTY);
                    self.int_status &= !ISR_CH_A_TX_READY;
                }
            },
            REG_TBB_WR => {
                println!("{}b: write {:x}", DEV_NAME, data[0]);
            },
            REG_CRB_WR => {
                let rx_cmd = (data[0] & 0x03);
                if rx_cmd == 0b01 {
                    self.rx_b_enabled = true;
                } else if rx_cmd == 0b10 {
                    self.rx_b_enabled = false;
                }

                let tx_cmd = ((data[0] & 0x0C) >> 2);
                if tx_cmd == 0b01 {
                    self.tx_b_enabled = true;
                    self.status_b |= SR_TX_READY | SR_TX_EMPTY;
                    self.int_status |= ISR_CH_B_TX_READY;
                } else if tx_cmd == 0b10 {
                    self.tx_b_enabled = false;
                    self.status_b &= !(SR_TX_READY | SR_TX_EMPTY);
                    self.int_status &= !ISR_CH_B_TX_READY;
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

