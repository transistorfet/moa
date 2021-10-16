
use std::fs;

use crate::error::Error;
use crate::system::System;
use crate::devices::{Clock, Steppable, AddressableDeviceBox};


pub const MAX_READ: usize = 4;

pub type Address = u64;

/// A device that can be addressed to read data from or write data to the device.
pub trait Addressable {
    fn len(&self) -> usize;
    fn read(&mut self, addr: Address, count: usize) -> Result<[u8; MAX_READ], Error>;
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

    fn read(&mut self, addr: Address, count: usize) -> Result<[u8; MAX_READ], Error> {
        let mut data = [0; MAX_READ];
        //self.contents[(addr as usize) .. (addr as usize + 4)].clone_from_slice(&data);
        for i in 0..std::cmp::min(count, MAX_READ) {
            data[i] = self.contents[(addr as usize) + i];
        }
        Ok(data)
    }

    fn write(&mut self, mut addr: Address, data: &[u8]) -> Result<(), Error> {
        for byte in data {
            self.contents[addr as usize] = *byte;
            addr += 1;
        }
        Ok(())
    }
}

impl Steppable for MemoryBlock {
    fn step(&mut self, _system: &System) -> Result<Clock, Error> {
        Ok(1)
    }
}


pub struct Block {
    pub base: Address,
    pub length: usize,
    pub dev: AddressableDeviceBox,
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

    pub fn insert(&mut self, base: Address, length: usize, dev: AddressableDeviceBox) {
        let block = Block { base, length, dev };
        for i in 0..self.blocks.len() {
            if self.blocks[i].base > block.base {
                self.blocks.insert(i, block);
                return;
            }
        }
        self.blocks.insert(0, block);
    }

    pub fn get_device_at(&self, addr: Address, count: usize) -> Result<(AddressableDeviceBox, Address), Error> {
        for block in &self.blocks {
            if addr >= block.base && addr <= (block.base + block.length as Address) {
                let relative_addr = addr - block.base;
                if relative_addr as usize + count <= block.length {
                    return Ok((block.dev.clone(), relative_addr));
                } else {
                    return Err(Error::new(&format!("Error reading address {:#010x}", addr)));
                }
            }
        }
        return Err(Error::new(&format!("No segment found at {:#08x}", addr)));
    }

    pub fn dump_memory(&mut self, mut addr: Address, mut count: Address) {
        while count > 0 {
            let mut line = format!("{:#010x}: ", addr);

            let to = if count < 16 { count / 2 } else { 8 };
            for _ in 0..to {
                let word = self.read_beu16(addr);
                if word.is_err() {
                    println!("{}", line);
                    return;
                }
                line += &format!("{:#06x} ", word.unwrap());
                addr += 2;
                count -= 2;
            }
            println!("{}", line);
        }
    }
}

impl Addressable for Bus {
    fn len(&self) -> usize {
        let block = &self.blocks[self.blocks.len() - 1];
        (block.base as usize) + block.length
    }

    fn read(&mut self, addr: Address, count: usize) -> Result<[u8; MAX_READ], Error> {
        let (dev, relative_addr) = self.get_device_at(addr, count)?;
        let result = dev.borrow_mut().read(relative_addr, count);
        result
    }

    fn write(&mut self, addr: Address, data: &[u8]) -> Result<(), Error> {
        let (dev, relative_addr) = self.get_device_at(addr, data.len())?;
        let result = dev.borrow_mut().write(relative_addr, data);
        result
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

