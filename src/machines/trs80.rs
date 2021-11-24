
use crate::error::Error;
use crate::system::System;
use crate::devices::{Debuggable, wrap_transmutable};
use crate::memory::{MemoryBlock, BusPort};

use crate::cpus::z80::{Z80, Z80Type};
use crate::peripherals::trs80;

use crate::host::traits::Host;

pub struct Trs80Options {
    pub rom: String,
    pub memory: u16,
    pub frequency: u32,
}

impl Trs80Options {
    pub fn new() -> Self {
        Self {
            rom: "binaries/trs80/level2.rom".to_string(),
            memory: 0xC000,
            frequency: 1_774_000,
        }
    }
}


pub fn build_trs80<H: Host>(host: &mut H, options: Trs80Options) -> Result<System, Error> {
    let mut system = System::new();

    let mut rom = MemoryBlock::new(vec![0; 0x3000]);
    //rom.load_at(0x0000, "binaries/trs80/level1.rom")?;
    //rom.load_at(0x0000, "binaries/trs80/level2.rom")?;
    rom.load_at(0x0000, &options.rom)?;
    rom.read_only();
    system.add_addressable_device(0x0000, wrap_transmutable(rom))?;

    let ram = MemoryBlock::new(vec![0; options.memory as usize]);
    system.add_addressable_device(0x4000, wrap_transmutable(ram))?;

    let model1 = trs80::model1::Model1Peripherals::create(host)?;
    system.add_addressable_device(0x37E0, wrap_transmutable(model1)).unwrap();

    let mut cpu = Z80::new(Z80Type::Z80, options.frequency, BusPort::new(0, 16, 8, system.bus.clone()));
    //cpu.add_breakpoint(0x0);
    //cpu.add_breakpoint(0xb55);
    //cpu.add_breakpoint(0xb76);
    //cpu.add_breakpoint(0x1e5);
    //cpu.add_breakpoint(0x340);        // "exec", the function that executes the line typed in
    //cpu.add_breakpoint(0x357);
    //cpu.add_breakpoint(0x401);        // LIST command exec
    //cpu.add_breakpoint(0x10);         // putchar
    //cpu.add_breakpoint(0x970);
    //cpu.add_breakpoint(0x9f9);
    //cpu.add_breakpoint(0xa58);          // return from printing the line number
    //cpu.add_breakpoint(0xc59);          // the function called first thing when printing a decimal number
    //cpu.add_breakpoint(0xe00);          // normalize the float?? 
    //cpu.add_breakpoint(0x970);          // just after the decimal number print function is called, but after the call at the start is complete
    //cpu.add_breakpoint(0xa6c); 

    //cpu.add_breakpoint(0xe00); 
    //cpu.add_breakpoint(0xc77); 
    //cpu.add_breakpoint(0xc83);
    //cpu.add_breakpoint(0x96d);
    //cpu.add_breakpoint(0x970);
    //cpu.add_breakpoint(0x9e2);
    //cpu.add_breakpoint(0x9f9);

    system.add_interruptable_device("cpu", wrap_transmutable(cpu))?;

    Ok(system)
}

