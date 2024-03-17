use cpal::{
    Stream, SampleRate, SampleFormat, StreamConfig, OutputCallbackInfo,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};

use crate::audio::{AudioOutput, SAMPLE_RATE};

#[allow(dead_code)]
pub struct CpalAudioOutput {
    stream: Stream,
}

impl CpalAudioOutput {
    pub fn create_audio_output(output: AudioOutput) -> CpalAudioOutput {
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

        let data_callback = move |data: &mut [f32], _info: &OutputCallbackInfo| {
            let mut index = 0;
            while index < data.len() {
                if let Some((clock, mut frame)) = output.receive() {
                    let size = (frame.data.len() * 2).min(data.len() - index);
                    frame
                        .data
                        .iter()
                        .zip(data[index..index + size].chunks_mut(2))
                        .for_each(|(sample, location)| {
                            location[0] = sample.0;
                            location[1] = sample.1;
                        });
                    index += size;
                    if size < frame.data.len() * 2 {
                        frame.data.drain(0..size / 2);
                        output.put_back(clock, frame);
                    }
                } else {
                    log::debug!("missed an audio frame");
                    break;
                }
            }
        };

        let stream = device
            .build_output_stream(
                &config,
                data_callback,
                move |err| {
                    log::error!("ERROR: {:?}", err);
                },
                None,
            )
            .unwrap();

        stream.play().unwrap();

        CpalAudioOutput {
            stream,
        }
    }

    pub fn set_mute(&self, mute: bool) {
        if mute {
            self.stream.pause().unwrap();
        } else {
            self.stream.play().unwrap();
        }
    }
}
