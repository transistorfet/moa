
use std::rc::Rc;
use std::cell::RefCell;

use crate::error::Error;
use crate::system::System;
use crate::signals::{Signal};
use crate::memory::{MemoryBlock, Bus, BusPort};
use crate::devices::{wrap_transmutable, Address, Addressable, Debuggable};

use crate::cpus::m68k::{M68k, M68kType};
use crate::cpus::z80::{Z80, Z80Type};
use crate::peripherals::ym2612::Ym2612;
use crate::peripherals::sn76489::Sn76489;
use crate::peripherals::genesis;
use crate::peripherals::genesis::coprocessor::{CoprocessorBankRegister, CoprocessorBankArea};

use crate::host::traits::{Host};


pub struct SegaGenesisOptions {
    pub rom: String,
}

impl SegaGenesisOptions {
    pub fn new() -> Self {
        Self {
            rom: "".to_string(),
        }
    }
}

pub fn build_genesis<H: Host>(host: &mut H, options: SegaGenesisOptions) -> Result<System, Error> {
    let mut system = System::new();

    let rom = MemoryBlock::load(&options.rom).unwrap();
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

    let nvram = MemoryBlock::new(vec![0; 0x00010000]);
    system.add_addressable_device(rom_end as Address, wrap_transmutable(nvram)).unwrap();

    let ram = MemoryBlock::new(vec![0; 0x00010000]);
    system.add_addressable_device(0x00ff0000, wrap_transmutable(ram)).unwrap();


    // Build the Coprocessor's Bus
    let bank_register = Signal::new(0);
    let coproc_ram = wrap_transmutable(MemoryBlock::new(vec![0; 0x00002000]));
    let coproc_ym_sound = wrap_transmutable(Ym2612::create(host)?);
    let coproc_sn_sound = wrap_transmutable(Sn76489::create(host)?);
    let coproc_register = wrap_transmutable(CoprocessorBankRegister::new(bank_register.clone()));
    let coproc_area = wrap_transmutable(CoprocessorBankArea::new(bank_register, system.bus.clone()));

    let coproc_bus = Rc::new(RefCell::new(Bus::new()));
    coproc_bus.borrow_mut().set_ignore_unmapped(true);
    coproc_bus.borrow_mut().insert(0x0000, coproc_ram.clone());
    coproc_bus.borrow_mut().insert(0x4000, coproc_ym_sound.clone());
    coproc_bus.borrow_mut().insert(0x6000, coproc_register.clone());
    coproc_bus.borrow_mut().insert(0x7f11, coproc_sn_sound.clone());
    coproc_bus.borrow_mut().insert(0x8000, coproc_area);
    let mut coproc = Z80::new(Z80Type::Z80, 3_579_545, BusPort::new(0, 16, 8, coproc_bus.clone()));
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


    let controllers = genesis::controllers::GenesisControllers::create(host)?;
    let interrupt = controllers.get_interrupt_signal();
    system.add_addressable_device(0x00a10000, wrap_transmutable(controllers)).unwrap();

    let coproc = genesis::coprocessor::CoprocessorCoordinator::new(reset, bus_request);
    system.add_addressable_device(0x00a11000, wrap_transmutable(coproc)).unwrap();

    let vdp = genesis::ym7101::Ym7101::new(host, interrupt, coproc_sn_sound);
    system.break_signal = Some(vdp.frame_complete.clone());
    system.add_peripheral("vdp", 0x00c00000, wrap_transmutable(vdp)).unwrap();

    let cpu = M68k::new(M68kType::MC68000, 7_670_454, BusPort::new(0, 24, 16, system.bus.clone()));
    system.add_interruptable_device("cpu", wrap_transmutable(cpu)).unwrap();

    Ok(system)
}

