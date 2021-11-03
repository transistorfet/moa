
use std::time::Duration;
use std::sync::{Arc, Mutex};

use minifb::{self, Key};

use moa::error::Error;
use moa::system::System;
use moa::host::traits::{Host, JoystickDevice, JoystickUpdater, WindowUpdater};


const WIDTH: usize = 320;
const HEIGHT: usize = 224;

pub struct MiniFrontend {
    pub buffer: Mutex<Vec<u32>>,
    pub updater: Mutex<Option<Box<dyn WindowUpdater>>>,
    pub input: Mutex<Option<Box<dyn JoystickUpdater>>>,
}

impl Host for MiniFrontend {
    fn add_window(&self, updater: Box<dyn WindowUpdater>) -> Result<(), Error> {
        let mut unlocked = self.updater.lock().unwrap();
        if unlocked.is_some() {
            return Err(Error::new("A window updater has already been registered with the frontend"));
        }
        *unlocked = Some(updater);
        Ok(())
    }

    fn register_joystick(&self, device: JoystickDevice, input: Box<dyn JoystickUpdater>) -> Result<(), Error> {
        if device != JoystickDevice::A {
            return Ok(())
        }

        let mut unlocked = self.input.lock().unwrap();
        if unlocked.is_some() {
            return Err(Error::new("A window updater has already been registered with the frontend"));
        }
        *unlocked = Some(input);
        Ok(())
    }
}

impl MiniFrontend {
    pub fn init_frontend() -> MiniFrontend {
        MiniFrontend {
            buffer: Mutex::new(vec![0; WIDTH * HEIGHT]),
            updater: Mutex::new(None),
            input: Mutex::new(None),
        }
    }

    //pub fn start(&self) {
    pub fn start(&self, mut system: System) {
        let mut options = minifb::WindowOptions::default();
        options.scale = minifb::Scale::X4;

        let mut window = minifb::Window::new(
            "Test - ESC to exit",
            WIDTH,
            HEIGHT,
            options,
        )
        .unwrap_or_else(|e| {
            panic!("{}", e);
        });

        // Limit to max ~60 fps update rate
        window.limit_update_rate(Some(Duration::from_micros(16600)));

        while window.is_open() && !window.is_key_down(Key::Escape) {
            system.run_for(16_600_000).unwrap();

            if let Some(keys) = window.get_keys_pressed(minifb::KeyRepeat::Yes) {
                let mut modifiers: u16 = 0;
                for key in keys {
                    match key {
                        Key::Enter => { modifiers |= 0xffff; },
                        Key::D => { system.enable_debugging(); },
                        _ => { },
                    }
                }
                if let Some(updater) = &mut *self.input.lock().unwrap() {
                    updater.update_joystick(modifiers);
                }
            }

            if let Some(updater) = &mut *self.updater.lock().unwrap() {
                let mut buffer = self.buffer.lock().unwrap();
                updater.update_frame(WIDTH as u32, HEIGHT as u32, &mut buffer);
                window.update_with_buffer(&buffer, WIDTH, HEIGHT).unwrap();
            }
        }
    }
}

