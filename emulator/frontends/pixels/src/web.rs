#![cfg(target_arch = "wasm32")]

use wasm_bindgen::prelude::*;

use crate::settings;
use crate::frontend::{self, LoadSystemFn};

pub fn start(load: LoadSystemFn) {
    settings::set_rom_data(include_bytes!("../sonic.bin").to_vec());

    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Info).expect("error initializing logger");

    wasm_bindgen_futures::spawn_local(frontend::run(load));
}

#[wasm_bindgen]
pub fn set_rom_data(rom_data: Vec<u8>) {
    settings::set_rom_data(rom_data);
}

#[wasm_bindgen]
pub fn request_reset() {
    settings::request_reset();
}

#[wasm_bindgen]
pub fn toggle_run() {
    settings::toggle_run();
}

#[wasm_bindgen]
pub fn set_speed(speed: f32) {
    settings::set_speed(speed);
}

#[wasm_bindgen]
pub fn get_frames_since() -> usize {
    settings::get_frames_since()
}

