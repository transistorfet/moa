
#[macro_use]
mod error;
mod memory;
mod cpus;

use crate::memory::{AddressSpace, Segment};
use crate::cpus::m68k::MC68010;

fn main() {
    let mut space = AddressSpace::new();
    let monitor = Segment::load(0x00000000, "monitor.bin").unwrap();
    for byte in monitor.contents.iter() {
        print!("{:02x} ", byte);
    }
    space.insert(monitor);

    let ram = Segment::new(0x00100000, vec![0; 0x00100000]);
    space.insert(ram);

    let serial = Segment::new(0x00700000, vec![0; 0x30]);
    space.insert(serial);

    let mut cpu = MC68010::new();
    while cpu.is_running() {
        match cpu.step(&mut space) {
            Ok(()) => { },
            Err(err) => {
                cpu.dump_state();
                panic!("{:?}", err);
            },
        }
    }
}

