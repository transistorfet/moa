
use std::fs;

use crate::error::Error;
use crate::memory::{Address, Addressable};


const ATA_REG_DEV_CONTROL: Address      = 0x1D;
const ATA_REG_DEV_ADDRESS: Address      = 0x1F;
const ATA_REG_DATA: Address             = 0x20;
const ATA_REG_DATA_BYTE: Address        = 0x21;
const ATA_REG_FEATURE: Address          = 0x23;
const ATA_REG_ERROR: Address            = 0x23;
const ATA_REG_SECTOR_COUNT: Address     = 0x25;
const ATA_REG_SECTOR_NUM: Address       = 0x27;
const ATA_REG_CYL_LOW: Address          = 0x29;
const ATA_REG_CYL_HIGH: Address         = 0x2B;
const ATA_REG_DRIVE_HEAD: Address       = 0x2D;
const ATA_REG_STATUS: Address           = 0x2F;
const ATA_REG_COMMAND: Address          = 0x2F;

const ATA_CMD_READ_SECTORS: u8          = 0x20;
const ATA_CMD_WRITE_SECTORS: u8         = 0x30;
const ATA_CMD_IDENTIFY: u8              = 0xEC;
const ATA_CMD_SET_FEATURE: u8           = 0xEF;


const DEV_NAME: &'static str = "ata";

pub struct AtaDevice {
    pub read_addr: u32,
    pub read_count: u32,
    pub contents: Vec<u8>,
}


impl AtaDevice {
    pub fn new() -> Self {
        AtaDevice {
            read_addr: 0,
            read_count: 0,
            contents: vec![],
        }
    }

    pub fn load(&mut self, filename: &str) -> Result<(), Error> {
        match fs::read(filename) {
            Ok(contents) => {
                self.contents = contents;
                Ok(())
            },
            Err(_) => Err(Error::new(&format!("Error reading contents of {}", filename))),
        }
    }
}

impl Addressable for AtaDevice {
    fn len(&self) -> usize {
        0x30
    }

    fn read(&mut self, addr: Address, count: usize) -> Vec<u8> {
        let mut data = vec![0; count];

        match addr {
            ATA_REG_COMMAND => {
                //data[0] = self.input;
            },
            _ => { println!("{}: reading from {:0x}", DEV_NAME, addr); },
        }

        data
    }

    fn write(&mut self, mut addr: Address, data: &[u8]) {
        match addr {
            _ => { println!("{}: writing {:0x} to {:0x}", DEV_NAME, data[0], addr); },
        }
    }
}

