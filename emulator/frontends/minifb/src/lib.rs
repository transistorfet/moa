use std::thread;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use minifb::{self, Key, MouseMode, MouseButton};
use clap::{Command, Arg, ArgAction, ArgMatches};
use femtos::{Duration as FemtosDuration};

use moa_core::{System, Error, Device};
use moa_debugger::{Debugger, DebugControl};
use moa_host::{
    Host, HostError, Audio, KeyEvent, MouseEvent, MouseState, ControllerDevice, ControllerEvent, EventSender, PixelEncoding, Frame,
    FrameReceiver,
};

use moa_common::{AudioMixer, AudioSource};
use moa_common::CpalAudioOutput;

mod controllers;
mod keys;

use crate::keys::map_key;
use crate::controllers::map_controller_a;


const WIDTH: u32 = 320;
const HEIGHT: u32 = 224;


pub fn new(name: &'static str) -> Command {
    Command::new(name)
        .arg(Arg::new("scale").short('s').long("scale").help("Scale the screen"))
        .arg(
            Arg::new("speed")
                .short('x')
                .long("speed")
                .help("Adjust the speed of the simulation"),
        )
        .arg(
            Arg::new("threaded")
                .short('t')
                .long("threaded")
                .action(ArgAction::SetTrue)
                .help("Run the simulation in a separate thread"),
        )
        .arg(
            Arg::new("log-level")
                .short('l')
                .long("log-level")
                .help("Set the type of log messages to print"),
        )
        .arg(
            Arg::new("debugger")
                .short('d')
                .long("debugger")
                .action(ArgAction::SetTrue)
                .help("Start the debugger before running machine"),
        )
        .arg(
            Arg::new("disable-audio")
                .short('a')
                .long("disable-audio")
                .action(ArgAction::SetTrue)
                .help("Disable audio output"),
        )
}

pub fn run<I>(matches: ArgMatches, init: I)
where
    I: FnOnce(&mut MiniFrontendBuilder) -> Result<System, Error> + Send + 'static,
{
    if matches.get_flag("threaded") {
        run_threaded(matches, init);
    } else {
        run_inline(matches, init);
    }
}

pub fn run_inline<I>(matches: ArgMatches, init: I)
where
    I: FnOnce(&mut MiniFrontendBuilder) -> Result<System, Error>,
{
    let mut frontend = MiniFrontendBuilder::default();
    let system = init(&mut frontend).unwrap();

    frontend.build().start(matches, Some(system));
}

pub fn run_threaded<I>(matches: ArgMatches, init: I)
where
    I: FnOnce(&mut MiniFrontendBuilder) -> Result<System, Error> + Send + 'static,
{
    let frontend = Arc::new(Mutex::new(MiniFrontendBuilder::default()));

    {
        let frontend = frontend.clone();
        thread::spawn(move || {
            let mut system = init(&mut frontend.lock().unwrap()).unwrap();
            frontend.lock().unwrap().finalize();
            system.run_forever().unwrap();
        });
    }

    wait_until_initialized(frontend.clone());

    frontend.lock().unwrap().build().start(matches, None);
}

fn wait_until_initialized(frontend: Arc<Mutex<MiniFrontendBuilder>>) {
    while !frontend.lock().unwrap().finalized {
        thread::sleep(Duration::from_millis(10));
    }
}


pub struct MiniFrontendBuilder {
    video: Option<FrameReceiver>,
    controllers: Option<EventSender<ControllerEvent>>,
    keyboard: Option<EventSender<KeyEvent>>,
    mouse: Option<EventSender<MouseEvent>>,
    mixer: Option<AudioMixer>,
    finalized: bool,
}

impl Default for MiniFrontendBuilder {
    fn default() -> Self {
        Self {
            video: None,
            controllers: None,
            keyboard: None,
            mouse: None,
            mixer: Some(AudioMixer::with_default_rate()),
            finalized: false,
        }
    }
}

impl MiniFrontendBuilder {
    pub fn finalize(&mut self) {
        self.finalized = true;
    }

    pub fn build(&mut self) -> MiniFrontend {
        let video = std::mem::take(&mut self.video);
        let controllers = std::mem::take(&mut self.controllers);
        let keyboard = std::mem::take(&mut self.keyboard);
        let mouse = std::mem::take(&mut self.mouse);
        let mixer = std::mem::take(&mut self.mixer);
        MiniFrontend::new(video, controllers, keyboard, mouse, mixer.unwrap())
    }
}

impl Host for MiniFrontendBuilder {
    type Error = Error;

    fn add_video_source(&mut self, receiver: FrameReceiver) -> Result<(), HostError<Self::Error>> {
        if self.video.is_some() {
            return Err(HostError::Specific(Error::new("Only one video source can be registered with this frontend")));
        }
        self.video = Some(receiver);
        Ok(())
    }

    fn add_audio_source(&mut self) -> Result<Box<dyn Audio>, HostError<Self::Error>> {
        let source = AudioSource::new(self.mixer.as_ref().unwrap().clone());
        Ok(Box::new(source))
    }

    fn register_controllers(&mut self, sender: EventSender<ControllerEvent>) -> Result<(), HostError<Self::Error>> {
        if self.controllers.is_some() {
            return Err(HostError::Specific(Error::new(
                "A controller updater has already been registered with the frontend",
            )));
        }
        self.controllers = Some(sender);
        Ok(())
    }

    fn register_keyboard(&mut self, sender: EventSender<KeyEvent>) -> Result<(), HostError<Self::Error>> {
        if self.keyboard.is_some() {
            return Err(HostError::Specific(Error::new("A keyboard updater has already been registered with the frontend")));
        }
        self.keyboard = Some(sender);
        Ok(())
    }

    fn register_mouse(&mut self, sender: EventSender<MouseEvent>) -> Result<(), HostError<Self::Error>> {
        if self.mouse.is_some() {
            return Err(HostError::Specific(Error::new("A mouse updater has already been registered with the frontend")));
        }
        self.mouse = Some(sender);
        Ok(())
    }
}


pub struct MiniFrontend {
    pub modifiers: u16,
    pub mouse_state: MouseState,
    pub video: Option<FrameReceiver>,
    pub controllers: Option<EventSender<ControllerEvent>>,
    pub keyboard: Option<EventSender<KeyEvent>>,
    pub mouse: Option<EventSender<MouseEvent>>,
    pub audio: Option<CpalAudioOutput>,
    pub mixer: AudioMixer,
}

impl MiniFrontend {
    pub fn new(
        video: Option<FrameReceiver>,
        controllers: Option<EventSender<ControllerEvent>>,
        keyboard: Option<EventSender<KeyEvent>>,
        mouse: Option<EventSender<MouseEvent>>,
        mixer: AudioMixer,
    ) -> Self {
        Self {
            modifiers: 0,
            mouse_state: Default::default(),
            video,
            controllers,
            keyboard,
            mouse,
            audio: None,
            mixer,
        }
    }

    pub fn start(&mut self, matches: ArgMatches, mut system: Option<System>) {
        let log_level = match matches.get_one("log-level").map(|s: &String| s.as_str()) {
            Some("trace") => log::Level::Trace,
            Some("debug") => log::Level::Debug,
            Some("info") => log::Level::Info,
            Some("warn") => log::Level::Warn,
            Some("error") => log::Level::Error,
            _ => log::Level::Warn,
        };

        simple_logger::SimpleLogger::new()
            .with_level(log_level.to_level_filter())
            .without_timestamps()
            .init()
            .unwrap();

        if self.mixer.borrow_mut().num_sources() != 0 && !matches.get_flag("disable-audio") {
            if let Some(system) = system.as_mut() {
                system.add_device("mixer", Device::new(self.mixer.clone())).unwrap();
            }
            self.audio = Some(CpalAudioOutput::create_audio_output(self.mixer.borrow_mut().get_sink()));
        }

        let options = minifb::WindowOptions {
            scale: match matches.get_one::<String>("scale").map(|s| s.parse::<u8>().unwrap()) {
                Some(1) => minifb::Scale::X1,
                Some(2) => minifb::Scale::X2,
                Some(4) => minifb::Scale::X4,
                Some(8) => minifb::Scale::X8,
                _ => minifb::Scale::X2,
            },
            ..Default::default()
        };

        let speed = matches.get_one::<f32>("speed").cloned().unwrap_or(1.0);

        let mut size = (WIDTH, HEIGHT);
        if let Some(queue) = self.video.as_mut() {
            size = queue.max_size();
            queue.request_encoding(PixelEncoding::ARGB);
        }

        let mut window = minifb::Window::new("Test - ESC to exit", size.0 as usize, size.1 as usize, options).unwrap_or_else(|e| {
            panic!("{}", e);
        });

        // Limit to max ~60 fps update rate
        window.limit_update_rate(Some(Duration::from_micros(16600)));
        //let nanoseconds_per_frame = (16_600_000 as f32 * speed) as u64;

        let mut debugger = Debugger::default();
        let mut run_debugger = matches.get_flag("debugger");
        let mut update_timer = Instant::now();
        let mut last_frame = Frame::new(size.0, size.1, PixelEncoding::ARGB);
        while window.is_open() && !window.is_key_down(Key::Escape) {
            if run_debugger {
                if let Some(system) = system.as_mut() {
                    debugger.print_step(system).unwrap();
                    if debugger.check_auto_command(system).unwrap() != DebugControl::Continue {
                        let mut buffer = String::new();
                        io::stdout().write_all(b"> ").unwrap();
                        io::stdin().read_line(&mut buffer).unwrap();
                        match debugger.run_command(system, &buffer) {
                            Ok(DebugControl::Exit) => {
                                run_debugger = false;
                            },
                            Ok(_) => {},
                            Err(err) => {
                                println!("Error: {:?}", err);
                            },
                        }
                    }
                }
            } else {
                let frame_time = update_timer.elapsed();
                update_timer = Instant::now();
                //println!("new frame after {:?}us", frame_time.as_micros());

                //let run_timer = Instant::now();
                if let Some(system) = system.as_mut() {
                    //system.run_for(nanoseconds_per_frame).unwrap();
                    match system.run_for_duration(FemtosDuration::from_nanos((frame_time.as_nanos() as f32 * speed) as u64)) {
                        Ok(()) => {},
                        Err(Error::Breakpoint(_)) => {
                            run_debugger = true;
                        },
                        Err(err) => panic!("{:?}", err),
                    }
                    //system.run_until_break().unwrap();
                }
                //let sim_time = run_timer.elapsed().as_micros();
                //println!("ran simulation for {:?}us in {:?}us (avg: {:?}us)", frame_time.as_micros(), sim_time, frame_time.as_micros() as f64 / sim_time as f64);
            }

            for key in window.get_keys_pressed(minifb::KeyRepeat::No) {
                self.check_key(key, true);

                // Process special keys
                if let Key::D = key {
                    run_debugger = true;
                }
            }
            for key in window.get_keys_released() {
                self.check_key(key, false);
            }

            if let Some(sender) = self.mouse.as_mut() {
                if let Some((x, y)) = window.get_mouse_pos(MouseMode::Clamp) {
                    let left = window.get_mouse_down(MouseButton::Left);
                    let right = window.get_mouse_down(MouseButton::Right);
                    let middle = window.get_mouse_down(MouseButton::Middle);

                    let next_state = MouseState::with(left, right, middle, x as u32, y as u32);
                    self.mouse_state
                        .to_events(next_state)
                        .into_iter()
                        .for_each(|event| sender.send(event));
                }
            }

            if let Some(queue) = self.video.as_mut() {
                if let Some((_clock, frame)) = queue.latest() {
                    last_frame = frame
                }
                window
                    .update_with_buffer(&last_frame.bitmap, last_frame.width as usize, last_frame.height as usize)
                    .unwrap();
            }
        }
    }

    fn check_key(&mut self, key: Key, state: bool) {
        if let Some(sender) = self.keyboard.as_mut() {
            sender.send(KeyEvent::new(map_key(key), state));
        }

        if let Some(sender) = self.controllers.as_mut() {
            if let Some(input) = map_controller_a(key, state) {
                let event = ControllerEvent::new(ControllerDevice::A, input);
                sender.send(event);
            }
        }
    }
}
