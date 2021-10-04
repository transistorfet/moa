
#[macro_use]
mod error;
mod memory;
mod timers;
mod cpus;
mod devices;

use crate::memory::{AddressSpace, MemoryBlock};
use crate::cpus::m68k::MC68010;
use crate::devices::mc68681::MC68681;
use crate::devices::ata::AtaDevice;

fn main() {
    let mut space = AddressSpace::new();
    let monitor = MemoryBlock::load("binaries/monitor.bin").unwrap();
    for byte in monitor.contents.iter() {
        print!("{:02x} ", byte);
    }
    space.insert(0x00000000, Box::new(monitor));

    let mut ram = MemoryBlock::new(vec![0; 0x00100000]);
    ram.load_at(0, "binaries/kernel.bin").unwrap();
    space.insert(0x00100000, Box::new(ram));

    let mut ata = AtaDevice::new();
    ata.load("binaries/compactflash.img").unwrap();
    space.insert(0x00600000, Box::new(ata));

    let mut serial = MC68681::new();
    serial.open().unwrap();
    space.insert(0x00700000, Box::new(serial));

    let mut cpu = MC68010::new();
    //cpu.enable_tracing();

    //cpu.add_breakpoint(0x0c94);
    //cpu.add_breakpoint(0x103234);
    //cpu.add_breakpoint(0x106e6a);

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

    /*
    // TODO I need to add a way to decode and dump the assembly for a section of code, in debugger
    cpu.state.pc = 0x00100000;
    cpu.state.pc = 0x0010c270;
    while cpu.is_running() {
        match cpu.decode_next(&mut space) {
            Ok(()) => { },
            Err(err) => {
                cpu.dump_state(&mut space);
                panic!("{:?}", err);
            },
        }
    }
    */
}

