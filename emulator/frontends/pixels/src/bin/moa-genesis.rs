
use moa_pixels::{PixelsFrontend, start};

use moa_core::{System, Error};
use moa_genesis::{SegaGenesisOptions, build_genesis};

fn load_system(host: &mut PixelsFrontend, rom_data: Vec<u8>) -> Result<System, Error> {
    let mut options = SegaGenesisOptions::new();
    options.rom_data = Some(rom_data);
    build_genesis(host, options)
}

fn main() {
    start(load_system);
}

#[cfg(target_arch = "wasm32")]
mod web {
    use wasm_bindgen::prelude::*;
    use moa_genesis::utils;

    #[wasm_bindgen]
    pub fn smd_to_bin(input: Vec<u8>) -> Vec<u8> {
        utils::smd_to_bin(input).unwrap()
    }
}

