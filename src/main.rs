
#[macro_use]
mod error;
mod memory;
mod m68k;

use crate::memory::{AddressSpace, Segment};
use crate::m68k::MC68010;

fn main() {
    let mut space = AddressSpace::new();
    let monitor = Segment::load(0x00000000, "monitor.bin").unwrap();
    for byte in monitor.contents.iter() {
        print!("{:02x} ", byte);
    }
    space.insert(monitor);


    let mut cpu = MC68010::new();
    while cpu.is_running() {
        cpu.step(&mut space).unwrap();
    }
}

