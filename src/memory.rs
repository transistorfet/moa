
use std::fs;

use crate::error::Error;
use crate::system::{Clock, DeviceNumber, Device, System};


pub type Address = u64;

pub trait Addressable {
    fn len(&self) -> usize;
    fn read(&mut self, addr: Address, count: usize) -> Result<Vec<u8>, Error>;
    fn write(&mut self, addr: Address, data: &[u8]) -> Result<(), Error>;

    fn read_u8(&mut self, addr: Address) -> Result<u8, Error> {
        Ok(self.read(addr, 1)?[0])
    }

    fn read_beu16(&mut self, addr: Address) -> Result<u16, Error> {
        Ok(read_beu16(&self.read(addr, 2)?))
    }

    fn read_beu32(&mut self, addr: Address) -> Result<u32, Error> {
        Ok(read_beu32(&self.read(addr, 4)?))
    }

    fn write_u8(&mut self, addr: Address, value: u8) -> Result<(), Error> {
        let data = [value];
        self.write(addr, &data)
    }

    fn write_beu16(&mut self, addr: Address, value: u16) -> Result<(), Error> {
        let data = write_beu16(value);
        self.write(addr, &data)
    }

    fn write_beu32(&mut self, addr: Address, value: u32) -> Result<(), Error> {
        let data = write_beu32(value);
        self.write(addr, &data)
    }
}


pub struct MemoryBlock {
    pub contents: Vec<u8>,
}

impl MemoryBlock {
    pub fn new(contents: Vec<u8>) -> MemoryBlock {
        MemoryBlock {
            contents
        }
    }

    pub fn load(filename: &str) -> Result<MemoryBlock, Error> {
        match fs::read(filename) {
            Ok(contents) => Ok(MemoryBlock::new(contents)),
            Err(_) => Err(Error::new(&format!("Error reading contents of {}", filename))),
        }
    }

    pub fn load_at(&mut self, mut addr: Address, filename: &str) -> Result<(), Error> {
        match fs::read(filename) {
            Ok(contents) => {
                for byte in contents {
                    self.contents[addr as usize] = byte;
                    addr += 1;
                }
                Ok(())
            },
            Err(_) => Err(Error::new(&format!("Error reading contents of {}", filename))),
        }
    }
}

impl Addressable for MemoryBlock {
    fn len(&self) -> usize {
        self.contents.len()
    }

    fn read(&mut self, addr: Address, count: usize) -> Result<Vec<u8>, Error> {
        Ok(self.contents[(addr as usize) .. (addr as usize + count)].to_vec())
    }

    fn write(&mut self, mut addr: Address, data: &[u8]) -> Result<(), Error> {
        for byte in data {
            self.contents[addr as usize] = *byte;
            addr += 1;
        }
        Ok(())
    }
}

impl Device for MemoryBlock {
    fn step(&mut self, _system: &System) -> Result<Clock, Error> {
        Ok(1)
    }
}


pub struct Block {
    pub base: Address,
    pub length: usize,
    pub dev: DeviceNumber,
}

pub struct Bus {
    pub blocks: Vec<Block>,
}

impl Bus {
    pub fn new() -> Bus {
        Bus {
            blocks: vec!(),
        }
    }

    pub fn insert(&mut self, base: Address, length: usize, dev: DeviceNumber) {
        let block = Block { base, length, dev };
        for i in 0..self.blocks.len() {
            if self.blocks[i].base > block.base {
                self.blocks.insert(i, block);
                return;
            }
        }
        self.blocks.insert(0, block);
    }

    pub fn get_device_at(&self, addr: Address, count: usize) -> Result<(DeviceNumber, Address), Error> {
        for block in &self.blocks {
            if addr >= block.base && addr <= (block.base + block.length as Address) {
                let relative_addr = addr - block.base;
                if relative_addr as usize + count <= block.length {
                    return Ok((block.dev, relative_addr));
                } else {
                    return Err(Error::new(&format!("Error reading address {:#010x}", addr)));
                }
            }
        }
        return Err(Error::new(&format!("No segment found at {:#08x}", addr)));
    }

    pub fn max_address(&self) -> Address {
        let block = &self.blocks[self.blocks.len() - 1];
        block.base + block.length as Address
    }
}


#[inline(always)]
pub fn read_beu16(data: &[u8]) -> u16 {
    (data[0] as u16) << 8 |
    (data[1] as u16)
}

#[inline(always)]
pub fn read_beu32(data: &[u8]) -> u32 {
    (data[0] as u32) << 24 |
    (data[1] as u32) << 16 |
    (data[2] as u32) << 8 |
    (data[3] as u32)
}

#[inline(always)]
pub fn write_beu16(value: u16) -> [u8; 2] {
    [
        (value >> 8) as u8,
        value as u8,
    ]
}

#[inline(always)]
pub fn write_beu32(value: u32) -> [u8; 4] {
    [
        (value >> 24) as u8,
        (value >> 16) as u8,
        (value >> 8) as u8,
        value as u8,
    ]
}

