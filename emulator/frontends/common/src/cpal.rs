
use std::sync::{Arc, Mutex};
use cpal::{Stream, SampleRate, SampleFormat, StreamConfig, traits::{DeviceTrait, HostTrait, StreamTrait}};

use moa_core::{warn, error};

use crate::audio::{AudioOutput, SAMPLE_RATE};

#[allow(dead_code)]
pub struct CpalAudioOutput {
    stream: Stream,
}

impl CpalAudioOutput {
    pub fn create_audio_output(output: Arc<Mutex<AudioOutput>>) -> CpalAudioOutput {
        let device = cpal::default_host()
            .default_output_device()
            .expect("No sound output device available");

        let config: StreamConfig = device
            .supported_output_configs()
            .expect("error while querying configs")
            .find(|config| config.sample_format() == SampleFormat::F32 && config.channels() == 2)
            .expect("no supported config?!")
            .with_sample_rate(SampleRate(SAMPLE_RATE as u32))
            .into();

        let data_callback = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            let result = if let Ok(mut output) = output.lock() {
                output.set_frame_size(data.len() / 2);
                output.pop_next()
            } else {
                return;
            };

            if let Some(frame) = result {
                let (start, middle, end) = unsafe { frame.data.align_to::<f32>() };
                if !start.is_empty() || !end.is_empty() {
                    warn!("audio: frame wasn't aligned");
                }
                let length = middle.len().min(data.len());
                data[..length].copy_from_slice(&middle[..length]);
            } else {
                warn!("missed an audio frame");
            }
        };

        let stream = device.build_output_stream(
            &config,
            data_callback,
            move |err| {
                error!("ERROR: {:?}", err);
            },
        ).unwrap();

        stream.play().unwrap();

        CpalAudioOutput {
            stream,
        }
    }
}

