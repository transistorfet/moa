
use cpal::{Sample, Stream, SampleRate, SampleFormat, StreamConfig, traits::{DeviceTrait, HostTrait, StreamTrait}};

use moa::host::traits::{HostData, Audio};
use crate::circularbuf::CircularBuffer;

const SAMPLE_RATE: usize = 48000;


pub struct AudioSource {
    sample_rate: usize,
    frame_size: usize,
    sequence_num: usize,
    mixer: HostData<AudioMixer>,
    buffer: CircularBuffer<f32>,
}

impl AudioSource {
    pub fn new(mixer: HostData<AudioMixer>) -> Self {
        let sample_rate = mixer.lock().sample_rate();
        let frame_size = mixer.lock().frame_size();
        let buffer = CircularBuffer::new(frame_size * 2, 0.0);

        Self {
            sample_rate,
            frame_size,
            sequence_num: 0,
            mixer,
            buffer,
        }
    }

    pub fn space_available(&self) -> usize {
        self.buffer.free_space() / 2
    }

    pub fn fill_with(&mut self, buffer: &[f32]) {
        if self.buffer.free_space() > buffer.len() * 2 {
            for sample in buffer.iter() {
                // TODO this is here to keep it quiet for testing, but should be removed later
                let sample = 0.5 * *sample;
                self.buffer.insert(sample);
                self.buffer.insert(sample);
            }
        }

        self.flush();
    }

    pub fn flush(&mut self) {
        if self.buffer.used_space() >= self.frame_size {
            let mut locked_mixer = self.mixer.lock();

            let mixer_sequence_num = locked_mixer.sequence_num();
            if mixer_sequence_num == self.sequence_num {
                return;
            }
            self.sequence_num = mixer_sequence_num;

            for i in 0..locked_mixer.buffer.len() {
                locked_mixer.buffer[i] += self.buffer.next().unwrap_or(0.0);
            }

            self.frame_size = locked_mixer.frame_size();
            self.buffer.resize(self.frame_size * 2);
        }
    }
}

impl Audio for AudioSource {
    fn samples_per_second(&self) -> usize {
        self.sample_rate
    }

    fn space_available(&self) -> usize {
        self.space_available()
    }

    fn write_samples(&mut self, buffer: &[f32]) {
        self.fill_with(buffer);
    }

    fn flush(&mut self) {
        self.flush();
    }
}

#[derive(Clone)]
pub struct AudioMixer {
    sample_rate: usize,
    //buffer: CircularBuffer<f32>,
    buffer: Vec<f32>,
    sequence_num: usize,
}

impl AudioMixer {
    pub fn new(sample_rate: usize) -> HostData<AudioMixer> {
        HostData::new(AudioMixer {
            sample_rate,
            //buffer: CircularBuffer::new(1280 * 2, 0.0),
            buffer: vec![0.0; 1280 * 2],
            sequence_num: 0,
        })
    }

    pub fn new_default() -> HostData<AudioMixer> {
        AudioMixer::new(SAMPLE_RATE)
    }

    pub fn sample_rate(&self) -> usize {
        self.sample_rate
    }

    pub fn frame_size(&self) -> usize {
        self.buffer.len()
    }

    pub fn sequence_num(&self) -> usize {
        self.sequence_num
    }

    pub fn resize_frame(&mut self, newlen: usize) {
        if self.buffer.len() != newlen {
            self.buffer = vec![0.0; newlen];
        }
    }

    pub fn assembly_frame(&mut self, data: &mut [f32]) {
        self.resize_frame(data.len());
        for i in 0..data.len() {
            data[i] = Sample::from(&self.buffer[i]);
            self.buffer[i] = 0.0;
        }
        self.sequence_num = self.sequence_num.wrapping_add(1); 

/*
        let mut buffer = vec![0.0; data.len()];

        for source in &self.sources {
            let mut locked_source = source.lock();
            // TODO these are quick hacks to delay or shrink the buffer if it's too small or big
            if locked_source.used_space() < data.len() {
                continue;
            }
            let excess = locked_source.used_space() - (data.len() * 2);
            if excess > 0 {
                locked_source.drop_next(excess);
            }

            for addr in buffer.iter_mut() {
                *addr += locked_source.next().unwrap_or(0.0);
            }
        }

        for i in 0..data.len() {
            let sample = buffer[i] / self.sources.len() as f32;
            data[i] = Sample::from(&sample);
        }
*/

/*
        let mut locked_source = self.sources[1].lock();
        for i in 0..data.len() {
            let sample = locked_source.next().unwrap_or(0.0);
            data[i] = Sample::from(&sample);
        }
*/
    }

    // TODO you need a way to add data to the mixer... the question is do you need to keep track of real time
    // If you have a counter that calculates the amount of time until the next sample based on the size of
    // the buffer given to the data_callback, then when submitting data, the audio sources can know that they
    // the next place to write to is a given position in the mixer buffer (maybe not the start of the buffer).

    // But what do you do if there needs to be some skipping.  If the source is generating data in 1 to 10 ms
    // chunks according to simulated time, there might be a case where it tries to write too much data because
    // it's running fast. (If it's running slow, you can insert silence)
}

#[allow(dead_code)]
pub struct AudioOutput {
    stream: Stream,
    mixer: HostData<AudioMixer>,
}

impl AudioOutput {
    pub fn create_audio_output(mixer: HostData<AudioMixer>) -> AudioOutput {
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

        //let channels = config.channels as usize;

        let data_callback = {
            let mixer = mixer.clone();
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                mixer.lock().assembly_frame(data);

/*
                let mut locked_mixer = mixer.lock();
                //println!(">>> {} into {}", locked_mixer.buffer.used_space(), data.len());

                // TODO these are quick hacks to delay or shrink the buffer if it's too small or big
                if locked_mixer.buffer.used_space() < data.len() {
                    return;
                }
                if locked_mixer.buffer.used_space() > data.len() * 2 {
                    for _ in 0..(locked_mixer.buffer.used_space() - (data.len() * 2)) {
                        locked_mixer.buffer.next();
                    }
                }

                for addr in data.iter_mut() {
                    let sample = locked_mixer.buffer.next().unwrap_or(0.0);
                    *addr = Sample::from(&sample);
                }
                //locked_mixer.buffer.clear();
*/
            }
        };

        let stream = device.build_output_stream(
            &config,
            data_callback,
            move |err| {
                println!("ERROR: {:?}", err);
            },
        ).unwrap();

        stream.play().unwrap();

        AudioOutput {
            stream,
            mixer,
        }
    }


    /*
    pub fn create_audio_output2(mut updater: Box<dyn AudioUpdater>) -> AudioOutput {
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

        let channels = config.channels as usize;
        let mixer = AudioMixer::new(SAMPLE_RATE);

        let data_callback = {
            let mixer = mixer.clone();
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let samples = data.len() / 2;
                let mut buffer = vec![0.0; samples];
                updater.update_audio_frame(samples, mixer.lock().sample_rate(), &mut buffer);

                for (i, channels) in data.chunks_mut(2).enumerate() {
                    let sample = Sample::from(&buffer[i]);
                    channels[0] = sample;
                    channels[1] = sample;
                }
            }
        };

        let stream = device.build_output_stream(
            &config,
            data_callback,
            move |err| {
                // react to errors here.
                println!("ERROR");
            },
        ).unwrap();

        stream.play().unwrap();

        AudioOutput {
            stream,
            mixer,
        }
    }
    */
}

