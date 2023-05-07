#![cfg(not(target_arch = "wasm32"))]

use std::rc::Rc;
use winit::dpi::LogicalSize;
use winit::event_loop::EventLoop;
use winit::window::{Window, WindowBuilder};

use crate::frontend::{self, LoadSystemFn, PixelsFrontend};

pub fn start(load: LoadSystemFn) {
    env_logger::init();

    let host = PixelsFrontend::new();
    pollster::block_on(frontend::run_loop(host));
}

pub fn create_window<T>(event_loop: &EventLoop<T>) -> Rc<Window> {
    let size = LogicalSize::new(frontend::WIDTH as f64, frontend::HEIGHT as f64);
    let window = WindowBuilder::new()
        .with_title("Hello Pixels + Web")
        .with_inner_size(size)
        .with_min_inner_size(size)
        .build(event_loop)
        .expect("WindowBuilder error");

    let window = Rc::new(window);
    window
}

