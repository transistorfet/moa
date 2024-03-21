use std::fs;
use std::cmp;
use std::rc::Rc;
use std::cell::RefCell;
use std::fmt::Write;
use femtos::Instant;

use crate::error::Error;
use crate::devices::{Address, Addressable, Transmutable, Device, read_beu16};


/// A contiguous block of `Addressable` memory, backed by a `Vec`
pub struct MemoryBlock {
    read_only: bool,
    contents: Vec<u8>,
}

impl MemoryBlock {
    pub fn new(contents: Vec<u8>) -> MemoryBlock {
        MemoryBlock {
            read_only: false,
            contents,
        }
    }

    pub fn load(filename: &str) -> Result<MemoryBlock, Error> {
        match fs::read(filename) {
            Ok(contents) => Ok(MemoryBlock::new(contents)),
            Err(_) => Err(Error::new(format!("Error reading contents of {}", filename))),
        }
    }

    pub fn load_at(&mut self, addr: Address, filename: &str) -> Result<(), Error> {
        match fs::read(filename) {
            Ok(contents) => {
                self.contents[(addr as usize)..(addr as usize) + contents.len()].copy_from_slice(&contents);
                Ok(())
            },
            Err(_) => Err(Error::new(format!("Error reading contents of {}", filename))),
        }
    }

    pub fn read_only(&mut self) {
        self.read_only = true;
    }

    pub fn resize(&mut self, new_size: usize) {
        self.contents.resize(new_size, 0);
    }
}

impl Addressable for MemoryBlock {
    fn size(&self) -> usize {
        self.contents.len()
    }

    fn read(&mut self, _clock: Instant, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        data.copy_from_slice(&self.contents[(addr as usize)..(addr as usize) + data.len()]);
        Ok(())
    }

    fn write(&mut self, _clock: Instant, addr: Address, data: &[u8]) -> Result<(), Error> {
        if self.read_only {
            return Err(Error::breakpoint(format!(
                "Attempt to write to read-only memory at {:x} with data {:?}",
                addr, data
            )));
        }

        self.contents[(addr as usize)..(addr as usize) + data.len()].copy_from_slice(data);
        Ok(())
    }
}

impl Transmutable for MemoryBlock {
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        Some(self)
    }
}


/// An address adapter that repeats the address space of the subdevice over the given range
pub struct AddressRepeater {
    subdevice: Device,
    range: Address,
}

impl AddressRepeater {
    pub fn new(subdevice: Device, range: Address) -> Self {
        Self {
            subdevice,
            range,
        }
    }
}

impl Addressable for AddressRepeater {
    fn size(&self) -> usize {
        self.range as usize
    }

    fn read(&mut self, clock: Instant, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        let size = self.subdevice.borrow_mut().as_addressable().unwrap().size() as Address;
        self.subdevice
            .borrow_mut()
            .as_addressable()
            .unwrap()
            .read(clock, addr % size, data)
    }

    fn write(&mut self, clock: Instant, addr: Address, data: &[u8]) -> Result<(), Error> {
        let size = self.subdevice.borrow_mut().as_addressable().unwrap().size() as Address;
        self.subdevice
            .borrow_mut()
            .as_addressable()
            .unwrap()
            .write(clock, addr % size, data)
    }
}

impl Transmutable for AddressRepeater {
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        Some(self)
    }
}


/// An address adapter that uses a closure to translate the address before accessing the subdevice
pub struct AddressTranslator {
    subdevice: Device,
    size: usize,
    func: Box<dyn Fn(Address) -> Address>,
}

impl AddressTranslator {
    pub fn new<F>(subdevice: Device, size: usize, func: F) -> Self
    where
        F: Fn(Address) -> Address + 'static,
    {
        Self {
            subdevice,
            size,
            func: Box::new(func),
        }
    }
}

impl Addressable for AddressTranslator {
    fn size(&self) -> usize {
        self.size
    }

    fn read(&mut self, clock: Instant, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        self.subdevice
            .borrow_mut()
            .as_addressable()
            .unwrap()
            .read(clock, (self.func)(addr), data)
    }

    fn write(&mut self, clock: Instant, addr: Address, data: &[u8]) -> Result<(), Error> {
        self.subdevice
            .borrow_mut()
            .as_addressable()
            .unwrap()
            .write(clock, (self.func)(addr), data)
    }
}

impl Transmutable for AddressTranslator {
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        Some(self)
    }
}


#[derive(Clone)]
pub struct Block {
    pub base: Address,
    pub size: usize,
    pub dev: Device,
}

/// A bus-like collection of `Addressable` `Device`s mapped to different address ranges
///
/// This is the fundamental means of connecting devices together to a CPU implementation.
#[derive(Clone, Default)]
pub struct Bus {
    blocks: Vec<Block>,
    ignore_unmapped: bool,
    watchers: Vec<Address>,
    watcher_modified: bool,
}

impl Bus {
    pub fn set_ignore_unmapped(&mut self, ignore_unmapped: bool) {
        self.ignore_unmapped = ignore_unmapped;
    }

    pub fn clear_all_bus_devices(&mut self) {
        self.blocks.clear();
    }

    pub fn insert(&mut self, base: Address, dev: Device) {
        let size = dev.borrow_mut().as_addressable().unwrap().size();
        let block = Block {
            base,
            size,
            dev,
        };
        let i = self
            .blocks
            .iter()
            .position(|cur| cur.base > block.base)
            .unwrap_or(self.blocks.len());
        self.blocks.insert(i, block);
    }

    pub fn get_device_at(&self, addr: Address, count: usize) -> Result<(Device, Address), Error> {
        for block in &self.blocks {
            if addr >= block.base && addr < (block.base + block.size as Address) {
                let relative_addr = addr - block.base;
                if relative_addr as usize + count <= block.size {
                    return Ok((block.dev.clone(), relative_addr));
                } else {
                    return Err(Error::new(format!("Error reading address {:#010x}", addr)));
                }
            }
        }
        Err(Error::new(format!("No segment found at {:#010x}", addr)))
    }

    pub fn dump_memory(&mut self, clock: Instant, mut addr: Address, mut count: Address) {
        while count > 0 {
            let mut line = format!("{:#010x}: ", addr);

            let to = if count < 16 { count / 2 } else { 8 };
            for _ in 0..to {
                let word = self.read_beu16(clock, addr);
                if word.is_err() {
                    println!("{}", line);
                    return;
                }
                write!(line, "{:#06x} ", word.unwrap()).unwrap();
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
    fn size(&self) -> usize {
        let block = &self.blocks[self.blocks.len() - 1];
        (block.base as usize) + block.size
    }

    fn read(&mut self, clock: Instant, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        let (dev, relative_addr) = match self.get_device_at(addr, data.len()) {
            Ok(result) => result,
            Err(err) if self.ignore_unmapped => {
                log::info!("{:?}", err);
                return Ok(());
            },
            Err(err) => return Err(err),
        };
        let result = dev.borrow_mut().as_addressable().unwrap().read(clock, relative_addr, data);
        result
    }

    fn write(&mut self, clock: Instant, addr: Address, data: &[u8]) -> Result<(), Error> {
        if self.watchers.iter().any(|a| *a == addr) {
            println!("watch: writing to address {:#06x} with {:?}", addr, data);
            self.watcher_modified = true;
        }

        let (dev, relative_addr) = match self.get_device_at(addr, data.len()) {
            Ok(result) => result,
            Err(err) if self.ignore_unmapped => {
                log::info!("{:?}", err);
                return Ok(());
            },
            Err(err) => return Err(err),
        };
        let result = dev.borrow_mut().as_addressable().unwrap().write(clock, relative_addr, data);
        result
    }
}

/// An adapter for limiting the access requests of a device (eg. CPU) on a `Bus` to the address
/// and data widths of the device
#[derive(Clone)]
pub struct BusPort {
    offset: Address,
    address_mask: Address,
    data_width: u8,
    subdevice: Rc<RefCell<Bus>>,
}

impl BusPort {
    pub fn new(offset: Address, address_bits: u8, data_bits: u8, bus: Rc<RefCell<Bus>>) -> Self {
        Self {
            offset,
            address_mask: (1 << address_bits) - 1,
            data_width: data_bits / 8,
            subdevice: bus,
        }
    }

    pub fn dump_memory(&mut self, clock: Instant, addr: Address, count: Address) {
        self.subdevice
            .borrow_mut()
            .dump_memory(clock, self.offset + (addr & self.address_mask), count)
    }

    #[inline]
    pub fn address_mask(&self) -> Address {
        self.address_mask
    }

    #[inline]
    pub fn data_width(&self) -> u8 {
        self.data_width
    }
}

impl Addressable for BusPort {
    fn size(&self) -> usize {
        self.subdevice.borrow().size()
    }

    fn read(&mut self, clock: Instant, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        let addr = self.offset + (addr & self.address_mask);
        let mut subdevice = self.subdevice.borrow_mut();
        for i in (0..data.len()).step_by(self.data_width as usize) {
            let addr_index = (addr + i as Address) & self.address_mask;
            let end = cmp::min(i + self.data_width as usize, data.len());
            subdevice.read(clock, addr_index, &mut data[i..end])?;
        }
        Ok(())
    }

    fn write(&mut self, clock: Instant, addr: Address, data: &[u8]) -> Result<(), Error> {
        let addr = self.offset + (addr & self.address_mask);
        let mut subdevice = self.subdevice.borrow_mut();
        for i in (0..data.len()).step_by(self.data_width as usize) {
            let addr_index = (addr + i as Address) & self.address_mask;
            let end = cmp::min(i + self.data_width as usize, data.len());
            subdevice.write(clock, addr_index, &data[i..end])?;
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
            write!(line, "{:#06x} ", word).unwrap();
            addr += 2;
            count -= 2;
        }
        println!("{}", line);
    }
}

pub fn dump_memory<Bus, Address, Instant>(bus: &mut Bus, clock: Instant, addr: Address, count: Address)
where
    Bus: BusAccess<Address, Instant = Instant>,
    Address: From<u64> + Into<u64> + Copy,
    Instant: Copy,
{
    let mut addr = addr.into();
    let mut count = count.into();
    while count > 0 {
        let mut line = format!("{:#010x}: ", addr);

        let to = if count < 16 { count / 2 } else { 8 };
        for _ in 0..to {
            let word = bus.read_beu16(clock, Address::from(addr));
            if word.is_err() {
                println!("{}", line);
                return;
            }
            write!(line, "{:#06x} ", word.unwrap()).unwrap();
            addr += 2;
            count -= 2;
        }
        println!("{}", line);
    }
}

use emulator_hal::bus::{self, BusAccess};

impl bus::Error for Error {}

impl BusAccess<u64> for &mut dyn Addressable {
    type Instant = Instant;
    type Error = Error;

    fn read(&mut self, now: Instant, addr: Address, data: &mut [u8]) -> Result<usize, Self::Error> {
        (*self).read(now, addr, data)?;
        Ok(data.len())
    }

    fn write(&mut self, now: Instant, addr: Address, data: &[u8]) -> Result<usize, Self::Error> {
        (*self).write(now, addr, data)?;
        Ok(data.len())
    }
}
