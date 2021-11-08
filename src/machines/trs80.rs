
use crate::error::Error;
use crate::system::System;
use crate::devices::{Debuggable, wrap_transmutable};
use crate::memory::{MemoryBlock, BusPort};

use crate::cpus::z80::{Z80, Z80Type};
use crate::peripherals::trs80;

use crate::host::traits::Host;
use crate::host::tty::SimplePty;


pub fn build_trs80<H: Host>(host: &mut H) -> Result<System, Error> {
    let mut system = System::new();

    let mut rom = MemoryBlock::new(vec![0; 0x4000]);
    rom.load_at(0x0000, "binaries/trs80/level1.rom")?;
    rom.load_at(0x1000, "binaries/trs80/level2.rom")?;
    rom.read_only();
    system.add_addressable_device(0x0000, wrap_transmutable(rom))?;

    let mut ram = MemoryBlock::new(vec![0; 0xC000]);
    system.add_addressable_device(0x4000, wrap_transmutable(ram))?;

    let model1 = trs80::model1::Model1Peripherals::create(host)?;
    system.add_addressable_device(0x37E0, wrap_transmutable(model1)).unwrap();

    let mut cpu = Z80::new(Z80Type::Z80, 4_000_000, BusPort::new(0, 16, 8, system.bus.clone()));
    //cpu.add_breakpoint(0x0);
    //cpu.add_breakpoint(0xb55);
    //cpu.add_breakpoint(0xb76);
    //cpu.add_breakpoint(0x1e5);
    //cpu.add_breakpoint(0x340);        // "exec", the function that executes the line typed in
    //cpu.add_breakpoint(0x357);
    system.add_interruptable_device("cpu", wrap_transmutable(cpu))?;

    Ok(system)
}

