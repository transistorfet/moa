
use std::fs;

use crate::error::Error;
use crate::memory::{Address, Addressable};


const ATA_REG_DEV_CONTROL: Address      = 0x1D;
const ATA_REG_DEV_ADDRESS: Address      = 0x1F;
const ATA_REG_DATA_WORD: Address        = 0x20;
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

const ATA_ST_BUSY: u8                   = 0x80;
const ATA_ST_DATA_READY: u8             = 0x08;
const ATA_ST_ERROR: u8                  = 0x01;

const ATA_SECTOR_SIZE: u32              = 512;

const DEV_NAME: &'static str = "ata";

pub struct AtaDevice {
    pub selected_sector: u32,
    pub selected_count: u32,
    pub contents: Vec<u8>,
}


impl AtaDevice {
    pub fn new() -> Self {
        AtaDevice {
            selected_sector: 0,
            selected_count: 0,
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

    fn read(&mut self, addr: Address, count: usize) -> Result<Vec<u8>, Error> {
        let mut data = vec![0; count];

        match addr {
            ATA_REG_DATA_WORD => {
                self.selected_count -= 2;
                let offset = ((self.selected_sector * ATA_SECTOR_SIZE) + (ATA_SECTOR_SIZE -1 - self.selected_count)) as usize;
                data[0] = self.contents[offset];
                data[1] = self.contents[offset + 1];
println!(">> {:x}{:x}", data[0], data[1]);
            },
            ATA_REG_DATA_BYTE => {
                self.selected_count -= 1;
                let offset = ((self.selected_sector * ATA_SECTOR_SIZE) + (ATA_SECTOR_SIZE - 1 - self.selected_count)) as usize;
                data[0] = self.contents[offset];
println!(">> {:x}", data[0]);
            },
            ATA_REG_STATUS => {
                data[0] = ATA_ST_DATA_READY;
            },
            _ => { println!("{}: reading from {:0x}", DEV_NAME, addr); },
        }

        Ok(data)
    }

    fn write(&mut self, mut addr: Address, data: &[u8]) -> Result<(), Error> {
        println!("{}: write to register {:x} with {:x}", DEV_NAME, addr, data[0]);
        match addr {
            ATA_REG_DRIVE_HEAD => { self.selected_sector |= ((data[0] & 0x1F) as u32) << 24; },
            ATA_REG_CYL_HIGH => { self.selected_sector |= (data[0] as u32) << 16; },
            ATA_REG_CYL_LOW => { self.selected_sector |= (data[0] as u32) << 8; },
            ATA_REG_SECTOR_NUM => { self.selected_sector |= data[0] as u32; },
            ATA_REG_SECTOR_COUNT => { self.selected_count = (data[0] as u32) * ATA_SECTOR_SIZE; },
            ATA_REG_COMMAND => {
                match data[0] {
                    ATA_CMD_READ_SECTORS => { println!("{}: reading sector {:x}", DEV_NAME, self.selected_sector); },
                    ATA_CMD_WRITE_SECTORS => { println!("{}: writing sector {:x}", DEV_NAME, self.selected_sector); },
                    ATA_CMD_IDENTIFY => { },
                    ATA_CMD_SET_FEATURE => { },
                    _ => { println!("{}: unrecognized command {:x}", DEV_NAME, data[0]); },
                }
            },
            ATA_REG_FEATURE => {
                // TODO implement features
            },
            ATA_REG_DATA => {
                // TODO implement writing
            },
            _ => { println!("{}: writing {:0x} to {:0x}", DEV_NAME, data[0], addr); },
        }
        Ok(())
    }
}

