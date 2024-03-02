
use instant::Instant;
use pixels::{Pixels, SurfaceTexture};
use winit::event::{Event, VirtualKeyCode, WindowEvent, ElementState};
use winit::event_loop::{ControlFlow, EventLoop};

use moa_core::{System, Error};
use moa_host::{Host, HostError, PixelEncoding, Frame, ControllerDevice, ControllerInput, ControllerEvent, EventSender, Audio, DummyAudio, FrameReceiver};
use moa_common::{AudioMixer, AudioSource, CpalAudioOutput};

use crate::settings;
use crate::create_window;


pub const WIDTH: u32 = 320;
pub const HEIGHT: u32 = 224;

pub type LoadSystemFn = fn (&mut PixelsFrontend, Vec<u8>) -> Result<System, Error>;

pub struct PixelsFrontend {
    video: Option<FrameReceiver>,
    controllers: Option<EventSender<ControllerEvent>>,
    mixer: AudioMixer,
}

impl PixelsFrontend {
    pub fn new() -> PixelsFrontend {
        settings::get().run = true;
        let mixer = AudioMixer::with_default_rate();

        PixelsFrontend {
            video: None,
            controllers: None,
            mixer,
        }
    }

    pub fn get_mixer(&self) -> AudioMixer {
        self.mixer.clone()
    }

    pub fn get_controllers(&self) -> Option<EventSender<ControllerEvent>> {
        self.controllers.clone()
    }
}

impl Host for PixelsFrontend {
    type Error = Error;

    fn add_video_source(&mut self, receiver: FrameReceiver) -> Result<(), HostError<Self::Error>> {
        self.video = Some(receiver);
        Ok(())
    }

    fn register_controllers(&mut self, sender: EventSender<ControllerEvent>) -> Result<(), HostError<Self::Error>> {
        self.controllers = Some(sender);
        Ok(())
    }

    fn add_audio_source(&mut self) -> Result<Box<dyn Audio>, HostError<Self::Error>> {
        let source = AudioSource::new(self.mixer.clone());
        Ok(Box::new(source))
        //Ok(Box::new(DummyAudio()))
    }
}

pub async fn run_loop(host: PixelsFrontend) {
    let event_loop = EventLoop::new();

    let window = create_window(&event_loop);

    if let Some(receiver) = host.video.as_ref() {
        receiver.request_encoding(PixelEncoding::ABGR);
    }

    let mut audio_output = None;
    if host.mixer.borrow_mut().num_sources() > 0 {
        audio_output = Some(CpalAudioOutput::create_audio_output(host.mixer.borrow_mut().get_sink()));
    }

    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture =
            SurfaceTexture::new(window_size.width, window_size.height, window.as_ref());
        Pixels::new_async(WIDTH, HEIGHT, surface_texture)
            .await
            .expect("Pixels error")
    };

    let mut mute = false;
    let mut last_size = (WIDTH, HEIGHT);
    let mut last_frame = Frame::new(WIDTH, HEIGHT, PixelEncoding::ABGR);
    //let mut update_timer = Instant::now();
    event_loop.run(move |event, _, control_flow| {

        // Draw the current frame
        if let Event::RedrawRequested(_) = event {
            settings::increment_frames();

            //log::warn!("updated after {:4}ms", update_timer.elapsed().as_millis());
            //update_timer = Instant::now();

            if let Some(updater) = host.video.as_ref() {
                if let Some((clock, frame)) = updater.latest() {
                    last_frame = frame;
                }

                if (last_frame.width, last_frame.height) != last_size {
                    last_size = (last_frame.width, last_frame.height);
                    pixels.resize_buffer(last_frame.width, last_frame.height);
                }

                let buffer = pixels.frame_mut();
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

                if let Some(sender) = host.controllers.as_ref() {
                    if let Some(key) = key {
                        let event = ControllerEvent::new(ControllerDevice::A, key);
                        sender.send(event);
                    }
                }
            }
        }

        if let Some(output) = audio_output.as_ref() {
            let requested_mute = settings::get().mute;
            if requested_mute != mute {
                mute = requested_mute;
                output.set_mute(mute);
                log::info!("setting mute to {}", mute);
            }
        }

        // Check if the run flag is no longer true, and exit the loop
        if !settings::get().run {
            // Clear the screen
            let buffer = pixels.frame_mut();
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

pub fn map_controller_a(key: VirtualKeyCode, state: bool) -> Option<ControllerInput> {
    match key {
        VirtualKeyCode::A => { Some(ControllerInput::ButtonA(state)) },
        VirtualKeyCode::S => { Some(ControllerInput::ButtonB(state)) },
        VirtualKeyCode::D => { Some(ControllerInput::ButtonC(state)) },
        VirtualKeyCode::Up => { Some(ControllerInput::DpadUp(state)) },
        VirtualKeyCode::Down => { Some(ControllerInput::DpadDown(state)) },
        VirtualKeyCode::Left => { Some(ControllerInput::DpadLeft(state)) },
        VirtualKeyCode::Right => { Some(ControllerInput::DpadRight(state)) },
        VirtualKeyCode::Return => { Some(ControllerInput::Start(state)) },
        VirtualKeyCode::M => { Some(ControllerInput::Mode(state)) },
        _ => None,
    }
}

