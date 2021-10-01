
#[macro_use]
mod error;
mod memory;
mod cpus;
mod devices;

use crate::memory::{AddressSpace, MemoryBlock};
use crate::cpus::m68k::MC68010;
use crate::devices::mc68681::MC68681;

fn main() {
    let mut space = AddressSpace::new();
    let monitor = MemoryBlock::load("monitor.bin").unwrap();
    for byte in monitor.contents.iter() {
        print!("{:02x} ", byte);
    }
    space.insert(0x00000000, Box::new(monitor));

    let ram = MemoryBlock::new(vec![0; 0x00100000]);
    space.insert(0x00100000, Box::new(ram));

    let mut serial = MC68681::new();
    serial.open().unwrap();
    space.insert(0x00700000, Box::new(serial));

    let mut cpu = MC68010::new();
    //cpu.set_breakpoint(0x02bc);
    cpu.use_tracing = true;
    while cpu.is_running() {
        match cpu.step(&mut space) {
            Ok(()) => { },
            Err(err) => {
                cpu.dump_state(&mut space);
                panic!("{:?}", err);
            },
        }

        //serial.step();
    }
}

