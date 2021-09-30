
use std::slice::Iter;

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

const DEV_NAME: &'static str = "mc68681";

pub struct MC68681 {
    pub input: [u8; 1],
}

impl MC68681 {
    pub fn new() -> Self {
        MC68681 {
            input: [0],
        }
    }
}

impl Addressable for MC68681 {
    fn len(&self) -> usize {
        0x30
    }

    fn read(&self, addr: Address) -> Iter<u8> {
        match addr {
            REG_TBA_WR => self.input.iter(),
            _ => { println!("{}: reading from {:0x}", DEV_NAME, addr); self.input.iter() },
        }
    }

    fn write(&mut self, mut addr: Address, data: &[u8]) {
        match addr {
            REG_TBA_WR => { println!(">>> {}", data[0]); },
            _ => { println!("{}: writing {:0x} to {:0x}", DEV_NAME, data[0], addr); },
        }
    }
}

