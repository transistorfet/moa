
use std::thread;
use std::time::Duration;
use std::sync::{Arc, Mutex};

use minifb::{self, Key};
use clap::{App, ArgMatches};

use moa::error::Error;
use moa::system::System;
use moa::host::traits::{Host, JoystickDevice, JoystickUpdater, KeyboardUpdater, WindowUpdater};

mod keys;
use crate::keys::map_key;


const WIDTH: u32 = 320;
const HEIGHT: u32 = 224;


pub fn new(name: &str) -> App {
    App::new(name)
        .arg("-s, --scale=[1,2,4]    'Scale the screen'")
        .arg("-t, --threaded         'Run the simulation in a separate thread'")
}

pub fn run<I>(matches: ArgMatches, init: I) where I: FnOnce(&mut MiniFrontendBuilder) -> Result<System, Error> + Send + 'static {
    if matches.value_of("threaded").is_some() {
        run_threaded(matches, init);
    } else {
        run_inline(matches, init);
    }
}

pub fn run_inline<I>(matches: ArgMatches, init: I) where I: FnOnce(&mut MiniFrontendBuilder) -> Result<System, Error> {
    let mut frontend = MiniFrontendBuilder::new();
    let system = init(&mut frontend).unwrap();

    frontend
        .build()
        .start(matches, Some(system));
}

pub fn run_threaded<I>(matches: ArgMatches, init: I) where I: FnOnce(&mut MiniFrontendBuilder) -> Result<System, Error> + Send + 'static {
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
        .start(matches, None);
}

fn wait_until_initialized(frontend: Arc<Mutex<MiniFrontendBuilder>>) {
    while frontend.lock().unwrap().finalized == false {
        thread::sleep(Duration::from_millis(10));
    }
}


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
            buffer: vec![0; (WIDTH * HEIGHT) as usize],
            window,
            joystick,
            keyboard,
        }
    }

    pub fn start(&mut self, matches: ArgMatches, mut system: Option<System>) {
        let mut options = minifb::WindowOptions::default();
        options.scale = match matches.value_of("scale").map(|s| u8::from_str_radix(s, 10).unwrap()) {
            Some(1) => minifb::Scale::X1,
            Some(2) => minifb::Scale::X2,
            Some(4) => minifb::Scale::X4,
            _ => minifb::Scale::X2,
        };

        let mut size = (WIDTH, HEIGHT);
        if let Some(updater) = self.window.as_mut() {
            size = updater.get_size();
            self.buffer = vec![0; (size.0 * size.1) as usize];
        }

        let mut window = minifb::Window::new(
            "Test - ESC to exit",
            size.0 as usize,
            size.1 as usize,
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
                updater.update_frame(size.0, size.1, &mut self.buffer);
                window.update_with_buffer(&self.buffer, size.0 as usize, size.1 as usize).unwrap();
            }
        }
    }
}

