
use femtos::Frequency;

use moa_core::{System, Error, MemoryBlock, Device};
use moa_core::host::Host;

use moa_z80::{Z80, Z80Type};

use crate::peripherals::model1::{Model1Keyboard, Model1Video};


pub struct Trs80Options {
    pub rom: String,
    pub memory: u16,
    pub frequency: Frequency,
}

impl Default for Trs80Options {
    fn default() -> Self {
        Self {
            rom: "binaries/trs80/level2.rom".to_string(),
            memory: 0xC000,
            frequency: Frequency::from_hz(1_774_000),
        }
    }
}


pub fn build_trs80<H: Host>(host: &mut H, options: Trs80Options) -> Result<System, Error> {
    let mut system = System::default();

    let mut rom = MemoryBlock::new(vec![0; 0x3000]);
    //rom.load_at(0x0000, "binaries/trs80/level1.rom")?;
    //rom.load_at(0x0000, "binaries/trs80/level2.rom")?;
    rom.load_at(0x0000, &options.rom)?;
    rom.read_only();
    system.add_addressable_device(0x0000, Device::new(rom))?;

    let ram = MemoryBlock::new(vec![0; options.memory as usize]);
    system.add_addressable_device(0x4000, Device::new(ram))?;

    let keyboard = Model1Keyboard::new(host)?;
    system.add_addressable_device(0x37E0, Device::new(keyboard)).unwrap();
    let video = Model1Video::new(host)?;
    system.add_addressable_device(0x37E0 + 0x420, Device::new(video)).unwrap();

    // TODO the ioport needs to be hooked up
    let cpu = Z80::from_type(Z80Type::Z80, options.frequency, system.bus.clone(), 0, None);
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

    system.add_interruptable_device("cpu", Device::new(cpu))?;

    Ok(system)
}

