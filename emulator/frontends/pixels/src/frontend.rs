
use std::sync::{Arc, Mutex};

use instant::Instant;
use pixels::{Pixels, SurfaceTexture};
use winit::event::{Event, VirtualKeyCode, WindowEvent, ElementState};
use winit::event_loop::{ControlFlow, EventLoop};

use moa_core::{System, Error};
use moa_core::host::{Host, WindowUpdater, ControllerDevice, ControllerEvent, ControllerUpdater, Audio, DummyAudio};
use moa_core::host::gfx::{PixelEncoding, Frame};
use moa_common::{AudioMixer, AudioSource, CpalAudioOutput};

use crate::settings;
use crate::create_window;


pub const WIDTH: u32 = 320;
pub const HEIGHT: u32 = 224;

pub type LoadSystemFn = fn (&mut PixelsFrontend, Vec<u8>) -> Result<System, Error>;

pub struct PixelsFrontend {
    updater: Option<Box<dyn WindowUpdater>>,
    controller: Option<Box<dyn ControllerUpdater>>,
    //mixer: Arc<Mutex<AudioMixer>>,
    //audio_output: CpalAudioOutput,
}

impl PixelsFrontend {
    pub fn new() -> PixelsFrontend {
        settings::get().run = true;
        //let mixer = AudioMixer::with_default_rate();
        //let audio_output = CpalAudioOutput::create_audio_output(mixer.lock().unwrap().get_sink());

        PixelsFrontend {
            controller: None,
            updater: None,
            //mixer,
            //audio_output,
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
        //let source = AudioSource::new(self.mixer.clone());
        //Ok(Box::new(source))
        Ok(Box::new(DummyAudio()))
    }
}

pub async fn run_loop(mut host: PixelsFrontend) {
    let event_loop = EventLoop::new();

    let window = create_window(&event_loop);

    if let Some(updater) = host.updater.as_mut() {
        updater.request_encoding(PixelEncoding::ABGR);
    }

    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture =
            SurfaceTexture::new(window_size.width, window_size.height, window.as_ref());
        Pixels::new_async(WIDTH, HEIGHT, surface_texture)
            .await
            .expect("Pixels error")
    };

    let mut last_size = (WIDTH, HEIGHT);
    let mut last_frame = Frame::new(WIDTH, HEIGHT, PixelEncoding::ABGR);
    //let mut update_timer = Instant::now();
    event_loop.run(move |event, _, control_flow| {

        // Draw the current frame
        if let Event::RedrawRequested(_) = event {
            settings::increment_frames();

            //log::warn!("updated after {:4}ms", update_timer.elapsed().as_millis());
            //update_timer = Instant::now();

            if let Some(updater) = host.updater.as_mut() {
                if let Ok(frame) = updater.take_frame() {
                    last_frame = frame;
                }

                if (last_frame.width, last_frame.height) != last_size {
                    last_size = (last_frame.width, last_frame.height);
                    pixels.resize_buffer(last_frame.width, last_frame.height);
                }

                let buffer = pixels.get_frame();
                buffer.copy_from_slice(unsafe { std::slice::from_raw_parts(last_frame.bitmap.as_ptr() as *const u8, last_frame.bitmap.len() * 4) });
            }

            if pixels
                .render()
                .map_err(|e| log::error!("pixels.render() failed: {}", e))
                .is_err()
            {
                *control_flow = ControlFlow::Exit;
                return;
            }

            window.request_redraw();
        }

        // Process key inputs and pass them to the emulator's controller device
        if let Event::WindowEvent { event: WindowEvent::KeyboardInput { input, .. }, .. } = event {
            if let Some(keycode) = input.virtual_keycode {
                let key = match input.state {
                    ElementState::Pressed => {
                        map_controller_a(keycode, true)
                    }
                    ElementState::Released => {
                        map_controller_a(keycode, false)
                    }
                };

                if let Some(updater) = host.controller.as_mut() {
                    if let Some(key) = key {
                        updater.update_controller(key);
                    }
                }
            }
        }

        // Check if the run flag is no longer true, and exit the loop
        if !settings::get().run {
            // Clear the screen
            let buffer = pixels.get_frame();
            buffer.iter_mut().for_each(|byte| *byte = 0);

            if pixels
                .render()
                .map_err(|e| log::error!("pixels.render() failed: {}", e))
                .is_err()
            {
                *control_flow = ControlFlow::Exit;
                return;
            }

            window.request_redraw();

            *control_flow = ControlFlow::Exit;
        }
    });
}

pub fn map_controller_a(key: VirtualKeyCode, state: bool) -> Option<ControllerEvent> {
    match key {
        VirtualKeyCode::A => { Some(ControllerEvent::ButtonA(state)) },
        VirtualKeyCode::S => { Some(ControllerEvent::ButtonB(state)) },
        VirtualKeyCode::D => { Some(ControllerEvent::ButtonC(state)) },
        VirtualKeyCode::Up => { Some(ControllerEvent::DpadUp(state)) },
        VirtualKeyCode::Down => { Some(ControllerEvent::DpadDown(state)) },
        VirtualKeyCode::Left => { Some(ControllerEvent::DpadLeft(state)) },
        VirtualKeyCode::Right => { Some(ControllerEvent::DpadRight(state)) },
        VirtualKeyCode::Return => { Some(ControllerEvent::Start(state)) },
        VirtualKeyCode::M => { Some(ControllerEvent::Mode(state)) },
        _ => None,
    }
}

