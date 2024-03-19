#![cfg(target_arch = "wasm32")]

use std::rc::Rc;
use std::cell::RefCell;
use instant::Instant;
use wasm_bindgen::prelude::*;
use winit::dpi::LogicalSize;
use winit::event_loop::EventLoop;
use winit::window::{Window, WindowBuilder};

use web_sys::Event;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

use femtos::{Duration as FemtosDuration};
use moa_core::{System, Device};
use moa_host::{ControllerInput, ControllerDevice, ControllerEvent, EventSender};

use crate::settings;
use crate::frontend::{self, PixelsFrontend, LoadSystemFn};

pub fn start(load: LoadSystemFn) {
    settings::set_rom_data(include_bytes!("../sonic.bin").to_vec());

    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Warn).expect("error initializing logger");

    //wasm_bindgen_futures::spawn_local(frontend::run(load));
}

#[wasm_bindgen]
pub fn set_rom_data(rom_data: Vec<u8>) {
    settings::set_rom_data(rom_data);
}

#[wasm_bindgen]
pub fn set_size(width: u32, height: u32) {
    settings::set_size(width, height);
}

#[wasm_bindgen]
pub fn request_stop() {
    settings::request_stop();
}

#[wasm_bindgen]
pub fn toggle_run() {
    settings::toggle_run();
}

#[wasm_bindgen]
pub fn is_running() -> bool {
    settings::get().run
}

#[wasm_bindgen]
pub fn set_speed(speed: f32) {
    //settings::get().speed = speed;
}

#[wasm_bindgen]
pub fn get_speed() -> f32 {
    settings::get().speed
}

#[wasm_bindgen]
pub fn get_frames_since() -> usize {
    settings::get_frames_since()
}

#[wasm_bindgen]
pub fn set_mute(mute: bool) {
    settings::get().mute = mute;
}

#[wasm_bindgen]
pub fn button_press(handle: &ControllersHandle, name: String, state: bool) {
    let input = match name.as_str() {
        "up" => Some(ControllerInput::DpadUp(state)),
        "down" => Some(ControllerInput::DpadDown(state)),
        "left" => Some(ControllerInput::DpadLeft(state)),
        "right" => Some(ControllerInput::DpadRight(state)),
        "start" => Some(ControllerInput::Start(state)),
        "a" => Some(ControllerInput::ButtonA(state)),
        "b" => Some(ControllerInput::ButtonB(state)),
        "c" => Some(ControllerInput::ButtonC(state)),
        _ => None
    };

    if let Some(input) = input {
        handle.0.send(ControllerEvent::new(ControllerDevice::A, input));
    }
}


#[wasm_bindgen]
pub struct HostHandle(PixelsFrontend);

#[wasm_bindgen]
pub struct ControllersHandle(EventSender<ControllerEvent>);

#[wasm_bindgen]
pub fn new_host() -> HostHandle {
    HostHandle(PixelsFrontend::new())
}

#[wasm_bindgen]
pub fn get_controllers(handle: &HostHandle) -> ControllersHandle {
    ControllersHandle(handle.0.get_controllers().unwrap())
}

#[wasm_bindgen]
pub fn host_run_loop(handle: HostHandle) {
    wasm_bindgen_futures::spawn_local(frontend::run_loop(handle.0));
}

#[wasm_bindgen]
pub struct SystemHandle(System);

#[wasm_bindgen]
pub struct LoadSystemFnHandle(LoadSystemFn);

impl LoadSystemFnHandle {
    pub fn new(load: LoadSystemFn) -> Self {
        Self(load)
    }
}

#[wasm_bindgen]
pub fn load_system(handle: &mut HostHandle, load: LoadSystemFnHandle) -> SystemHandle {
    let mut system = load.0(&mut handle.0, settings::get().rom_data.clone()).unwrap();
    let mixer = handle.0.get_mixer();
    if mixer.borrow_mut().num_sources() > 0 {
        system.add_device("mixer", Device::new(mixer.clone())).unwrap();
    }
    SystemHandle(system)
}

#[wasm_bindgen]
pub fn run_system_for(handle: &mut SystemHandle, nanos: u32) -> usize {
    let run_timer = Instant::now();
    let nanoseconds_per_frame = FemtosDuration::from_nanos(nanos as u64);
    //let nanoseconds_per_frame = (16_600_000 as f32 * settings::get().speed) as Clock;
    if let Err(err) = handle.0.run_for_duration(nanoseconds_per_frame) {
        log::error!("{:?}", err);
    }
    let run_time = run_timer.elapsed().as_millis();
    log::debug!("ran simulation for {:?}ms in {:?}ms", nanoseconds_per_frame / 1_000_000_u32, run_time);
    run_time as usize
}

pub fn create_window<T>(event_loop: &EventLoop<T>) -> Rc<Window> {
    use web_sys::HtmlCanvasElement;
    use winit::platform::web::{WindowExtWebSys, WindowBuilderExtWebSys};

    let canvas = web_sys::window()
        .and_then(|win| win.document())
        .and_then(|doc| doc.get_element_by_id("video"))
        .and_then(|el| el.dyn_into::<web_sys::HtmlCanvasElement>().ok())
        .expect("document to have canvas");

    let window = {
        let size = LogicalSize::new(frontend::WIDTH as f64, frontend::HEIGHT as f64);
        WindowBuilder::new()
            .with_canvas(Some(canvas))
            .with_title("Moa Emulator")
            //.with_inner_size(size)
            //.with_min_inner_size(size)
            .build(event_loop)
            .expect("WindowBuilder error")
    };

    let window = Rc::new(window);

    /*
    // Retrieve current width and height dimensions of browser client window
    let get_window_size = || {
        let client_window = web_sys::window().unwrap();
        LogicalSize::new(
            client_window.inner_width().unwrap().as_f64().unwrap(),
            client_window.inner_height().unwrap().as_f64().unwrap(),
        )
    };

    // Initialize winit window with current dimensions of browser client
    window.set_inner_size(get_window_size());
    */

    let client_window = web_sys::window().unwrap();

    /*
    // Attach winit canvas to body element
    web_sys::window()
        .and_then(|win| win.document())
        .and_then(|doc| doc.get_element_by_id("video-screen"))
        .and_then(|el| {
            while let Some(child) = el.first_child() {
                el.remove_child(&child);
            }
            el.append_child(&web_sys::Element::from(window.canvas()))
                .ok()
        })
        .expect("couldn't append canvas to document body");
    */

    /*
    {
        let window = window.clone();
        // Listen for resize event on browser client. Adjust winit window dimensions
        // on event trigger
        let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move |_e: web_sys::Event| {
            let size = get_window_size();
            window.set_inner_size(size)
        }) as Box<dyn FnMut(_)>);
        client_window
            .add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }
    */

    window
}


#[wasm_bindgen]
pub fn start_system(handle: SystemHandle) -> Handle {
    let emulator = Emulator::new(handle.0);
    set_timeout(emulator.clone(), 0);
    Handle(emulator)
}

#[wasm_bindgen]
pub struct Handle(Rc<RefCell<Emulator>>);

pub struct Emulator {
    running: bool,
    //frontend: PixelsFrontend,
    system: System,
    last_update: Instant,
}

impl Emulator {
    pub fn new(system: System) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            running: false,
            system,
            last_update: Instant::now(),
        }))
    }

    pub fn stop(&mut self) {
        self.running = false;
    }
}

/// This updater tries to work like the JS one, by simulating the amount of time that has passed
/// since the last update, but it doesn't work too well yet
fn update(emulator: Rc<RefCell<Emulator>>) {
    let run_timer = Instant::now();
    let last_update = {
        let mut emulator = emulator.borrow_mut();
        let last_update = emulator.last_update;
        emulator.last_update = run_timer;
        last_update
    };

    let diff = run_timer.duration_since(last_update);
    let nanoseconds_per_frame = FemtosDuration::from_nanos(diff.as_nanos() as u64);
    //let nanoseconds_per_frame = (16_600_000 as f32 * settings::get().speed) as Clock;
    if let Err(err) = emulator.borrow_mut().system.run_for_duration(nanoseconds_per_frame) {
        log::error!("{:?}", err);
    }
    let run_time = run_timer.elapsed().as_millis();
    log::debug!("ran simulation for {:?}ms in {:?}ms", nanoseconds_per_frame / 1_000_000_u32, run_time);

    let running = emulator.borrow().running;
    if running {
        let remaining = (diff.as_millis() - run_time - (diff.as_millis() / 10)).max(1);
        set_timeout(emulator, 16);
    }
}

/*
/// This is the constant-time updater which always tries to simulate one frame
fn update(emulator: Rc<RefCell<Emulator>>) {
    let run_timer = Instant::now();
    let nanoseconds_per_frame = (16_600_000 as f32 * settings::get().speed) as u64;
    if let Err(err) = emulator.borrow_mut().system.run_for_duration(FemtosDuration::from_nanos(nanoseconds_per_frame)) {
        log::error!("{:?}", err);
    }
    log::info!("ran simulation for {:?}ms in {:?}ms", nanoseconds_per_frame / 1_000_000, run_timer.elapsed().as_millis());

    let running = emulator.borrow().running;
    if running {
        set_timeout(emulator, 17);
    }
}
*/

fn set_timeout(emulator: Rc<RefCell<Emulator>>, timeout: i32) {
    emulator.borrow_mut().running = true;
    let closure: Closure<dyn Fn(Event)> = Closure::new(move |_e: Event| {
        update(emulator.clone());
    });

    let client_window = web_sys::window().unwrap();
    client_window
        .set_timeout_with_callback_and_timeout_and_arguments_0(closure.as_ref().unchecked_ref(), timeout)
        .unwrap();
    closure.forget();
}

