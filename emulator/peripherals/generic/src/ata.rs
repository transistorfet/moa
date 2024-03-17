use std::fs;
use femtos::Instant;

use moa_core::{Error, Address, Addressable, Transmutable};

#[rustfmt::skip]
mod reg {
    use super::Address;
    pub(super) const DATA_WORD: Address        = 0x20;
    pub(super) const DATA_BYTE: Address        = 0x21;
    pub(super) const FEATURE: Address          = 0x23;
    pub(super) const ERROR: Address            = 0x23;
    pub(super) const SECTOR_COUNT: Address     = 0x25;
    pub(super) const SECTOR_NUM: Address       = 0x27;
    pub(super) const CYL_LOW: Address          = 0x29;
    pub(super) const CYL_HIGH: Address         = 0x2B;
    pub(super) const DRIVE_HEAD: Address       = 0x2D;
    pub(super) const STATUS: Address           = 0x2F;
    pub(super) const COMMAND: Address          = 0x2F;
}

#[rustfmt::skip]
mod cmd {
    pub(super) const READ_SECTORS: u8          = 0x20;
    pub(super) const WRITE_SECTORS: u8         = 0x30;
    pub(super) const IDENTIFY: u8              = 0xEC;
    pub(super) const SET_FEATURE: u8           = 0xEF;
}

#[allow(dead_code)]
const ATA_ST_BUSY: u8 = 0x80;
#[allow(dead_code)]
const ATA_ST_DATA_READY: u8 = 0x08;
#[allow(dead_code)]
const ATA_ST_ERROR: u8 = 0x01;

const ATA_SECTOR_SIZE: u32 = 512;

const DEV_NAME: &str = "ata";

#[derive(Default)]
pub struct AtaDevice {
    selected_sector: u32,
    selected_count: u32,
    last_error: u8,
    contents: Vec<u8>,
}

impl AtaDevice {
    pub fn load(&mut self, filename: &str) -> Result<(), Error> {
        match fs::read(filename) {
            Ok(contents) => {
                self.contents = contents;
                Ok(())
            },
            Err(_) => Err(Error::new(format!("Error reading contents of {}", filename))),
        }
    }
}

impl Addressable for AtaDevice {
    fn size(&self) -> usize {
        0x30
    }

    fn read(&mut self, _clock: Instant, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        match addr {
            reg::DATA_WORD => {
                self.selected_count -= 2;
                let offset = ((self.selected_sector * ATA_SECTOR_SIZE) + (ATA_SECTOR_SIZE - 1 - self.selected_count)) as usize;
                data[0] = self.contents[offset];
                data[1] = self.contents[offset + 1];
                if self.selected_count == 0 {
                    self.selected_sector = 0;
                    self.selected_count = 0;
                }
            },
            reg::DATA_BYTE => {
                self.selected_count -= 1;
                let offset = ((self.selected_sector * ATA_SECTOR_SIZE) + (ATA_SECTOR_SIZE - 1 - self.selected_count)) as usize;
                data[0] = self.contents[offset];
                if self.selected_count == 0 {
                    self.selected_sector = 0;
                    self.selected_count = 0;
                }
            },
            reg::STATUS => {
                data[0] = ATA_ST_DATA_READY;
            },
            reg::ERROR => {
                data[0] = self.last_error;
            },
            _ => {
                log::debug!("{}: reading from {:0x}", DEV_NAME, addr);
            },
        }

        Ok(())
    }

    fn write(&mut self, _clock: Instant, addr: Address, data: &[u8]) -> Result<(), Error> {
        log::debug!("{}: write to register {:x} with {:x}", DEV_NAME, addr, data[0]);
        match addr {
            reg::DRIVE_HEAD => {
                self.selected_sector |= ((data[0] & 0x1F) as u32) << 24;
            },
            reg::CYL_HIGH => {
                self.selected_sector |= (data[0] as u32) << 16;
            },
            reg::CYL_LOW => {
                self.selected_sector |= (data[0] as u32) << 8;
            },
            reg::SECTOR_NUM => {
                self.selected_sector |= data[0] as u32;
            },
            reg::SECTOR_COUNT => {
                self.selected_count = (data[0] as u32) * ATA_SECTOR_SIZE;
            },
            reg::COMMAND => match data[0] {
                cmd::READ_SECTORS => {
                    log::debug!("{}: reading sector {:x}", DEV_NAME, self.selected_sector);
                },
                cmd::WRITE_SECTORS => {
                    log::debug!("{}: writing sector {:x}", DEV_NAME, self.selected_sector);
                },
                cmd::IDENTIFY => {},
                cmd::SET_FEATURE => {},
                _ => {
                    log::debug!("{}: unrecognized command {:x}", DEV_NAME, data[0]);
                },
            },
            reg::FEATURE => {
                // TODO implement features
            },
            reg::DATA_BYTE => {
                // TODO implement writing
            },
            _ => {
                log::debug!("{}: writing {:0x} to {:0x}", DEV_NAME, data[0], addr);
            },
        }
        Ok(())
    }
}

impl Transmutable for AtaDevice {
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        Some(self)
    }
}
