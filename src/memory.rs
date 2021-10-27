
use std::fs;
use std::rc::Rc;
use std::cell::RefCell;

use crate::error::Error;
use crate::devices::{Address, Addressable, Transmutable, TransmutableBox, MAX_READ};


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

    fn read(&mut self, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        for i in 0..data.len() {
            data[i] = self.contents[(addr as usize) + i];
        }
        Ok(())
    }

    fn write(&mut self, mut addr: Address, data: &[u8]) -> Result<(), Error> {
        for byte in data {
            self.contents[addr as usize] = *byte;
            addr += 1;
        }
        Ok(())
    }
}

impl Transmutable for MemoryBlock {
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        Some(self)
    }
}



pub struct Block {
    pub base: Address,
    pub length: usize,
    pub dev: TransmutableBox,
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

    pub fn insert(&mut self, base: Address, length: usize, dev: TransmutableBox) {
        let block = Block { base, length, dev };
        for i in 0..self.blocks.len() {
            if self.blocks[i].base > block.base {
                self.blocks.insert(i, block);
                return;
            }
        }
        self.blocks.insert(0, block);
    }

    pub fn get_device_at(&self, addr: Address, count: usize) -> Result<(TransmutableBox, Address), Error> {
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

    fn read(&mut self, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        let (dev, relative_addr) = self.get_device_at(addr, data.len())?;
        let result = dev.borrow_mut().as_addressable().unwrap().read(relative_addr, data);
        result
    }

    fn write(&mut self, addr: Address, data: &[u8]) -> Result<(), Error> {
        let (dev, relative_addr) = self.get_device_at(addr, data.len())?;
        let result = dev.borrow_mut().as_addressable().unwrap().write(relative_addr, data);
        result
    }
}

pub struct BusPort {
    pub offset: Address,
    pub address_mask: Address,
    pub data_width: u8,
    pub subdevice: Rc<RefCell<Bus>>,
}

impl BusPort {
    pub fn new(offset: Address, address_bits: u8, data_bits: u8, bus: Rc<RefCell<Bus>>) -> Self {
        let mut address_mask = 0;
        for _ in 0..address_bits {
            address_mask = (address_mask << 1) | 0x01;
        }

        Self {
            offset,
            address_mask,
            data_width: data_bits / 8,
            subdevice: bus,
        }
    }

    pub fn dump_memory(&mut self, mut addr: Address, mut count: Address) {
        self.subdevice.borrow_mut().dump_memory(addr, count)
    }
}

impl Addressable for BusPort {
    fn len(&self) -> usize {
        self.subdevice.borrow().len()
    }

    fn read(&mut self, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        let addr = self.offset + (addr & self.address_mask);
        let mut subdevice = self.subdevice.borrow_mut();
        for i in (0..data.len()).step_by(self.data_width as usize) {
            let end = std::cmp::min(i + self.data_width as usize, data.len());
            subdevice.read(addr + i as Address, &mut data[i..end])?;
        }
        Ok(())
    }

    fn write(&mut self, addr: Address, data: &[u8]) -> Result<(), Error> {
        let addr = self.offset + (addr & self.address_mask);
        let mut subdevice = self.subdevice.borrow_mut();
        for i in (0..data.len()).step_by(self.data_width as usize) {
            let end = std::cmp::min(i + self.data_width as usize, data.len());
            subdevice.write(addr + i as Address, &data[i..end])?;
        }
        Ok(())
    }
}

