
use std::mem;
use std::rc::Rc;
use std::cell::RefCell;

use moa_core::{System, Error, Frequency, Signal, MemoryBlock, Bus, BusPort, Address, Addressable, Debuggable, wrap_transmutable};
use moa_core::host::Host;

use moa_m68k::{M68k, M68kType};
use moa_z80::{Z80, Z80Type};
use moa_peripherals_yamaha::Ym2612;
use moa_peripherals_yamaha::Sn76489;

use crate::utils;
use crate::peripherals::ym7101::Ym7101;
use crate::peripherals::controllers::GenesisControllers;
use crate::peripherals::coprocessor::{CoprocessorCoordinator, CoprocessorBankRegister, CoprocessorBankArea};


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
    //let mut rom = MemoryBlock::load("binaries/genesis/GenTestV3.0.bin").unwrap();
    //let mut rom = MemoryBlock::load("binaries/genesis/HDRV_Genesis_Test_v1_4.bin").unwrap();
    //let mut rom = MemoryBlock::load("binaries/genesis/ComradeOj's tiny demo.bin").unwrap();
    //let mut rom = MemoryBlock::load("binaries/genesis/Digital Rain demo.bin").unwrap();
    //let mut rom = MemoryBlock::load("binaries/genesis/Sonic The Hedgehog (W) (REV 00) [!].bin").unwrap();
    //let mut rom = MemoryBlock::load("binaries/genesis/Sonic The Hedgehog (W) (REV 01) [!].bin").unwrap();
    //let mut rom = MemoryBlock::load("binaries/genesis/Sonic the Hedgehog 2 (JUE) [!].bin").unwrap();
    //let mut rom = MemoryBlock::load("binaries/genesis/Sonic the Hedgehog 3 (U) [!].bin").unwrap();
    //let mut rom = MemoryBlock::load("binaries/genesis/Earthworm Jim (U) [h1].bin").unwrap();
    //let mut rom = MemoryBlock::load("binaries/genesis/Home Alone (beta).bin").unwrap();
    //let mut rom = MemoryBlock::load("binaries/genesis/F1 World Championship (JUE) [!].bin").unwrap();
    //let mut rom = MemoryBlock::load("binaries/genesis/Ren and Stimpy's Invention (U) [!].bin").unwrap();
    //let mut rom = MemoryBlock::load("binaries/genesis/Out of this World (U) [!].bin").unwrap();
    //let mut rom = MemoryBlock::load("binaries/genesis/Ghostbusters (REV 00) (JUE).bin").unwrap();
    //let mut rom = MemoryBlock::load("binaries/genesis/Teenage Mutant Ninja Turtles - The Hyperstone Heist (U) [!].bin").unwrap();
    //rom.read_only();
    let rom_end = rom.len();
    system.add_addressable_device(0x00000000, wrap_transmutable(rom)).unwrap();

    let cartridge_nvram = MemoryBlock::new(vec![0; 0x400000 - rom_end]);
    system.add_addressable_device(rom_end as Address, wrap_transmutable(cartridge_nvram)).unwrap();

    let ram = MemoryBlock::new(vec![0; 0x00010000]);
    system.add_addressable_device(0x00ff0000, wrap_transmutable(ram)).unwrap();


    // Build the Coprocessor's Bus
    let bank_register = Signal::new(0);
    let coproc_ram = wrap_transmutable(MemoryBlock::new(vec![0; 0x00002000]));
    let coproc_ym_sound = wrap_transmutable(Ym2612::new(host, Frequency::from_hz(7_670_454))?);
    let coproc_sn_sound = wrap_transmutable(Sn76489::new(host, Frequency::from_hz(3_579_545))?);
    let coproc_register = wrap_transmutable(CoprocessorBankRegister::new(bank_register.clone()));
    let coproc_area = wrap_transmutable(CoprocessorBankArea::new(bank_register, system.bus.clone()));

    let coproc_bus = Rc::new(RefCell::new(Bus::default()));
    coproc_bus.borrow_mut().set_ignore_unmapped(true);
    coproc_bus.borrow_mut().insert(0x0000, coproc_ram.clone());
    coproc_bus.borrow_mut().insert(0x4000, coproc_ym_sound.clone());
    coproc_bus.borrow_mut().insert(0x6000, coproc_register.clone());
    coproc_bus.borrow_mut().insert(0x7f11, coproc_sn_sound.clone());
    coproc_bus.borrow_mut().insert(0x8000, coproc_area);
    let mut coproc = Z80::new(Z80Type::Z80, Frequency::from_hz(3_579_545), BusPort::new(0, 16, 8, coproc_bus));
    coproc.set_debugging(true);
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
    system.add_device("coproc", wrap_transmutable(coproc))?;


    let controllers = GenesisControllers::new(host)?;
    let interrupt = controllers.get_interrupt_signal();
    system.add_addressable_device(0x00a10000, wrap_transmutable(controllers)).unwrap();

    let coproc = CoprocessorCoordinator::new(reset, bus_request);
    system.add_addressable_device(0x00a11000, wrap_transmutable(coproc)).unwrap();

    let vdp = Ym7101::new(host, interrupt, coproc_sn_sound);
    system.break_signal = Some(vdp.frame_complete.clone());
    system.add_peripheral("vdp", 0x00c00000, wrap_transmutable(vdp)).unwrap();

    let cpu = M68k::new(M68kType::MC68000, Frequency::from_hz(7_670_454), BusPort::new(0, 24, 16, system.bus.clone()));
    system.add_interruptable_device("cpu", wrap_transmutable(cpu)).unwrap();

    Ok(system)
}

