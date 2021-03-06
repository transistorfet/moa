
use std::fs;
use std::rc::Rc;
use std::cell::RefCell;

use crate::error::Error;
use crate::devices::{Address, Addressable, Transmutable, TransmutableBox, read_beu16};


pub struct MemoryBlock {
    read_only: bool,
    contents: Vec<u8>,
}

impl MemoryBlock {
    pub fn new(contents: Vec<u8>) -> MemoryBlock {
        MemoryBlock {
            read_only: false,
            contents
        }
    }

    pub fn load(filename: &str) -> Result<MemoryBlock, Error> {
        match fs::read(filename) {
            Ok(contents) => Ok(MemoryBlock::new(contents)),
            Err(_) => Err(Error::new(&format!("Error reading contents of {}", filename))),
        }
    }

    pub fn load_at(&mut self, addr: Address, filename: &str) -> Result<(), Error> {
        match fs::read(filename) {
            Ok(contents) => {
                for i in 0..contents.len() {
                    self.contents[(addr as usize) + i] = contents[i];
                }
                Ok(())
            },
            Err(_) => Err(Error::new(&format!("Error reading contents of {}", filename))),
        }
    }

    pub fn read_only(&mut self) {
        self.read_only = true;
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

    fn write(&mut self, addr: Address, data: &[u8]) -> Result<(), Error> {
        if self.read_only {
            return Err(Error::breakpoint(&format!("Attempt to write to read-only memory at {:x} with data {:?}", addr, data)));
        }

        for i in 0..data.len() {
            self.contents[(addr as usize) + i] = data[i];
        }
        Ok(())
    }
}

impl Transmutable for MemoryBlock {
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        Some(self)
    }
}


pub struct AddressAdapter {
    subdevice: TransmutableBox,
    shift: u8,
}

impl AddressAdapter {
    pub fn new(subdevice: TransmutableBox, shift: u8) -> Self {
        Self {
            subdevice,
            shift,
        }
    }
}

impl Addressable for AddressAdapter {
    fn len(&self) -> usize {
        let len = self.subdevice.borrow_mut().as_addressable().unwrap().len();
        len << self.shift
    }

    fn read(&mut self, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        self.subdevice.borrow_mut().as_addressable().unwrap().read(addr >> self.shift, data)
    }

    fn write(&mut self, addr: Address, data: &[u8]) -> Result<(), Error> {
        self.subdevice.borrow_mut().as_addressable().unwrap().write(addr >> self.shift, data)
    }
}

impl Transmutable for AddressAdapter {
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
    blocks: Vec<Block>,
    ignore_unmapped: bool,
    watchers: Vec<Address>,
    watcher_modified: bool,
}

impl Bus {
    pub fn new() -> Bus {
        Bus {
            ignore_unmapped: false,
            blocks: vec!(),
            watchers: vec!(),
            watcher_modified: false,
        }
    }

    pub fn set_ignore_unmapped(&mut self, ignore_unmapped: bool) {
        self.ignore_unmapped = ignore_unmapped;
    }

    pub fn clear_all_bus_devices(&mut self) {
        self.blocks.clear();
    }

    pub fn insert(&mut self, base: Address, dev: TransmutableBox) {
        let length = dev.borrow_mut().as_addressable().unwrap().len();
        let block = Block { base, length, dev };
        let i = self.blocks.iter().position(|cur| cur.base > block.base).unwrap_or(self.blocks.len());
        self.blocks.insert(i, block);
    }

    pub fn get_device_at(&self, addr: Address, count: usize) -> Result<(TransmutableBox, Address), Error> {
        for block in &self.blocks {
            if addr >= block.base && addr < (block.base + block.length as Address) {
                let relative_addr = addr - block.base;
                if relative_addr as usize + count <= block.length {
                    return Ok((block.dev.clone(), relative_addr));
                } else {
                    return Err(Error::new(&format!("Error reading address {:#010x}", addr)));
                }
            }
        }
        return Err(Error::new(&format!("No segment found at {:#010x}", addr)));
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

    pub fn add_watcher(&mut self, addr: Address) {
        self.watchers.push(addr);
    }

    pub fn remove_watcher(&mut self, addr: Address) {
        self.watchers.push(addr);
        if let Some(index) = self.watchers.iter().position(|a| *a == addr) {
            self.watchers.remove(index);
        }
    }

    pub fn check_and_reset_watcher_modified(&mut self) -> bool {
        let result = self.watcher_modified;
        self.watcher_modified = false;
        result
    }
}

impl Addressable for Bus {
    fn len(&self) -> usize {
        let block = &self.blocks[self.blocks.len() - 1];
        (block.base as usize) + block.length
    }

    fn read(&mut self, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        let (dev, relative_addr) = match self.get_device_at(addr, data.len()) {
            Ok(result) => result,
            Err(err) if self.ignore_unmapped => {
                info!("{:?}", err);
                return Ok(())
            },
            Err(err) => return Err(err),
        };
        let result = dev.borrow_mut().as_addressable().unwrap().read(relative_addr, data);
        result
    }

    fn write(&mut self, addr: Address, data: &[u8]) -> Result<(), Error> {
        if let Some(_) = self.watchers.iter().position(|a| *a == addr) {
            println!("watch: writing to address {:#06x} with {:?}", addr, data);
            self.watcher_modified = true;
        }

        let (dev, relative_addr) = match self.get_device_at(addr, data.len()) {
            Ok(result) => result,
            Err(err) if self.ignore_unmapped => {
                info!("{:?}", err);
                return Ok(())
            },
            Err(err) => return Err(err),
        };
        let result = dev.borrow_mut().as_addressable().unwrap().write(relative_addr, data);
        result
    }
}

pub struct BusPort {
    offset: Address,
    address_mask: Address,
    data_width: u8,
    subdevice: Rc<RefCell<Bus>>,
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

    pub fn dump_memory(&mut self, addr: Address, count: Address) {
        self.subdevice.borrow_mut().dump_memory(self.offset + (addr & self.address_mask), count)
    }

    pub fn data_width(&self) -> u8 {
        self.data_width
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

pub fn dump_slice(data: &[u8], mut count: usize) {
    let mut addr = 0;
    while count > 0 {
        let mut line = format!("{:#010x}: ", addr);

        let to = if count < 16 { count / 2 } else { 8 };
        for _ in 0..to {
            let word = read_beu16(&data[addr..]);
            line += &format!("{:#06x} ", word);
            addr += 2;
            count -= 2;
        }
        println!("{}", line);
    }
}

