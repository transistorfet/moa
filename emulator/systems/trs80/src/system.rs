use femtos::Frequency;

use moa_core::{System, Error, MemoryBlock, Device};
use moa_host::Host;

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

    system.add_interruptable_device("cpu", Device::new(cpu))?;

    Ok(system)
}
