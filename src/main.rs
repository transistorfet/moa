
#[macro_use]
mod error;
mod memory;
mod timers;
mod devices;
mod interrupts;
mod cpus;
mod peripherals;
mod system;

use crate::system::System;
use crate::memory::MemoryBlock;
use crate::cpus::m68k::MC68010;
use crate::peripherals::ata::AtaDevice;
use crate::peripherals::mc68681::MC68681;
use crate::devices::{wrap_addressable, wrap_interruptable};

fn main() {
    let mut system = System::new();

    let monitor = MemoryBlock::load("binaries/monitor.bin").unwrap();
    for byte in monitor.contents.iter() {
        print!("{:02x} ", byte);
    }
    system.add_addressable_device(0x00000000, wrap_addressable(monitor)).unwrap();

    let mut ram = MemoryBlock::new(vec![0; 0x00100000]);
    ram.load_at(0, "binaries/kernel.bin").unwrap();
    system.add_addressable_device(0x00100000, wrap_addressable(ram)).unwrap();

    let mut ata = AtaDevice::new();
    ata.load("binaries/disk-with-partition-table.img").unwrap();
    system.add_addressable_device(0x00600000, wrap_addressable(ata)).unwrap();

    let mut serial = MC68681::new();
    launch_terminal_emulator(serial.port_a.open().unwrap());
    launch_slip_connection(serial.port_b.open().unwrap());
    system.add_addressable_device(0x00700000, wrap_addressable(serial)).unwrap();


    let mut cpu = MC68010::new();

    //cpu.enable_tracing();
    //cpu.add_breakpoint(0x10781a);
    //cpu.add_breakpoint(0x10bc9c);
    //cpu.add_breakpoint(0x106a94);
    //cpu.add_breakpoint(0x10d0c6);

    system.add_interruptable_device(wrap_interruptable(cpu)).unwrap();
    loop {
        match system.step() {
            Ok(()) => { },
            Err(err) => {
                system.exit_error();
                panic!("{:?}", err);
            },
        }
    }

    /*
    // TODO I need to add a way to decode and dump the assembly for a section of code, in debugger
    cpu.enable_tracing();
    cpu.state.pc = 0x0010781a;
    while cpu.is_running() {
        match cpu.decode_next(&system) {
            Ok(()) => { },
            Err(err) => {
                cpu.dump_state(&system);
                panic!("{:?}", err);
            },
        }
    }
    */
}

pub fn launch_terminal_emulator(name: String) {
    use nix::unistd::sleep;
    use std::process::Command;

    Command::new("x-terminal-emulator").arg("-e").arg(&format!("pyserial-miniterm {}", name)).spawn().unwrap();
    sleep(1);
}

pub fn launch_slip_connection(name: String) {
    use nix::unistd::sleep;
    use std::process::Command;

    //Command::new("x-terminal-emulator").arg("-e").arg(&format!("pyserial-miniterm {}", name)).spawn().unwrap();

    Command::new("sudo").args(["slattach", "-s", "38400", "-p", "slip", &name]).spawn().unwrap();
    Command::new("sudo").args(["ifconfig", "sl0", "192.168.1.2", "pointopoint", "192.168.1.200", "up"]).status().unwrap();
    Command::new("sudo").args(["arp", "-Ds", "192.168.1.200", "enp4s0", "pub"]).status().unwrap();
    Command::new("sudo").args(["iptables", "-A", "FORWARD", "-i", "sl0", "-j", "ACCEPT"]).status().unwrap();
    Command::new("sudo").args(["iptables", "-A", "FORWARD", "-o", "sl0", "-j", "ACCEPT"]).status().unwrap();
    Command::new("sudo").args(["sh", "-c", "echo 1 > /proc/sys/net/ipv4/ip_forward"]).status().unwrap();
    /*
    */
    /*
    sudo slattach -s 38400 -p slip /dev/ttyUSB1
    sudo ifconfig sl0 192.168.1.2 pointopoint 192.168.1.200 up
    // (this is automatically added on my machine) >> sudo route add -host 192.168.1.200 sl0
    sudo arp -Ds 192.168.1.200 enp3s0 pub
    sudo iptables -A FORWARD -i sl0 -j ACCEPT
    sudo iptables -A FORWARD -o sl0 -j ACCEPT
    sudo sh -c "echo 1 > /proc/sys/net/ipv4/ip_forward"
    */

    sleep(1);
}

