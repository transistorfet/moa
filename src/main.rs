
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
    //cpu.add_breakpoint(0x07f8);
    //cpu.add_breakpoint(0x0836);
    //cpu.add_breakpoint(0x0838);
    //cpu.add_breakpoint(0x0ea0);

    cpu.add_breakpoint(0x0034);
    cpu.enable_tracing();
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

    // TODO I need to add a way to decode and dump the assembly for a section of code, in debugger
    /*
    cpu.state.pc = 0x07f8;
    while cpu.is_running() {
        cpu.decode_next(&mut space).unwrap();
    }
    */
}

