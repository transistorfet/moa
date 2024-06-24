use std::fs;
use std::io;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use core::convert::Infallible;
use femtos::Instant;
use emulator_hal::{BusAccess, BusAdapter, Step, FromAddress, IntoAddress, Error as EmuError};

//use moa_core::{Error, Bus, MoaBus, Address, Addressable, Transmutable, DeviceInterface};
use moa_system::{Error as MoaError, MoaBus, DeviceInterface};

#[rustfmt::skip]
mod reg {
    use super::DeviceAddress;
    pub(super) const DATA_WORD: u8       = 0x20;
    pub(super) const DATA_BYTE: u8       = 0x21;
    pub(super) const FEATURE: u8         = 0x23;
    pub(super) const ERROR: u8           = 0x23;
    pub(super) const SECTOR_COUNT: u8    = 0x25;
    pub(super) const SECTOR_NUM: u8      = 0x27;
    pub(super) const CYL_LOW: u8         = 0x29;
    pub(super) const CYL_HIGH: u8        = 0x2B;
    pub(super) const DRIVE_HEAD: u8      = 0x2D;
    pub(super) const STATUS: u8          = 0x2F;
    pub(super) const COMMAND: u8         = 0x2F;
}

#[rustfmt::skip]
mod cmd {
    pub(super) const READ_SECTORS: u8               = 0x20;
    pub(super) const WRITE_SECTORS: u8              = 0x30;
    pub(super) const IDENTIFY: u8                   = 0xEC;
    pub(super) const SET_FEATURE: u8                = 0xEF;
}

#[allow(dead_code)]
const ATA_ST_BUSY: u8 = 0x80;
#[allow(dead_code)]
const ATA_ST_DATA_READY: u8 = 0x08;
#[allow(dead_code)]
const ATA_ST_ERROR: u8 = 0x01;

const ATA_SECTOR_SIZE: u32 = 512;

const DEV_NAME: &str = "ata";

pub struct DeviceAddress(u8);

#[derive(Default)]
pub struct AtaDevice<Error>
where
    Error: Default,
{
    selected_sector: u32,
    selected_count: u32,
    last_error: u8,
    contents: Vec<u8>,
    error: PhantomData<Error>,
}

impl<Error> AtaDevice<Error>
where
    Error: Default,
{
    pub fn load(&mut self, filename: &str) -> Result<(), io::Error> {
        let contents = fs::read(filename)?;
        self.contents = contents;
        Ok(())
    }

    pub fn address_space(&self) -> usize {
        0x30
    }
}

impl<Address, Error> BusAccess<Address> for AtaDevice<Error>
where
    Error: EmuError + Default,
    Address: IntoAddress<DeviceAddress> + Copy,
{
    type Instant = Instant;
    type Error = Error;

    #[inline]
    fn read(&mut self, _clock: Self::Instant, addr: Address, data: &mut [u8]) -> Result<usize, Self::Error> {
        let addr = addr.into_address().0;

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

        Ok(1)
    }

    #[inline]
    fn write(&mut self, _clock: Self::Instant, addr: Address, data: &[u8]) -> Result<usize, Self::Error> {
        let addr = addr.into_address().0;

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
        Ok(1)
    }
}

impl FromAddress<u64> for DeviceAddress {
    fn from_address(address: u64) -> Self {
        Self(address as u8)
    }
}

impl DeviceInterface for AtaDevice<MoaError> {
    fn as_bus_access(&mut self) -> Option<&mut MoaBus> {
        Some(self)
    }
}

/*
pub struct MoaAtaDevice(BusAdapter<u64, u8, AtaDevice, Error>);

impl Default for MoaAtaDevice {
    fn default() -> Self {
        MoaAtaDevice(BusAdapter::new(AtaDevice::default(), |addr| addr as u8, |err| Error::new(format!("{:?}", err))))
    }
}

impl DeviceInterface for MoaAtaDevice {
    fn as_bus_access(&mut self) -> Option<&mut MoaBus> {
        Some(&mut self.0)
    }
}

impl Deref for MoaAtaDevice {
    type Target = AtaDevice;

    fn deref(&self) -> &Self::Target {
        &self.0.bus
    }
}

impl DerefMut for MoaAtaDevice {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0.bus
    }
}
*/

//// OLD INTERFACE

/*
impl Addressable for AtaDevice<u64> {
    fn size(&self) -> usize {
        self.address_space()
    }

    fn read(&mut self, clock: Instant, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        <Self as BusAccess<u8>>::read(self, clock, addr as u8, data)
            .map_err(|err| Error::new(format!("{:?}", err)))?;
        Ok(())
    }

    fn write(&mut self, clock: Instant, addr: Address, data: &[u8]) -> Result<(), Error> {
        <Self as BusAccess<u8>>::write(self, clock, addr as u8, data)
            .map_err(|err| Error::new(format!("{:?}", err)))?;
        Ok(())
    }
}

impl Transmutable for AtaDevice<u64> {
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        Some(self)
    }
}
*/
