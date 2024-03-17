use std::thread;
use std::time::Duration;
use femtos::Frequency;

use moa_core::{System, MemoryBlock, Device};

use moa_m68k::{M68k, M68kType};
use moa_peripherals_generic::AtaDevice;
use moa_peripherals_motorola::MC68681;

fn main() {
    thread::spawn(|| {
        let mut system = System::default();

        let monitor = MemoryBlock::load("binaries/computie/monitor.bin").unwrap();
        system.add_addressable_device(0x00000000, Device::new(monitor)).unwrap();

        let mut ram = MemoryBlock::new(vec![0; 0x00100000]);
        ram.load_at(0, "binaries/computie/kernel.bin").unwrap();
        system.add_addressable_device(0x00100000, Device::new(ram)).unwrap();

        let mut ata = AtaDevice::default();
        ata.load("binaries/computie/disk-with-partition-table.img").unwrap();
        system.add_addressable_device(0x00600000, Device::new(ata)).unwrap();

        let serial = MC68681::default();
        system.add_addressable_device(0x00700000, Device::new(serial)).unwrap();


        let cpu = M68k::from_type(M68kType::MC68010, Frequency::from_mhz(8));

        //cpu.enable_tracing();
        //cpu.add_breakpoint(0x10781a);
        //cpu.add_breakpoint(0x10bc9c);
        //cpu.add_breakpoint(0x106a94);
        //cpu.add_breakpoint(0x1015b2);
        //cpu.add_breakpoint(0x103332);
        //cpu.decoder.dump_disassembly(&mut system, 0x100000, 0x2000);
        //cpu.decoder.dump_disassembly(&mut system, 0x2ac, 0x200);

        system.add_interruptable_device("cpu", Device::new(cpu)).unwrap();

        system.run_forever().unwrap();
    });
    thread::sleep(Duration::from_secs(10));
}
