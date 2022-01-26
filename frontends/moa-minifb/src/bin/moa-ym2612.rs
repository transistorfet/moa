
use std::sync::mpsc;

use moa_minifb;
use moa::peripherals::ym2612::{Ym2612};
use moa::peripherals::sn76489::{Sn76489};

use moa::error::Error;
use moa::system::System;
use moa::host::gfx::Frame;
use moa::devices::{ClockElapsed, Address, Addressable, Steppable, Transmutable, TransmutableBox, wrap_transmutable};
use moa::host::keys::{Key};
use moa::host::traits::{Host, HostData, KeyboardUpdater};


pub struct SynthControlsUpdater(mpsc::Sender<(Key, bool)>);

impl KeyboardUpdater for SynthControlsUpdater {
    fn update_keyboard(&mut self, key: Key, state: bool) {
        self.0.send((key, state)).unwrap();
    }
}

struct SynthControl {
    receiver: mpsc::Receiver<(Key, bool)>,
}

impl SynthControl {
    pub fn new(receiver: mpsc::Receiver<(Key, bool)>) -> Self {
        Self {
            receiver,
        }
    }
}

impl Steppable for SynthControl {
    fn step(&mut self, system: &System) -> Result<ClockElapsed, Error> {
        if let Ok((key, state)) = self.receiver.try_recv() {

            match key {
                Key::Enter => {
                    system.get_bus().write_u8(0x00, 0x28)?;
                    system.get_bus().write_u8(0x01, if state { 0xF0 } else { 0x00 })?;
                },

                Key::A => {
                    system.get_bus().write_u8(0x10, 0x84)?;
                    system.get_bus().write_u8(0x10, 0x0F)?;
                    system.get_bus().write_u8(0x10, if state { 0x90 } else { 0x9F })?;
                },
                _ => { },
            }
        }

        Ok(1_000_000)
    }
}

impl Transmutable for SynthControl {
    fn as_steppable(&mut self) -> Option<&mut dyn Steppable> {
        Some(self)
    }
}

fn set_register(device: &mut dyn Addressable, bank: u8, reg: u8, data: u8) -> Result<(), Error> {
    let addr = (bank as Address) * 2;
    device.write_u8(addr, reg)?;
    device.write_u8(addr + 1, data)?;
    Ok(())
}

fn initialize_ym(ym_sound: TransmutableBox) -> Result<(), Error> {
    let mut borrow = ym_sound.borrow_mut();
    let device = borrow.as_addressable().unwrap();

    set_register(device, 0, 0x30, 0x71)?;
    set_register(device, 0, 0x34, 0x0D)?;
    set_register(device, 0, 0x38, 0x33)?;
    set_register(device, 0, 0x3C, 0x01)?;

    set_register(device, 0, 0xA4, 0x22)?;
    set_register(device, 0, 0xA0, 0x69)?;
    set_register(device, 0, 0xB0, 0x30)?;
    Ok(())
}

fn main() {
    let matches = moa_minifb::new("YM2612 Tester/Synth")
        .get_matches();

    moa_minifb::run(matches, |host| {
        let mut system = System::new();

        let (sender, receiver) = mpsc::channel();
        let control = wrap_transmutable(SynthControl::new(receiver));
        system.add_device("control", control)?;

        let ym_sound = wrap_transmutable(Ym2612::create(host)?);
        initialize_ym(ym_sound.clone())?;
        system.add_addressable_device(0x00, ym_sound)?;

        let sn_sound = wrap_transmutable(Sn76489::create(host)?);
        system.add_addressable_device(0x10, sn_sound)?;

        let frame = Frame::new_shared(384, 128);
        host.add_window(Frame::new_updater(frame.clone()))?;
        host.register_keyboard(Box::new(SynthControlsUpdater(sender)))?;

        Ok(system)
    });
}


