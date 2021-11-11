
mod keys;

use std::thread;
use std::time::Duration;
use std::sync::{Arc, Mutex};

use minifb::{self, Key};

use moa::error::Error;
use moa::system::System;
use moa::host::traits::{Host, JoystickDevice, JoystickUpdater, KeyboardUpdater, WindowUpdater};

use crate::keys::map_key;


//const WIDTH: usize = 320;
//const HEIGHT: usize = 224;

const WIDTH: usize = 384;
const HEIGHT: usize = 128;

pub struct MiniFrontendBuilder {
    pub window: Option<Box<dyn WindowUpdater>>,
    pub joystick: Option<Box<dyn JoystickUpdater>>,
    pub keyboard: Option<Box<dyn KeyboardUpdater>>,
    pub finalized: bool,
}

impl MiniFrontendBuilder {
    pub fn new() -> Self {
        Self {
            window: None,
            joystick: None,
            keyboard: None,
            finalized: false,
        }
    }

    pub fn finalize(&mut self) {
        self.finalized = true;
    }

    pub fn build(&mut self) -> MiniFrontend {
        let window = std::mem::take(&mut self.window);
        let joystick = std::mem::take(&mut self.joystick);
        let keyboard = std::mem::take(&mut self.keyboard);
        MiniFrontend::new(window, joystick, keyboard)
    }
}

impl Host for MiniFrontendBuilder {
    fn add_window(&mut self, updater: Box<dyn WindowUpdater>) -> Result<(), Error> {
        if self.window.is_some() {
            return Err(Error::new("A window updater has already been registered with the frontend"));
        }
        self.window = Some(updater);
        Ok(())
    }

    fn register_joystick(&mut self, device: JoystickDevice, input: Box<dyn JoystickUpdater>) -> Result<(), Error> {
        if device != JoystickDevice::A {
            return Ok(())
        }

        if self.joystick.is_some() {
            return Err(Error::new("A joystick updater has already been registered with the frontend"));
        }
        self.joystick = Some(input);
        Ok(())
    }

    fn register_keyboard(&mut self, input: Box<dyn KeyboardUpdater>) -> Result<(), Error> {
        if self.keyboard.is_some() {
            return Err(Error::new("A keyboard updater has already been registered with the frontend"));
        }
        self.keyboard = Some(input);
        Ok(())
    }
}


pub struct MiniFrontend {
    pub buffer: Vec<u32>,
    pub window: Option<Box<dyn WindowUpdater>>,
    pub joystick: Option<Box<dyn JoystickUpdater>>,
    pub keyboard: Option<Box<dyn KeyboardUpdater>>,
}

impl MiniFrontend {
    pub fn new(window: Option<Box<dyn WindowUpdater>>, joystick: Option<Box<dyn JoystickUpdater>>, keyboard: Option<Box<dyn KeyboardUpdater>>) -> Self {
        Self {
            buffer: vec![0; WIDTH * HEIGHT],
            window,
            joystick,
            keyboard,
        }
    }

    pub fn start(&mut self, mut system: Option<System>) {
        let mut options = minifb::WindowOptions::default();
        options.scale = minifb::Scale::X2;

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
            if let Some(system) = system.as_mut() {
                system.run_for(16_600_000).unwrap();
            }

            if let Some(keys) = window.get_keys_pressed(minifb::KeyRepeat::Yes) {
                let mut modifiers: u16 = 0;
                for key in keys {
                    if let Some(updater) = self.keyboard.as_mut() {
                        updater.update_keyboard(map_key(key), true);
                    }
                    match key {
                        Key::Enter => { modifiers |= 0xffff; },
                        Key::D => { system.as_ref().map(|s| s.enable_debugging()); },
                        _ => { },
                    }
                }
                if let Some(updater) = self.joystick.as_mut() {
                    updater.update_joystick(modifiers);
                }
            }
            if let Some(keys) = window.get_keys_released() {
                for key in keys {
                    if let Some(updater) = self.keyboard.as_mut() {
                        updater.update_keyboard(map_key(key), false);
                    }
                }
            }

            if let Some(updater) = self.window.as_mut() {
                updater.update_frame(WIDTH as u32, HEIGHT as u32, &mut self.buffer);
                window.update_with_buffer(&self.buffer, WIDTH, HEIGHT).unwrap();
            }
        }
    }
}


pub fn run_inline<I>(init: I) where I: FnOnce(&mut MiniFrontendBuilder) -> Result<System, Error> {
    let mut frontend = MiniFrontendBuilder::new();
    let system = init(&mut frontend).unwrap();

    frontend
        .build()
        .start(Some(system));
}

pub fn run_threaded<I>(init: I) where I: FnOnce(&mut MiniFrontendBuilder) -> Result<System, Error> + Send + 'static {
    let frontend = Arc::new(Mutex::new(MiniFrontendBuilder::new()));

    {
        let frontend = frontend.clone();
        thread::spawn(move || {
            let mut system = init(&mut *(frontend.lock().unwrap())).unwrap();
            frontend.lock().unwrap().finalize();
            system.run_loop();
        });
    }

    wait_until_initialized(frontend.clone());

    frontend
        .lock().unwrap()
        .build()
        .start(None);
}

fn wait_until_initialized(frontend: Arc<Mutex<MiniFrontendBuilder>>) {
    while frontend.lock().unwrap().finalized == false {
        thread::sleep(Duration::from_millis(10));
    }
}


