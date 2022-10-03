
use std::rc::Rc;

use pixels::{Pixels, SurfaceTexture};
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode, WindowEvent, ElementState};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use instant::Instant;

use moa_core::{System, Error, Clock};
use moa_core::host::{Host, WindowUpdater, ControllerDevice, ControllerEvent, ControllerUpdater, Audio, DummyAudio};
use moa_core::host::gfx::Frame;

use crate::settings;

const WIDTH: u32 = 320;
const HEIGHT: u32 = 224;

pub type LoadSystemFn = fn (&mut PixelsFrontend, Vec<u8>) -> Result<System, Error>;

pub struct PixelsFrontend {
    updater: Option<Box<dyn WindowUpdater>>,
    controller: Option<Box<dyn ControllerUpdater>>,
}

impl PixelsFrontend {
    pub fn new() -> PixelsFrontend {
        PixelsFrontend {
            controller: None,
            updater: None,
        }
    }
}

impl Host for PixelsFrontend {
    fn add_window(&mut self, updater: Box<dyn WindowUpdater>) -> Result<(), Error> {
        self.updater = Some(updater);
        Ok(())
    }

    fn register_controller(&mut self, device: ControllerDevice, input: Box<dyn ControllerUpdater>) -> Result<(), Error> {
        if device != ControllerDevice::A {
            return Ok(())
        }

        self.controller = Some(input);
        Ok(())
    }

    fn create_audio_source(&mut self) -> Result<Box<dyn Audio>, Error> {
        Ok(Box::new(DummyAudio()))
    }    
}

pub async fn run(load: LoadSystemFn) {
    loop {
        let host = PixelsFrontend::new();
        run_loop(host, load).await
    }
}

pub async fn run_loop(mut host: PixelsFrontend, load: LoadSystemFn) {
    let event_loop = EventLoop::new();
    let window = {
        let size = LogicalSize::new(WIDTH as f64, HEIGHT as f64);
        WindowBuilder::new()
            .with_title("Hello Pixels + Web")
            .with_inner_size(size)
            .with_min_inner_size(size)
            .build(&event_loop)
            .expect("WindowBuilder error")
    };

    let window = Rc::new(window);

    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        use winit::platform::web::WindowExtWebSys;

        // Retrieve current width and height dimensions of browser client window
        let get_window_size = || {
            let client_window = web_sys::window().unwrap();
            LogicalSize::new(
                client_window.inner_width().unwrap().as_f64().unwrap(),
                client_window.inner_height().unwrap().as_f64().unwrap(),
            )
        };

        let window = Rc::clone(&window);

        // Initialize winit window with current dimensions of browser client
        window.set_inner_size(get_window_size());

        let client_window = web_sys::window().unwrap();

        // Attach winit canvas to body element
        web_sys::window()
            .and_then(|win| win.document())
            .and_then(|doc| doc.body())
            .and_then(|body| {
                body.append_child(&web_sys::Element::from(window.canvas()))
                    .ok()
            })
            .expect("couldn't append canvas to document body");

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

        /*
        let host = host.clone();
        let mut system = load(&mut host.lock().unwrap(), settings::get().rom_data.clone()).unwrap();
        let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move |_e: web_sys::Event| {
            let run_timer = Instant::now();
            let nanoseconds_per_frame = (16_600_000 as f32 * settings::get().speed) as Clock;
            if let Err(err) = system.run_for(nanoseconds_per_frame) {
                log::error!("{:?}", err);
            }
            log::info!("ran simulation for {:?}ms in {:?}ms", nanoseconds_per_frame / 1_000_000, run_timer.elapsed().as_millis());

            let mut settings = settings::get();
            if settings.reset {
                settings.reset = false;

                match load(&mut host.lock().unwrap(), settings.rom_data.clone()) {
                    Ok(s) => { system = s; },
                    Err(err) => log::error!("{:?}", err),
                }
            }
        }) as Box<dyn FnMut(_)>);
        client_window
            .set_interval_with_callback_and_timeout_and_arguments_0(closure.as_ref().unchecked_ref(), 17)
            .unwrap();
        closure.forget();
        */
    }

    //let mut input = WinitInputHelper::new();
    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture =
            SurfaceTexture::new(window_size.width, window_size.height, window.as_ref());
        Pixels::new_async(WIDTH, HEIGHT, surface_texture)
            .await
            .expect("Pixels error")
    };

    let mut last_frame = Frame::new(WIDTH, HEIGHT);
    let mut update_timer = Instant::now();
    let mut system = load(&mut host, settings::get().rom_data.clone()).unwrap();
    event_loop.run(move |event, _, control_flow| {

        // Draw the current frame
        if let Event::RedrawRequested(_) = event {
            settings::increment_frames();
            log::info!("updated after {:4}ms", update_timer.elapsed().as_millis());
            update_timer = Instant::now();

            let run_timer = Instant::now();
            let nanoseconds_per_frame = (16_600_000 as f32 * settings::get().speed) as Clock;
            if let Err(err) = system.run_for(nanoseconds_per_frame) {
                log::error!("{:?}", err);
            }
            log::info!("ran simulation for {:?}ms in {:?}ms", nanoseconds_per_frame / 1_000_000, run_timer.elapsed().as_millis());

            if let Some(updater) = host.updater.as_mut() {
                let buffer = pixels.get_frame();
                if let Ok(frame) = updater.take_frame() {
                    last_frame = frame;
                }

                for y in 0..last_frame.height {
                    for x in 0..last_frame.width {
                        let pixel = last_frame.bitmap[((y * last_frame.width) + x) as usize];

                        let i = ((y * WIDTH) + x) as usize;
                        buffer[i * 4] = (pixel >> 16) as u8;
                        buffer[i * 4 + 1] = (pixel >> 8) as u8;
                        buffer[i * 4 + 2] = pixel as u8;
                        buffer[i * 4 + 3] = 255;
                    }
                }
            }

            if pixels
                .render()
                .map_err(|e| println!("pixels.render() failed: {}", e))
                .is_err()
            {
                *control_flow = ControlFlow::Exit;
                return;
            }

            window.request_redraw();
        }

        let mut key = None;
        if let Event::WindowEvent { event: WindowEvent::KeyboardInput { input, .. }, .. } = event {
            if let Some(keycode) = input.virtual_keycode {
                match input.state {
                    ElementState::Pressed => {
                        key = map_controller_a(keycode, true);
                    }
                    ElementState::Released => {
                        key = map_controller_a(keycode, false);
                    }
                }
            }
        }

        if let Some(updater) = host.controller.as_mut() {
            if let Some(key) = key {
                updater.update_controller(key);
            }
        }

        let mut settings = settings::get();
        if settings.reset {
            settings.reset = false;

            match load(&mut host, settings.rom_data.clone()) {
                Ok(s) => { system = s; },
                Err(err) => log::error!("{:?}", err),
            }
        }
    });
}

pub fn map_controller_a(key: VirtualKeyCode, state: bool) -> Option<ControllerEvent> {
    match key {
        VirtualKeyCode::A => { Some(ControllerEvent::ButtonA(state)) },
        VirtualKeyCode::O => { Some(ControllerEvent::ButtonB(state)) },
        VirtualKeyCode::E => { Some(ControllerEvent::ButtonC(state)) },
        VirtualKeyCode::Up => { Some(ControllerEvent::DpadUp(state)) },
        VirtualKeyCode::Down => { Some(ControllerEvent::DpadDown(state)) },
        VirtualKeyCode::Left => { Some(ControllerEvent::DpadLeft(state)) },
        VirtualKeyCode::Right => { Some(ControllerEvent::DpadRight(state)) },
        VirtualKeyCode::Return => { Some(ControllerEvent::Start(state)) },
        VirtualKeyCode::M => { Some(ControllerEvent::Mode(state)) },
        _ => None,
    }
}

