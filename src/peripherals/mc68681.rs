
use std::process::Command;
use std::io::{Read, Write};
use std::os::unix::io::AsRawFd;

use nix::pty::{self, PtyMaster};
use nix::fcntl::OFlag;
use nix::unistd::sleep;
use nix::fcntl::{fcntl, FcntlArg};

use crate::error::Error;
use crate::system::{Clock, Device, System};
use crate::memory::{Address, Addressable};


const REG_MR1A_MR2A: Address = 0x01;
const REG_SRA_RD: Address = 0x03;
const REG_CSRA_WR: Address = 0x03;
const REG_CRA_WR: Address = 0x05;
const REG_TBA_WR: Address = 0x07;
const REG_RBA_RD: Address = 0x07;
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


const DEV_NAME: &'static str = "mc68681";

pub struct MC68681 {
    pub tty: Option<PtyMaster>,
    pub status: u8,
    pub input: u8,
}

impl MC68681 {
    pub fn new() -> Self {
        MC68681 {
            tty: None,
            status: 0x0C,
            input: 0,
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

    pub fn step_internal(&mut self, _system: &System) -> Result<(), Error> {
        if !self.rx_ready() && self.tty.is_some() {
            let mut buf = [0; 1];
            let tty = self.tty.as_mut().unwrap();
            match tty.read(&mut buf) {
                Ok(count) => {
                    println!("READ {:?}", count);
                    self.input = buf[0];
                    self.status |= SR_RX_READY;
                },
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => { },
                Err(err) => {
                    println!("ERROR: {:?}", err);
                }
            }
        }

        Ok(())
    }

    pub fn rx_ready(&self) -> bool {
        (self.status & SR_RX_READY) != 0
    }
}

impl Addressable for MC68681 {
    fn len(&self) -> usize {
        0x30
    }

    fn read(&mut self, addr: Address, count: usize) -> Result<Vec<u8>, Error> {
        let mut data = vec![0; count];

        // TODO this is temporary
        //self.step();

        match addr {
            REG_SRA_RD => {
                data[0] = self.status
            },
            REG_RBA_RD => {
                data[0] = self.input;
                self.status &= !SR_RX_READY;
            },
            _ => { println!("{}: reading from {:0x}", DEV_NAME, addr); data[0] = self.input; },
        }

        Ok(data)
    }

    fn write(&mut self, addr: Address, data: &[u8]) -> Result<(), Error> {
        match addr {
            REG_TBA_WR => {
                println!("{}: {}", DEV_NAME, data[0] as char);
                self.tty.as_mut().map(|tty| tty.write_all(&[data[0]]));
            },
            _ => { println!("{}: writing {:0x} to {:0x}", DEV_NAME, data[0], addr); },
        }
        Ok(())
    }
}

impl Device for MC68681 {
    fn step(&mut self, system: &System) -> Result<Clock, Error> {
        self.step_internal(system)?;
        Ok(1)
    }
}

