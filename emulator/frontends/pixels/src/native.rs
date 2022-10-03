#![cfg(not(target_arch = "wasm32"))]

use crate::frontend::{self, LoadSystemFn};

pub fn start(load: LoadSystemFn) {
    env_logger::init();

    pollster::block_on(frontend::run(load));
}

