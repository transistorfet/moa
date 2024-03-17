use femtos::Frequency;

use moa_core::{System, Error, Debuggable, MemoryBlock, Device};
use moa_host::Host;

use moa_m68k::{M68k, M68kType};
use moa_peripherals_generic::AtaDevice;
use moa_peripherals_motorola::MC68681;

pub struct ComputieOptions {
    pub rom: String,
    pub ram: usize,
    pub frequency: Frequency,
}

impl Default for ComputieOptions {
    fn default() -> Self {
        Self {
            rom: "binaries/computie/monitor.bin".to_string(),
            ram: 0x10_0000,
            frequency: Frequency::from_hz(10_000_000),
        }
    }
}

pub fn build_computie<H: Host>(host: &H, options: ComputieOptions) -> Result<System, Error> {
    let mut system = System::default();

    let mut rom = MemoryBlock::new(vec![0; 0x10000]);
    rom.load_at(0x0000, &options.rom)?;
    system.add_addressable_device(0x00000000, Device::new(rom))?;

    let mut ram = MemoryBlock::new(vec![0; options.ram]);
    ram.load_at(0, "binaries/computie/kernel.bin")?;
    system.add_addressable_device(0x00100000, Device::new(ram))?;

    let mut ata = AtaDevice::default();
    ata.load("binaries/computie/disk-with-partition-table.img")?;
    system.add_addressable_device(0x00600000, Device::new(ata))?;

    let mut serial = MC68681::default();
    launch_terminal_emulator(serial.port_a.connect(host.add_pty()?)?);
    launch_slip_connection(serial.port_b.connect(host.add_pty()?)?);
    system.add_addressable_device(0x00700000, Device::new(serial))?;


    let mut cpu = M68k::from_type(M68kType::MC68010, options.frequency);

    cpu.add_breakpoint(0);

    system.add_interruptable_device("cpu", Device::new(cpu))?;

    Ok(system)
}

pub fn build_computie_k30<H: Host>(host: &H) -> Result<System, Error> {
    let mut system = System::default();

    let monitor = MemoryBlock::load("binaries/computie/monitor-68030.bin")?;
    system.add_addressable_device(0x00000000, Device::new(monitor))?;

    let mut ram = MemoryBlock::new(vec![0; 0x00100000]);
    ram.load_at(0, "binaries/computie/kernel-68030.bin")?;
    system.add_addressable_device(0x00100000, Device::new(ram))?;

    let mut ata = AtaDevice::default();
    ata.load("binaries/computie/disk-with-partition-table.img")?;
    system.add_addressable_device(0x00600000, Device::new(ata))?;

    let mut serial = MC68681::default();
    launch_terminal_emulator(serial.port_a.connect(host.add_pty()?)?);
    //launch_slip_connection(serial.port_b.connect(host.add_pty()?)?);
    system.add_addressable_device(0x00700000, Device::new(serial))?;


    let cpu = M68k::from_type(M68kType::MC68030, Frequency::from_hz(10_000_000));

    system.add_interruptable_device("cpu", Device::new(cpu))?;

    Ok(system)
}

pub fn launch_terminal_emulator(name: String) {
    use std::thread;
    use std::time::Duration;
    use std::process::Command;

    Command::new("x-terminal-emulator")
        .arg("-e")
        .arg(&format!("pyserial-miniterm {}", name))
        .spawn()
        .unwrap();
    thread::sleep(Duration::from_secs(1));
}

pub fn launch_slip_connection(name: String) {
    use std::process::Command;

    Command::new("sudo")
        .args(["slattach", "-s", "38400", "-p", "slip", &name])
        .spawn()
        .unwrap();
    Command::new("sudo")
        .args(["ifconfig", "sl0", "192.168.1.2", "pointopoint", "192.168.1.200", "up"])
        .status()
        .unwrap();
    Command::new("sudo")
        .args(["arp", "-Ds", "192.168.1.200", "enp4s0", "pub"])
        .status()
        .unwrap();
    Command::new("sudo")
        .args(["iptables", "-A", "FORWARD", "-i", "sl0", "-j", "ACCEPT"])
        .status()
        .unwrap();
    Command::new("sudo")
        .args(["iptables", "-A", "FORWARD", "-o", "sl0", "-j", "ACCEPT"])
        .status()
        .unwrap();
    Command::new("sudo")
        .args(["sh", "-c", "echo 1 > /proc/sys/net/ipv4/ip_forward"])
        .status()
        .unwrap();
}
