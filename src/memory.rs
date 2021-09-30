
use std::fs;
use std::slice::Iter;

use crate::error::Error;


pub type Address = u64;

pub trait Addressable {
    fn len(&self) -> usize;
    fn read(&self, addr: Address) -> Iter<u8>;
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
}

impl Addressable for MemoryBlock {
    fn len(&self) -> usize {
        self.contents.len()
    }

    fn read(&self, addr: Address) -> Iter<u8> {
        self.contents[(addr) as usize .. ].iter()
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


    pub fn read(&self, addr: Address) -> Result<Iter<u8>, Error> {
        let seg = self.get_segment(addr)?;
        Ok(seg.contents.read(addr - seg.base))
    }

    pub fn read_u8(&self, addr: Address) -> Result<u8, Error> {
        let seg = self.get_segment(addr)?;
        Ok(*seg.contents.read(addr - seg.base).next().unwrap())
    }

    pub fn read_beu16(&self, addr: Address) -> Result<u16, Error> {
        let seg = self.get_segment(addr)?;
        Ok(read_beu16(seg.contents.read(addr - seg.base)))
    }

    pub fn read_beu32(&self, addr: Address) -> Result<u32, Error> {
        let seg = self.get_segment(addr)?;
        Ok(read_beu32(seg.contents.read(addr - seg.base)))
    }


    pub fn write_u8(&mut self, addr: Address, value: u8) -> Result<(), Error> {
        let seg = self.get_segment_mut(addr)?;
        let data = [value];
        Ok(seg.contents.write(addr - seg.base, &data))
    }

    pub fn write_beu16(&mut self, addr: Address, value: u16) -> Result<(), Error> {
        let seg = self.get_segment_mut(addr)?;
        let data = [
            (value >> 8) as u8,
            value as u8,
        ];
        Ok(seg.contents.write(addr - seg.base, &data))
    }

    pub fn write_beu32(&mut self, addr: Address, value: u32) -> Result<(), Error> {
        let seg = self.get_segment_mut(addr)?;
        let data = [
            (value >> 24) as u8,
            (value >> 16) as u8,
            (value >> 8) as u8,
            value as u8,
        ];
        Ok(seg.contents.write(addr - seg.base, &data))
    }
}

pub fn read_beu16(mut iter: Iter<u8>) -> u16 {
    (*iter.next().unwrap() as u16) << 8 |
    (*iter.next().unwrap() as u16)
}

pub fn read_beu32(mut iter: Iter<u8>) -> u32 {
    (*iter.next().unwrap() as u32) << 24 |
    (*iter.next().unwrap() as u32) << 16 |
    (*iter.next().unwrap() as u32) << 8 |
    (*iter.next().unwrap() as u32)
}

