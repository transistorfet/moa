
use crate::error::Error;
use crate::system::System;
use crate::memory::{MemoryBlock, BusPort};
use crate::devices::{wrap_transmutable, Debuggable};

use crate::cpus::m68k::{M68k, M68kType};
use crate::peripherals::genesis;

use crate::host::traits::{Host, WindowUpdater};


pub fn build_genesis<H: Host>(host: &mut H) -> Result<System, Error> {
    let mut system = System::new();

    //let mut rom = MemoryBlock::load("binaries/genesis/Sonic The Hedgehog (W) (REV 00) [!].bin").unwrap();
    //let mut rom = MemoryBlock::load("binaries/genesis/Sonic The Hedgehog (W) (REV 01) [!].bin").unwrap();
    let mut rom = MemoryBlock::load("binaries/genesis/Sonic the Hedgehog 2 (JUE) [!].bin").unwrap();
    //let mut rom = MemoryBlock::load("binaries/genesis/Sonic the Hedgehog 3 (U) [!].bin").unwrap();
    //let mut rom = MemoryBlock::load("binaries/genesis/Earthworm Jim (U) [h1].bin").unwrap();
    //let mut rom = MemoryBlock::load("binaries/genesis/Home Alone (beta).bin").unwrap();
    //let mut rom = MemoryBlock::load("binaries/genesis/F1 World Championship (JUE) [!].bin").unwrap();
    //let mut rom = MemoryBlock::load("binaries/genesis/Ren and Stimpy's Invention (U) [!].bin").unwrap();
    //let mut rom = MemoryBlock::load("binaries/genesis/Out of this World (U) [!].bin").unwrap();
    //let mut rom = MemoryBlock::load("binaries/genesis/Ghostbusters (REV 00) (JUE).bin").unwrap();
    //let mut rom = MemoryBlock::load("binaries/genesis/Teenage Mutant Ninja Turtles - The Hyperstone Heist (U) [!].bin").unwrap();
    rom.read_only();
    system.add_addressable_device(0x00000000, wrap_transmutable(rom)).unwrap();

    let ram = MemoryBlock::new(vec![0; 0x00010000]);
    system.add_addressable_device(0x00FF0000, wrap_transmutable(ram)).unwrap();



    let coproc_shared_mem = MemoryBlock::new(vec![0; 0x00010000]);
    system.add_addressable_device(0x00A00000, wrap_transmutable(coproc_shared_mem)).unwrap();


    let controllers = genesis::controllers::GenesisController::create(host)?;
    let interrupt = controllers.get_interrupt_signal();
    system.add_addressable_device(0x00a10000, wrap_transmutable(controllers)).unwrap();

    let coproc = genesis::coproc_memory::CoprocessorMemory::new();
    system.add_addressable_device(0x00a11000, wrap_transmutable(coproc)).unwrap();

    let vdp = genesis::ym7101::Ym7101::new(host, interrupt);
    system.add_addressable_device(0x00c00000, wrap_transmutable(vdp)).unwrap();


    let mut cpu = M68k::new(M68kType::MC68000, 7_670_454, BusPort::new(0, 24, 16, system.bus.clone()));

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

    system.add_interruptable_device("cpu", wrap_transmutable(cpu)).unwrap();

    Ok(system)
}

