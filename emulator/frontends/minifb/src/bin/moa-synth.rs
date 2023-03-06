
use std::sync::mpsc;

use moa_peripherals_yamaha::{Ym2612, Sn76489};

use moa_core::host::gfx::{Frame, FrameQueue};
use moa_core::host::{Host, WindowUpdater, KeyboardUpdater, Key, KeyEvent /*, MouseUpdater, MouseState, MouseEvent*/};
use moa_core::{System, Error, ClockElapsed, Address, Addressable, Steppable, Transmutable, TransmutableBox, wrap_transmutable};


pub struct SynthControlsUpdater(mpsc::Sender<KeyEvent>);

impl KeyboardUpdater for SynthControlsUpdater {
    fn update_keyboard(&mut self, event: KeyEvent) {
        self.0.send(event).unwrap();
    }
}

//impl MouseUpdater for SynthControlsUpdater {
//    fn update_mouse(&mut self, event: MouseEvent) {
//        self.0.send(event).unwrap();
//    }
//}

struct SynthControl {
    queue: FrameQueue,
    receiver: mpsc::Receiver<KeyEvent>,
}

impl SynthControl {
    pub fn new(queue: FrameQueue, receiver: mpsc::Receiver<KeyEvent>) -> Self {
        Self {
            queue,
            receiver,
        }
    }
}

impl Steppable for SynthControl {
    fn step(&mut self, system: &System) -> Result<ClockElapsed, Error> {
        if let Ok(event) = self.receiver.try_recv() {

            match event.key {
                Key::Enter => {
                    system.get_bus().write_u8(0x00, 0x28)?;
                    system.get_bus().write_u8(0x01, if event.state { 0xF0 } else { 0x00 })?;
                },

                Key::A => {
                    system.get_bus().write_u8(0x10, 0x84)?;
                    system.get_bus().write_u8(0x10, 0x0F)?;
                    system.get_bus().write_u8(0x10, if event.state { 0x90 } else { 0x9F })?;
                },

                _ => { },
            }
        }

        let size = self.queue.max_size();
        let frame = Frame::new(size.0, size.1);
        self.queue.add(system.clock, frame);

        Ok(33_000_000)
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
    set_register(device, 0, 0x3C, 0x00)?;

    set_register(device, 0, 0xA4, 0x22)?;
    set_register(device, 0, 0xA0, 0x69)?;
    set_register(device, 0, 0xB0, 0x30)?;
    Ok(())
}

fn main() {
    let matches = moa_minifb::new("YM2612 Tester/Synth")
        .get_matches();

    moa_minifb::run(matches, |host| {
        let mut system = System::default();

        let queue = FrameQueue::new(384, 128);
        let (sender, receiver) = mpsc::channel();
        let control = wrap_transmutable(SynthControl::new(queue.clone(), receiver));
        system.add_device("control", control)?;

        let ym_sound = wrap_transmutable(Ym2612::create(host)?);
        initialize_ym(ym_sound.clone())?;
        system.add_addressable_device(0x00, ym_sound)?;

        let sn_sound = wrap_transmutable(Sn76489::create(host)?);
        system.add_addressable_device(0x10, sn_sound)?;

        host.add_window(Box::new(queue))?;
        host.register_keyboard(Box::new(SynthControlsUpdater(sender)))?;
        //host.register_mouse(Box::new(SynthControlsUpdater(sender)))?;

        Ok(system)
    });
}


