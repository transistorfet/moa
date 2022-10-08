
use std::thread;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use minifb::{self, Key, MouseMode, MouseButton};
use clap::{App, Arg, ArgMatches};

use moa_core::{System, Error, Clock};
use moa_core::host::{Host, ControllerUpdater, KeyboardUpdater, KeyEvent, MouseUpdater, MouseState, WindowUpdater, Audio, ControllerDevice};
use moa_core::host::gfx::Frame;

use moa_common::audio::{AudioOutput, AudioMixer, AudioSource, CpalAudioOutput};

mod keys;
mod controllers;

use crate::keys::map_key;
use crate::controllers::map_controller_a;


const WIDTH: u32 = 320;
const HEIGHT: u32 = 224;


pub fn new(name: &str) -> App {
    App::new(name)
        .arg(Arg::new("scale")
            .short('s')
            .long("scale")
            .takes_value(true)
            .help("Scale the screen"))
        .arg(Arg::new("threaded")
            .short('t')
            .long("threaded")
            .help("Run the simulation in a separate thread"))
        .arg(Arg::new("speed")
            .short('x')
            .long("speed")
            .takes_value(true)
            .help("Adjust the speed of the simulation"))
        .arg(Arg::new("debugger")
            .short('d')
            .long("debugger")
            .help("Start the debugger before running machine"))
        .arg(Arg::new("disable-audio")
            .short('a')
            .long("disable-audio")
            .help("Disable audio output"))
}

pub fn run<I>(matches: ArgMatches, init: I) where I: FnOnce(&mut MiniFrontendBuilder) -> Result<System, Error> + Send + 'static {
    if matches.occurrences_of("threaded") > 0 {
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
    pub controller: Option<Box<dyn ControllerUpdater>>,
    pub keyboard: Option<Box<dyn KeyboardUpdater>>,
    pub mouse: Option<Box<dyn MouseUpdater>>,
    pub mixer: Option<Arc<Mutex<AudioMixer>>>,
    pub finalized: bool,
}

impl MiniFrontendBuilder {
    pub fn new() -> Self {
        Self {
            window: None,
            controller: None,
            keyboard: None,
            mouse: None,
            mixer: Some(AudioMixer::with_default_rate()),
            finalized: false,
        }
    }

    pub fn finalize(&mut self) {
        self.finalized = true;
    }

    pub fn build(&mut self) -> MiniFrontend {
        let window = std::mem::take(&mut self.window);
        let controller = std::mem::take(&mut self.controller);
        let keyboard = std::mem::take(&mut self.keyboard);
        let mouse = std::mem::take(&mut self.mouse);
        let mixer = std::mem::take(&mut self.mixer);
        MiniFrontend::new(window, controller, keyboard, mouse, mixer.unwrap())
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

    fn register_controller(&mut self, device: ControllerDevice, input: Box<dyn ControllerUpdater>) -> Result<(), Error> {
        if device != ControllerDevice::A {
            return Ok(())
        }

        if self.controller.is_some() {
            return Err(Error::new("A controller updater has already been registered with the frontend"));
        }
        self.controller = Some(input);
        Ok(())
    }

    fn register_keyboard(&mut self, input: Box<dyn KeyboardUpdater>) -> Result<(), Error> {
        if self.keyboard.is_some() {
            return Err(Error::new("A keyboard updater has already been registered with the frontend"));
        }
        self.keyboard = Some(input);
        Ok(())
    }

    fn register_mouse(&mut self, input: Box<dyn MouseUpdater>) -> Result<(), Error> {
        if self.mouse.is_some() {
            return Err(Error::new("A mouse updater has already been registered with the frontend"));
        }
        self.mouse = Some(input);
        Ok(())
    }

    fn create_audio_source(&mut self) -> Result<Box<dyn Audio>, Error> {
        let source = AudioSource::new(self.mixer.as_ref().unwrap().clone());
        Ok(Box::new(source))
    }
}


pub struct MiniFrontend {
    pub modifiers: u16,
    pub mouse_state: MouseState,
    pub window: Option<Box<dyn WindowUpdater>>,
    pub controller: Option<Box<dyn ControllerUpdater>>,
    pub keyboard: Option<Box<dyn KeyboardUpdater>>,
    pub mouse: Option<Box<dyn MouseUpdater>>,
    pub audio: Option<CpalAudioOutput>,
    pub mixer: Arc<Mutex<AudioMixer>>,
}

impl MiniFrontend {
    pub fn new(
        window: Option<Box<dyn WindowUpdater>>,
        controller: Option<Box<dyn ControllerUpdater>>,
        keyboard: Option<Box<dyn KeyboardUpdater>>,
        mouse: Option<Box<dyn MouseUpdater>>,
        mixer: Arc<Mutex<AudioMixer>>,
    ) -> Self {
        Self {
            modifiers: 0,
            mouse_state: Default::default(),
            window,
            controller,
            keyboard,
            mouse,
            audio: None,
            mixer,
        }
    }

    pub fn start(&mut self, matches: ArgMatches, mut system: Option<System>) {
        if matches.occurrences_of("debugger") > 0 {
            system.as_mut().map(|system| system.enable_debugging());
        }

        if matches.occurrences_of("disable-audio") <= 0 {
            self.audio = Some(CpalAudioOutput::create_audio_output(self.mixer.lock().unwrap().get_sink()));
        }

        let mut options = minifb::WindowOptions::default();
        options.scale = match matches.value_of("scale").map(|s| u8::from_str_radix(s, 10).unwrap()) {
            Some(1) => minifb::Scale::X1,
            Some(2) => minifb::Scale::X2,
            Some(4) => minifb::Scale::X4,
            Some(8) => minifb::Scale::X8,
            _ => minifb::Scale::X2,
        };

        let speed = match matches.value_of("speed") {
            Some(x) => f32::from_str(x).unwrap(),
            None => 1.0,
        };
        let nanoseconds_per_frame = (16_600_000 as f32 * speed) as Clock;

        let mut size = (WIDTH, HEIGHT);
        if let Some(updater) = self.window.as_mut() {
            size = updater.max_size();
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

        //let mut average_time = 0;
        let mut update_timer = Instant::now();
        let mut last_frame = Frame::new(size.0, size.1);
        while window.is_open() && !window.is_key_down(Key::Escape) {
            let frame_time = update_timer.elapsed();
            update_timer = Instant::now();
            //println!("new frame after {:?}us", frame_time.as_micros());

            let run_timer = Instant::now();
            if let Some(system) = system.as_mut() {
                //system.run_for(nanoseconds_per_frame).unwrap();
                system.run_for(frame_time.as_nanos() as u64).unwrap();
                //system.run_until_break().unwrap();
            }
            let sim_time = run_timer.elapsed().as_micros();
            //average_time = (average_time + sim_time) / 2;
            //println!("ran simulation for {:?}us in {:?}us (avg: {:?}us)", frame_time.as_nanos() / 1_000, sim_time, average_time);

            if let Some(keys) = window.get_keys_pressed(minifb::KeyRepeat::No) {
                for key in keys {
                    self.check_key(key, true);

                    // Process special keys
                    match key {
                        Key::D => { system.as_ref().map(|s| s.enable_debugging()); },
                        _ => { },
                    }
                }
            }
            if let Some(keys) = window.get_keys_released() {
                for key in keys {
                    self.check_key(key, false);
                }
            }

            if let Some(updater) = self.mouse.as_mut() {
                if let Some((x, y)) = window.get_mouse_pos(MouseMode::Clamp) {
                    let left = window.get_mouse_down(MouseButton::Left);
                    let right = window.get_mouse_down(MouseButton::Right);
                    let middle = window.get_mouse_down(MouseButton::Middle);

                    let next_state = MouseState::with(left, right, middle, x as u32, y as u32);
                    self.mouse_state
                        .to_events(next_state)
                        .into_iter()
                        .for_each(|event| updater.update_mouse(event));
                }
            }

            if let Some(updater) = self.window.as_mut() {
                if let Ok(frame) = updater.take_frame() {
                    last_frame = frame
                }
                window.update_with_buffer(&last_frame.bitmap, last_frame.width as usize, last_frame.height as usize).unwrap();
            }
        }
    }

    fn check_key(&mut self, key: Key, state: bool) {
        if let Some(updater) = self.keyboard.as_mut() {
            updater.update_keyboard(KeyEvent::new(map_key(key), state));
        }

        if let Some(updater) = self.controller.as_mut() {
            if let Some(event) = map_controller_a(key, state) {
                updater.update_controller(event);
            }
        }
    }
}

