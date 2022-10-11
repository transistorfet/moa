
use std::sync::{Mutex, MutexGuard};

static EMULATOR_OPTIONS: Mutex<EmulatorSettings> = Mutex::new(EmulatorSettings::new());

pub struct EmulatorSettings {
    pub rom_data: Vec<u8>,
    pub run: bool,
    pub speed: f32,
    pub frames_since: usize,
}

impl EmulatorSettings {
    const fn new() -> Self {
        Self {
            rom_data: vec![],
            run: false,
            speed: 4.0,
            frames_since: 0,
        }
    }
}

pub fn get<'a>() -> MutexGuard<'a, EmulatorSettings> {
    EMULATOR_OPTIONS.lock().unwrap()
}

pub fn set_rom_data(rom_data: Vec<u8>) {
    get().rom_data = rom_data;
}

pub fn get_frames_since() -> usize {
    let mut options = get();
    let frames_since = options.frames_since;
    options.frames_since = 0;
    frames_since
}

pub fn increment_frames() {
    get().frames_since += 1;
}

pub fn request_stop() {
    get().run = false;
}

pub fn toggle_run() {
    let mut options = get();
    options.run = !options.run;
}

