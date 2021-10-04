
use std::fs;
use std::slice::Iter;

use crate::error::Error;


pub type Address = u64;

pub trait Addressable {
    fn len(&self) -> usize;
    fn read(&mut self, addr: Address, count: usize) -> Vec<u8>;
    fn write(&mut self, addr: Address, data: &[u8]);
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

    fn read(&mut self, addr: Address, count: usize) -> Vec<u8> {
        self.contents[(addr as usize) .. (addr as usize + count)].to_vec()
    }

    fn write(&mut self, mut addr: Address, data: &[u8]) {
        for byte in data {
            self.contents[addr as usize] = *byte;
            addr += 1;
        }
    }
}



pub struct Segment {
    pub base: Address,
    pub contents: Box<dyn Addressable>,
}

impl Segment {
    pub fn new(base: Address, contents: Box<dyn Addressable>) -> Segment {
        Segment {
            base,
            contents,
        }
    }
}

pub struct AddressSpace {
    pub segments: Vec<Segment>,
}

impl AddressSpace {
    pub fn new() -> AddressSpace {
        AddressSpace {
            segments: vec!(),
        }
    }

    pub fn insert(&mut self, base: Address, contents: Box<dyn Addressable>) {
        let seg = Segment::new(base, contents);
        for i in 0..self.segments.len() {
            if self.segments[i].base > seg.base {
                self.segments.insert(i, seg);
                return;
            }
        }
        self.segments.insert(0, seg);
    }

    pub fn get_segment(&self, addr: Address) -> Result<&Segment, Error> {
        for i in 0..self.segments.len() {
            if addr >= self.segments[i].base && addr <= (self.segments[i].base + self.segments[i].contents.len() as Address) {
                return Ok(&self.segments[i]);
            }
        }
        return Err(Error::new(&format!("No segment found at {:#08x}", addr)));
    }

    pub fn get_segment_mut(&mut self, addr: Address) -> Result<&mut Segment, Error> {
        for i in 0..self.segments.len() {
            if addr >= self.segments[i].base && addr <= (self.segments[i].base + self.segments[i].contents.len() as Address) {
                return Ok(&mut self.segments[i]);
            }
        }
        return Err(Error::new(&format!("No segment found at {:#08x}", addr)));
    }


    pub fn dump_memory(&mut self, mut addr: Address, mut count: Address) {
        while count > 0 {
            let mut line = format!("{:#010x}: ", addr);

            let to = if count < 16 { count / 2 } else { 8 };
            for i in 0..to {
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


    pub fn read(&mut self, addr: Address, count: usize) -> Result<Vec<u8>, Error> {
        let mut seg = self.get_segment_mut(addr)?;
        let relative_addr = addr - seg.base;
        if relative_addr as usize + count > seg.contents.len() {
            Err(Error::new(&format!("Error reading address {:#010x}", addr)))
        } else {
            Ok(seg.contents.read(relative_addr, count))
        }
    }

    pub fn read_u8(&mut self, addr: Address) -> Result<u8, Error> {
        Ok(self.read(addr, 1)?[0])
    }

    pub fn read_beu16(&mut self, addr: Address) -> Result<u16, Error> {
        Ok(read_beu16(&self.read(addr, 2)?))
    }

    pub fn read_beu32(&mut self, addr: Address) -> Result<u32, Error> {
        Ok(read_beu32(&self.read(addr, 4)?))
    }


    pub fn write(&mut self, addr: Address, data: &[u8]) -> Result<(), Error> {
        let seg = self.get_segment_mut(addr)?;
        Ok(seg.contents.write(addr - seg.base, data))
    }

    pub fn write_u8(&mut self, addr: Address, value: u8) -> Result<(), Error> {
        let data = [value];
        self.write(addr, &data)
    }

    pub fn write_beu16(&mut self, addr: Address, value: u16) -> Result<(), Error> {
        let data = [
            (value >> 8) as u8,
            value as u8,
        ];
        self.write(addr, &data)
    }

    pub fn write_beu32(&mut self, addr: Address, value: u32) -> Result<(), Error> {
        let data = [
            (value >> 24) as u8,
            (value >> 16) as u8,
            (value >> 8) as u8,
            value as u8,
        ];
        self.write(addr, &data)
    }
}

#[inline(always)]
pub fn read_beu16(mut data: &[u8]) -> u16 {
    (data[0] as u16) << 8 |
    (data[1] as u16)
}

#[inline(always)]
pub fn read_beu32(mut data: &[u8]) -> u32 {
    (data[0] as u32) << 24 |
    (data[1] as u32) << 16 |
    (data[2] as u32) << 8 |
    (data[3] as u32)
}

