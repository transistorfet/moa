
use std::thread;
use std::time::Duration;

use moa::error::Error;
use moa::system::System;
use moa::devices::wrap_transmutable;
use moa::memory::{MemoryBlock, BusPort};

use moa::cpus::m68k::{M68k, M68kType};
use moa::peripherals::ata::AtaDevice;
use moa::peripherals::mc68681::MC68681;

use moa::machines::computie::build_computie;

fn main() {
    thread::spawn(|| {
        let mut system = System::new();

        let monitor = MemoryBlock::load("binaries/monitor.bin").unwrap();
        system.add_addressable_device(0x00000000, wrap_transmutable(monitor)).unwrap();

        let mut ram = MemoryBlock::new(vec![0; 0x00100000]);
        ram.load_at(0, "binaries/kernel.bin").unwrap();
        system.add_addressable_device(0x00100000, wrap_transmutable(ram)).unwrap();

        let mut ata = AtaDevice::new();
        ata.load("binaries/disk-with-partition-table.img").unwrap();
        system.add_addressable_device(0x00600000, wrap_transmutable(ata)).unwrap();

        let mut serial = MC68681::new();
        system.add_addressable_device(0x00700000, wrap_transmutable(serial)).unwrap();


        let mut cpu = M68k::new(M68kType::MC68010, 8_000_000, BusPort::new(0, 24, 16, system.bus.clone()));

        //cpu.enable_tracing();
        //cpu.add_breakpoint(0x10781a);
        //cpu.add_breakpoint(0x10bc9c);
        //cpu.add_breakpoint(0x106a94);
        //cpu.add_breakpoint(0x1015b2);
        //cpu.add_breakpoint(0x103332);
        //cpu.decoder.dump_disassembly(&mut system, 0x100000, 0x2000);
        //cpu.decoder.dump_disassembly(&mut system, 0x2ac, 0x200);

        system.add_interruptable_device(wrap_transmutable(cpu)).unwrap();

        system.run_loop();
    });
    thread::sleep(Duration::from_secs(10));
}

