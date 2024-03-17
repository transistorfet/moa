use std::mem;
use std::rc::Rc;
use std::cell::RefCell;

use femtos::Frequency;

use moa_core::{System, Error, MemoryBlock, Bus, Address, Addressable, Device};
use moa_host::Host;

use moa_m68k::{M68k, M68kType};
use moa_z80::{Z80, Z80Type};
use moa_peripherals_yamaha::Ym2612;
use moa_peripherals_yamaha::Sn76489;

use crate::utils;
use crate::peripherals::ym7101::Ym7101;
use crate::peripherals::controllers::GenesisControllers;
use crate::peripherals::coprocessor::{CoprocessorCoordinator, CoprocessorBankArea};


pub struct SegaGenesisOptions {
    pub rom: String,
    pub rom_data: Option<Vec<u8>>,
}

impl Default for SegaGenesisOptions {
    fn default() -> Self {
        Self {
            rom: "".to_string(),
            rom_data: None,
        }
    }
}

pub fn build_genesis<H: Host>(host: &mut H, mut options: SegaGenesisOptions) -> Result<System, Error> {
    let mut system = System::default();

    let rom_data = if options.rom_data.is_some() {
        mem::take(&mut options.rom_data).unwrap()
    } else {
        utils::load_rom_file(&options.rom)?
    };

    let rom = MemoryBlock::new(rom_data);
    //rom.read_only();
    let rom_end = rom.size();
    system.add_addressable_device(0x00000000, Device::new(rom))?;

    let cartridge_nvram = MemoryBlock::new(vec![0; 0x400000 - rom_end]);
    system.add_addressable_device(rom_end as Address, Device::new(cartridge_nvram))?;

    let ram = MemoryBlock::new(vec![0; 0x00010000]);
    system.add_addressable_device(0x00ff0000, Device::new(ram))?;


    // Build the Coprocessor's Bus
    let coproc_ram = Device::new(MemoryBlock::new(vec![0; 0x00002000]));
    let coproc_ym_sound = Device::new(Ym2612::new(host, Frequency::from_hz(7_670_454))?);
    let coproc_sn_sound = Device::new(Sn76489::new(host, Frequency::from_hz(3_579_545))?);
    let (coproc_area, coproc_register) = CoprocessorBankArea::new(system.bus.clone());
    let coproc_area = Device::new(coproc_area);
    let coproc_register = Device::new(coproc_register);

    let coproc_bus = Rc::new(RefCell::new(Bus::default()));
    coproc_bus.borrow_mut().set_ignore_unmapped(true);
    coproc_bus.borrow_mut().insert(0x0000, coproc_ram.clone());
    coproc_bus.borrow_mut().insert(0x4000, coproc_ym_sound.clone());
    coproc_bus.borrow_mut().insert(0x6000, coproc_register.clone());
    coproc_bus.borrow_mut().insert(0x7f11, coproc_sn_sound.clone());
    coproc_bus.borrow_mut().insert(0x8000, coproc_area);
    let coproc = Z80::from_type(Z80Type::Z80, Frequency::from_hz(3_579_545), coproc_bus, 0, None);
    let mut reset = coproc.reset.clone();
    let mut bus_request = coproc.bus_request.clone();
    reset.set(true);
    bus_request.set(true);

    // Add coprocessor devices to the system bus so the 68000 can access them too
    system.add_addressable_device(0x00a00000, coproc_ram)?;
    system.add_addressable_device(0x00a04000, coproc_ym_sound)?;
    system.add_addressable_device(0x00a06000, coproc_register)?;
    //system.add_addressable_device(0x00c00010, coproc_sn_sound)?;
    system.add_device("sn_sound", coproc_sn_sound.clone())?;
    system.add_device("coproc", Device::new(coproc))?;


    let controllers = GenesisControllers::new(host)?;
    let interrupt = controllers.get_interrupt_signal();
    system.add_addressable_device(0x00a10000, Device::new(controllers))?;

    let coproc = CoprocessorCoordinator::new(reset, bus_request);
    system.add_addressable_device(0x00a11000, Device::new(coproc))?;

    let vdp = Ym7101::new(host, interrupt, coproc_sn_sound)?;
    system.add_peripheral("vdp", 0x00c00000, Device::new(vdp))?;

    let cpu = M68k::from_type(M68kType::MC68000, Frequency::from_hz(7_670_454));
    system.add_interruptable_device("cpu", Device::new(cpu))?;

    Ok(system)
}
