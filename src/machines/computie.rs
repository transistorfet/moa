
use crate::error::Error;
use crate::system::System;
use crate::devices::wrap_transmutable;
use crate::memory::{MemoryBlock, BusPort};

use crate::cpus::m68k::{M68k, M68kType};
use crate::peripherals::ata::AtaDevice;
use crate::peripherals::mc68681::MC68681;

use crate::host::traits::Host;
use crate::host::tty::SimplePty;


pub fn build_computie<H: Host>(host: &H) -> Result<System, Error> {
    let mut system = System::new();

    let monitor = MemoryBlock::load("binaries/computie/monitor.bin")?;
    system.add_addressable_device(0x00000000, wrap_transmutable(monitor))?;

    let mut ram = MemoryBlock::new(vec![0; 0x00100000]);
    ram.load_at(0, "binaries/computie/kernel.bin")?;
    system.add_addressable_device(0x00100000, wrap_transmutable(ram))?;

    let mut ata = AtaDevice::new();
    ata.load("binaries/computie/disk-with-partition-table.img")?;
    system.add_addressable_device(0x00600000, wrap_transmutable(ata))?;

    let mut serial = MC68681::new();
    launch_terminal_emulator(serial.port_a.connect(Box::new(SimplePty::open()?))?);
    launch_slip_connection(serial.port_b.connect(Box::new(SimplePty::open()?))?);
    system.add_addressable_device(0x00700000, wrap_transmutable(serial))?;


    let mut cpu = M68k::new(M68kType::MC68010, 10_000_000, BusPort::new(0, 24, 16, system.bus.clone()));

    //cpu.enable_tracing();
    //cpu.add_breakpoint(0x10781a);
    //cpu.add_breakpoint(0x10bc9c);
    //cpu.add_breakpoint(0x106a94);
    //cpu.add_breakpoint(0x1015b2);
    //cpu.add_breakpoint(0x103332);
    //cpu.decoder.dump_disassembly(&mut system, 0x100000, 0x2000);
    //cpu.decoder.dump_disassembly(&mut system, 0x2ac, 0x200);

    system.add_interruptable_device(wrap_transmutable(cpu))?;

    Ok(system)
}

pub fn build_computie_k30<H: Host>(host: &H) -> Result<System, Error> {
    let mut system = System::new();

    let monitor = MemoryBlock::load("binaries/computie/monitor-68030.bin")?;
    system.add_addressable_device(0x00000000, wrap_transmutable(monitor))?;

    let mut ram = MemoryBlock::new(vec![0; 0x00100000]);
    ram.load_at(0, "binaries/computie/kernel-68030.bin")?;
    system.add_addressable_device(0x00100000, wrap_transmutable(ram))?;

    let mut ata = AtaDevice::new();
    ata.load("binaries/computie/disk-with-partition-table.img")?;
    system.add_addressable_device(0x00600000, wrap_transmutable(ata))?;

    let mut serial = MC68681::new();
    launch_terminal_emulator(serial.port_a.connect(Box::new(SimplePty::open()?))?);
    //launch_slip_connection(serial.port_b.connect(Box::new(SimplePty::open()?))?);
    system.add_addressable_device(0x00700000, wrap_transmutable(serial))?;


    let mut cpu = M68k::new(M68kType::MC68030, 10_000_000, BusPort::new(0, 32, 32, system.bus.clone()));

    //cpu.enable_tracing();
    //cpu.add_breakpoint(0x10781a);
    //cpu.add_breakpoint(0x10bc9c);
    //cpu.add_breakpoint(0x106a94);
    //cpu.add_breakpoint(0x1015b2);
    //cpu.add_breakpoint(0x103332);
    //cpu.decoder.dump_disassembly(&mut system, 0x100000, 0x2000);
    //cpu.decoder.dump_disassembly(&mut system, 0x2ac, 0x200);

    system.add_interruptable_device(wrap_transmutable(cpu))?;

    Ok(system)
}

pub fn launch_terminal_emulator(name: String) {
    use std::thread;
    use std::time::Duration;
    use std::process::Command;

    Command::new("x-terminal-emulator").arg("-e").arg(&format!("pyserial-miniterm {}", name)).spawn().unwrap();
    thread::sleep(Duration::from_secs(1));
}

pub fn launch_slip_connection(name: String) {
    use std::process::Command;

    Command::new("sudo").args(["slattach", "-s", "38400", "-p", "slip", &name]).spawn().unwrap();
    Command::new("sudo").args(["ifconfig", "sl0", "192.168.1.2", "pointopoint", "192.168.1.200", "up"]).status().unwrap();
    Command::new("sudo").args(["arp", "-Ds", "192.168.1.200", "enp4s0", "pub"]).status().unwrap();
    Command::new("sudo").args(["iptables", "-A", "FORWARD", "-i", "sl0", "-j", "ACCEPT"]).status().unwrap();
    Command::new("sudo").args(["iptables", "-A", "FORWARD", "-o", "sl0", "-j", "ACCEPT"]).status().unwrap();
    Command::new("sudo").args(["sh", "-c", "echo 1 > /proc/sys/net/ipv4/ip_forward"]).status().unwrap();
}

