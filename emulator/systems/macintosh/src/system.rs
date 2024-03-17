use femtos::Frequency;

use moa_core::{System, Error, MemoryBlock, Debuggable, Device};
use moa_host::Host;

use moa_m68k::{M68k, M68kType};

use crate::peripherals::video::MacVideo;
use crate::peripherals::mainboard::Mainboard;


pub fn build_macintosh_512k<H: Host>(host: &mut H) -> Result<System, Error> {
    let mut system = System::default();

    /*
    use crate::peripherals::mos6522::Mos6522;
    use crate::peripherals::z8530::Z8530;
    use crate::peripherals::macintosh::iwm::IWM;

    let mut ram = MemoryBlock::new(vec![0; 0x00100000]);
    ram.load_at(0, "binaries/macintosh/Macintosh 128k.rom")?;
    let boxed_ram = wrap_transmutable(ram);
    //system.add_addressable_device(0x00000000, boxed_ram.clone())?;
    //system.add_addressable_device(0x00600000, boxed_ram)?;

    let mut rom = MemoryBlock::load("binaries/macintosh/Macintosh 128k.rom")?;
    rom.read_only();
    let boxed_rom = wrap_transmutable(rom);
    //system.add_addressable_device(0x00400000, wrap_transmutable(rom))?;


    // The ROM accesses 0xf80000 to look for the debugger, and then accesses 0xf0000 which is the phase area
    let misc = MemoryBlock::new(vec![0; 0x100000]);
    system.add_addressable_device(0x00f00000, wrap_transmutable(misc))?;

    let video = MacVideo::new(host)?;
    system.add_device("video", wrap_transmutable(video)).unwrap();

    let scc1 = Z8530::new();
    //launch_terminal_emulator(serial.port_a.connect(Box::new(SimplePty::open()?))?);
    //launch_slip_connection(serial.port_b.connect(Box::new(SimplePty::open()?))?);
    system.add_addressable_device(0x009FFFF0, wrap_transmutable(scc1))?;

    let scc2 = Z8530::new();
    //launch_terminal_emulator(serial.port_a.connect(Box::new(SimplePty::open()?))?);
    //launch_slip_connection(serial.port_b.connect(Box::new(SimplePty::open()?))?);
    system.add_addressable_device(0x00BFFFF0, wrap_transmutable(scc2))?;

    let iwm = IWM::new();
    let adapter = AddressRightShifter::new(wrap_transmutable(iwm), 9);
    system.add_addressable_device(0x00DFE1FF, wrap_transmutable(adapter))?;

    //let via = wrap_transmutable(Mos6522::new());
    let mainboard = Mainboard::new(boxed_ram, boxed_rom);
    system.add_addressable_device(0x00000000, mainboard.bus.clone())?;
    let mainboard_boxed = wrap_transmutable(mainboard);
    system.add_device("via", mainboard_boxed.clone())?;
    let adapter = AddressRightShifter::new(mainboard_boxed, 9);
    system.add_addressable_device(0x00EFE000, wrap_transmutable(adapter))?;
    */

    let ram = MemoryBlock::new(vec![0; 0x00080000]);
    let mut rom = MemoryBlock::load("binaries/macintosh/Macintosh 512k.rom")?;
    rom.read_only();

    let video = MacVideo::new(host)?;
    system.add_device("video", Device::new(video)).unwrap();

    let mainboard = Mainboard::new(Device::new(ram), Device::new(rom))?;
    system.add_addressable_device(0x00000000, Device::new(mainboard))?;


    let mut cpu = M68k::from_type(M68kType::MC68000, Frequency::from_hz(7_833_600));

    //cpu.enable_tracing();
    //system.enable_debugging();
    //cpu.add_breakpoint(0x10781a);
    //cpu.add_breakpoint(0x40002a);
    //cpu.add_breakpoint(0x400694);       // Ram Test

    //cpu.add_breakpoint(0x400170);       // Failed, loops infinitely
    cpu.add_breakpoint(0x4000f4); // Failed, should show the sad mac
    //cpu.add_breakpoint(0x4006ae);
    //cpu.add_breakpoint(0x400706);
    //cpu.add_breakpoint(0x400722);       // end of ram test

    //cpu.add_breakpoint(0x40026c);       // System Initialization
    //cpu.add_breakpoint(0x402adc);
    //cpu.add_breakpoint(0x40078e);
    //cpu.add_breakpoint(0x40080a);

    //cpu.add_breakpoint(0x400448);
    //cpu.add_breakpoint(0x40040a);       // InitROMCore (set up trap dispatch table)
    //cpu.add_breakpoint(0x402acc);       // InitMem

    //cpu.add_breakpoint(0x40045c);
    //cpu.add_breakpoint(0x400614);     // Start of InitIO
    cpu.add_breakpoint(0x40062a); // Loop in InitIO
    //cpu.add_breakpoint(0x400648);
    //cpu.add_breakpoint(0x40064c);
    //cpu.add_breakpoint(0x4014a6);       // DrvrInstall
    //cpu.add_breakpoint(0x401262);       // $A000 handler, which is where the rom write happens
    //cpu.add_breakpoint(0x4012ec);
    //cpu.add_breakpoint(0x40133a);

    // Issue of writing to 0x100000 which doesn't exist
    cpu.add_breakpoint(0x400d62);

    cpu.add_breakpoint(0x400464); // Boot Screen

    /*
    use crate::devices::Addressable;
    use crate::cpus::m68k::state::M68kState;
    for i in 0..=65535 {
        cpu.state = M68kState::new();
        system.get_bus().write_beu16(0, i).unwrap();
        match cpu.decode_next(&system) {
            Ok(()) => { println!("TestCase {{ cpu: M68kType::MC68000, data: &[{:#06X}]\tins: Some({:?}) }},", i, cpu.decoder.instruction); },
            Err(_) => { println!("TestCase {{ cpu: M68kType::MC68000, data: &[{:#06X}]\tins: None }},", i); },
        }
    }
    panic!("");
    */

    system.add_interruptable_device("cpu", Device::new(cpu))?;

    Ok(system)
}
