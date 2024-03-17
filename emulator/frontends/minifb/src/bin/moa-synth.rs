use femtos::{Instant, Duration, Frequency};

use moa_peripherals_yamaha::{Ym2612, Sn76489};

use moa_host::{self, Host, Frame, FrameSender, PixelEncoding, Key, KeyEvent, EventReceiver};
use moa_core::{System, Error, Address, Addressable, Steppable, Transmutable, Device};

const SCREEN_WIDTH: u32 = 384;
const SCREEN_HEIGHT: u32 = 128;

struct SynthControl {
    key_receiver: EventReceiver<KeyEvent>,
    frame_sender: FrameSender,
}

impl SynthControl {
    pub fn new(key_receiver: EventReceiver<KeyEvent>, frame_sender: FrameSender) -> Self {
        Self {
            key_receiver,
            frame_sender,
        }
    }
}

impl Steppable for SynthControl {
    fn step(&mut self, system: &System) -> Result<Duration, Error> {
        if let Some(event) = self.key_receiver.receive() {
            match event.key {
                Key::Enter => {
                    system.get_bus().write_u8(system.clock, 0x00, 0x28)?;
                    system
                        .get_bus()
                        .write_u8(system.clock, 0x01, if event.state { 0xF0 } else { 0x00 })?;
                },

                Key::A => {
                    system.get_bus().write_u8(system.clock, 0x10, 0x84)?;
                    system.get_bus().write_u8(system.clock, 0x10, 0x0F)?;
                    system
                        .get_bus()
                        .write_u8(system.clock, 0x10, if event.state { 0x90 } else { 0x9F })?;
                },

                _ => {},
            }
        }

        let frame = Frame::new(SCREEN_WIDTH, SCREEN_HEIGHT, PixelEncoding::RGBA);
        self.frame_sender.add(system.clock, frame);

        Ok(Duration::from_micros(100))
    }
}

impl Transmutable for SynthControl {
    fn as_steppable(&mut self) -> Option<&mut dyn Steppable> {
        Some(self)
    }
}

fn set_register(device: &mut dyn Addressable, bank: u8, reg: u8, data: u8) -> Result<(), Error> {
    let addr = (bank as Address) * 2;
    device.write_u8(Instant::START, addr, reg)?;
    device.write_u8(Instant::START, addr + 1, data)?;
    Ok(())
}

fn initialize_ym(ym_sound: Device) -> Result<(), Error> {
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
    let matches = moa_minifb::new("YM2612 Tester/Synth").get_matches();

    moa_minifb::run(matches, |host| {
        let mut system = System::default();

        let (frame_sender, frame_receiver) = moa_host::frame_queue(SCREEN_WIDTH, SCREEN_HEIGHT);
        let (key_sender, key_receiver) = moa_host::event_queue();
        let control = Device::new(SynthControl::new(key_receiver, frame_sender));
        system.add_device("control", control)?;

        let ym_sound = Device::new(Ym2612::new(host, Frequency::from_hz(7_670_454))?);
        initialize_ym(ym_sound.clone())?;
        system.add_addressable_device(0x00, ym_sound)?;

        let sn_sound = Device::new(Sn76489::new(host, Frequency::from_hz(3_579_545))?);
        system.add_addressable_device(0x10, sn_sound)?;

        host.add_video_source(frame_receiver)?;
        host.register_keyboard(key_sender)?;

        Ok(system)
    });
}
