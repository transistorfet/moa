
use std::fs;
use std::slice::Iter;

use crate::error::Error;


pub type Address = u64;

trait Addressable {
    fn read(&self, addr: Address) -> Iter<u8>;
    fn write(&mut self, addr: Address, data: &[u8]);
}


pub struct Segment {
    pub base: Address,
    pub contents: Vec<u8>,
}

impl Segment {
    pub fn new(base: Address, contents: Vec<u8>) -> Segment {
        Segment {
            base,
            contents,
        }
    }

    pub fn load(base: Address, filename: &str) -> Result<Segment, Error> {
        match fs::read(filename) {
            Ok(contents) => Ok(Segment::new(base, contents)),
            Err(_) => Err(Error::new(&format!("Error reading contents of {}", filename))),
        }
    }
}

impl Addressable for Segment {
    fn read(&self, addr: Address) -> Iter<u8> {
        self.contents[(addr - self.base) as usize .. ].iter()
    }

    fn write(&mut self, addr: Address, data: &[u8]) {
        for byte in data {
            self.contents[(addr - self.base) as usize] = *byte;
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

    pub fn insert(&mut self, seg: Segment) {
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
        return Err(Error::new(&format!("No segment found at {:08x}", addr)));
    }

    pub fn get_segment_mut(&mut self, addr: Address) -> Result<&mut Segment, Error> {
        for i in 0..self.segments.len() {
            if addr >= self.segments[i].base && addr <= (self.segments[i].base + self.segments[i].contents.len() as Address) {
                return Ok(&mut self.segments[i]);
            }
        }
        return Err(Error::new(&format!("No segment found at {:08x}", addr)));
    }


    pub fn read_u8(&self, addr: Address) -> Result<u8, Error> {
        let seg = self.get_segment(addr)?;
        Ok(*seg.read(addr).next().unwrap())
    }

    pub fn read_beu16(&self, addr: Address) -> Result<u16, Error> {
        let seg = self.get_segment(addr)?;
        Ok(read_beu16(seg.read(addr)))
    }

    pub fn read_beu32(&self, addr: Address) -> Result<u32, Error> {
        let seg = self.get_segment(addr)?;
        Ok(read_beu32(seg.read(addr)))
    }

    pub fn write_u8(&mut self, addr: Address, value: u8) -> Result<(), Error> {
        let seg = self.get_segment_mut(addr)?;
        let data = [value];
        Ok(seg.write(addr, &data))
    }

    pub fn write_beu16(&mut self, addr: Address, value: u16) -> Result<(), Error> {
        let seg = self.get_segment_mut(addr)?;
        let data = [
            (value >> 8) as u8,
            value as u8,
        ];
        Ok(seg.write(addr, &data))
    }

    pub fn write_beu32(&mut self, addr: Address, value: u32) -> Result<(), Error> {
        let seg = self.get_segment_mut(addr)?;
        let data = [
            (value >> 24) as u8,
            (value >> 16) as u8,
            (value >> 8) as u8,
            value as u8,
        ];
        Ok(seg.write(addr, &data))
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

