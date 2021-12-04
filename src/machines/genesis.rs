
use std::rc::Rc;
use std::cell::RefCell;

use crate::error::Error;
use crate::system::System;
use crate::memory::{MemoryBlock, Bus, BusPort};
use crate::devices::{wrap_transmutable, Address, Addressable, Debuggable};

use crate::cpus::m68k::{M68k, M68kType};
use crate::cpus::z80::{Z80, Z80Type};
use crate::peripherals::genesis;

use crate::host::traits::{Host};


pub fn build_genesis<H: Host>(host: &mut H) -> Result<System, Error> {
    let mut system = System::new();

    let mut rom = MemoryBlock::load("binaries/genesis/GenTestV3.0.bin").unwrap();
    //let mut rom = MemoryBlock::load("binaries/genesis/HDRV_Genesis_Test_v1_4.bin").unwrap();
    //let mut rom = MemoryBlock::load("binaries/genesis/ComradeOj's tiny demo.bin").unwrap();
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


    let coproc_bus = Rc::new(RefCell::new(Bus::new()));
    let coproc_mem = wrap_transmutable(MemoryBlock::new(vec![0; 0x00010000]));
    coproc_bus.borrow_mut().insert(0x0000, coproc_mem.borrow_mut().as_addressable().unwrap().len(), coproc_mem.clone());
    let mut coproc = Z80::new(Z80Type::Z80, 3_579_545, BusPort::new(0, 16, 8, coproc_bus.clone()));
    let reset = coproc.reset.clone();
    let bus_request = coproc.bus_request.clone();

    system.add_addressable_device(0x00a00000, coproc_mem)?;
    //system.add_device("coproc", wrap_transmutable(coproc))?;



    let controllers = genesis::controllers::GenesisController::create(host)?;
    let interrupt = controllers.get_interrupt_signal();
    system.add_addressable_device(0x00a10000, wrap_transmutable(controllers)).unwrap();

    let coproc = genesis::coprocessor::CoprocessorCoordinator::new(reset, bus_request);
    system.add_addressable_device(0x00a11000, wrap_transmutable(coproc)).unwrap();

    let vdp = genesis::ym7101::Ym7101::new(host, interrupt);
    system.add_peripheral("vdp", 0x00c00000, wrap_transmutable(vdp)).unwrap();


    // TODO this is temporarily a 68010 because GenTest tests the CPU type by relying on the illegal instruction
    // exception which is bypassed to Error and I don't want to un-bypass yet while testing
    let mut cpu = M68k::new(M68kType::MC68010, 7_670_454, BusPort::new(0, 24, 16, system.bus.clone()));

    //cpu.enable_tracing();
    //cpu.add_breakpoint(0x206);
    //cpu.add_breakpoint(0x1dd0);         // Sonic: some kind of palette fading function
    //cpu.add_breakpoint(0x16ee);
    //cpu.decoder.dump_disassembly(&mut system, 0x206, 0x2000);

    //cpu.add_breakpoint(0x16a0e);
    //cpu.add_breakpoint(0x16812);
    //cpu.add_breakpoint(0x166ec);
    //cpu.add_breakpoint(0x13e18);
    //cpu.add_breakpoint(0x16570);
    //cpu.add_breakpoint(0x1714);

    //cpu.add_breakpoint(0x43c2);

    //cpu.add_breakpoint(0x21a);

    system.add_interruptable_device("cpu", wrap_transmutable(cpu)).unwrap();

    Ok(system)
}

