
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use cpal::{Sample, Stream, SampleRate, SampleFormat, StreamConfig, traits::{DeviceTrait, HostTrait, StreamTrait}};

use moa_core::Clock;
use moa_core::host::{HostData, Audio, ClockedQueue};
use crate::circularbuf::CircularBuffer;

const SAMPLE_RATE: usize = 48000;


#[derive(Clone)]
pub struct AudioFrame {
    data: Vec<f32>,
}

pub struct AudioSource {
    id: usize,
    sample_rate: usize,
    frame_size: usize,
    sequence_num: usize,
    mixer: Arc<Mutex<AudioMixer>>,
    buffer: CircularBuffer<f32>,
    queue: ClockedQueue<AudioFrame>,
}

impl AudioSource {
    pub fn new(mixer: Arc<Mutex<AudioMixer>>) -> Self {
        let queue = ClockedQueue::new();
        let (id, sample_rate, frame_size) = {
            let mut mixer = mixer.lock().unwrap();
            let id = mixer.add_source(queue.clone());
            (
                id,
                mixer.sample_rate(),
                mixer.frame_size(),
            )
        };
        let buffer = CircularBuffer::new(frame_size * 2, 0.0);

        Self {
            id,
            sample_rate,
            frame_size,
            sequence_num: 0,
            mixer,
            buffer,
            queue,
        }
    }

    pub fn space_available(&self) -> usize {
        //self.buffer.free_space() / 2
        self.frame_size / 2
    }

    pub fn fill_with(&mut self, clock: Clock, buffer: &[f32]) {
        let mut data = vec![];
        //if self.buffer.free_space() > buffer.len() * 2 {
            for sample in buffer.iter() {
                // TODO this is here to keep it quiet for testing, but should be removed later
                let sample = 0.5 * *sample;
                data.push(sample);
                data.push(sample);
            }
        //}

        let frame = AudioFrame {
            data, //: Vec::from(buffer)
        };

//println!("synthesized {}: {:?}", self.id, frame.data);

        self.queue.push(clock, frame);
        self.flush();
    }

    pub fn flush(&mut self) {
        self.mixer.lock().unwrap().check_next_frame();
    }


    /*
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
                println!("repeated seq");
                return;
            }
            self.sequence_num = mixer_sequence_num;
            println!("flushing to audio mixer {}", self.sequence_num);

            //for i in 0..locked_mixer.buffer.len() {
            //    locked_mixer.buffer[i] = (locked_mixer.buffer[i] + self.buffer.next().unwrap_or(0.0)).clamp(-1.0, 1.0);
            //}
            self.queue.push(0, AudioFrame { data: (0..self.frame_size).map(|_| self.buffer.next().unwrap()).collect() });

            self.frame_size = locked_mixer.frame_size();
            self.buffer.resize(self.frame_size * 2);
        }
    }
    */
}

// could have the audio source use the circular buffer and then publish to its queue, and then call the mixer to flush if possible,
// and have the mixer (from the sim thread effectively) build the frame and publish it to its output.  Frames in the source queues
// could even be 1ms, and the assembler could just fetch multiple frames, adjusting for sim time


// you could either only use the circular buffer, or only use the source queue

impl Audio for AudioSource {
    fn samples_per_second(&self) -> usize {
        self.sample_rate
    }

    fn space_available(&self) -> usize {
        self.space_available()
    }

    fn write_samples(&mut self, clock: Clock, buffer: &[f32]) {
        self.fill_with(clock, buffer);
    }

    fn flush(&mut self) {
        self.flush();
    }
}

use moa_core::host::audio::SquareWave;

#[derive(Clone)]
pub struct AudioMixer {
    sample_rate: usize,
    frame_size: usize,
    sequence_num: usize,
    clock: Clock,
    sources: Vec<ClockedQueue<AudioFrame>>,
    buffer_underrun: bool,
    output: Arc<Mutex<AudioOutput>>,
    test: SquareWave,
}

impl AudioMixer {
    pub fn new(sample_rate: usize) -> Arc<Mutex<AudioMixer>> {
        Arc::new(Mutex::new(AudioMixer {
            sample_rate,
            frame_size: 1280,
            sequence_num: 0,
            clock: 0,
            sources: vec![],
            buffer_underrun: false,
            output: AudioOutput::new(),
            test: SquareWave::new(600.0, sample_rate),
        }))
    }

    pub fn with_default_rate() -> Arc<Mutex<AudioMixer>> {
        AudioMixer::new(SAMPLE_RATE)
    }

    pub fn add_source(&mut self, source: ClockedQueue<AudioFrame>) -> usize {
        self.sources.push(source);
        self.sources.len() - 1
    }

    pub fn get_sink(&mut self) -> Arc<Mutex<AudioOutput>> {
        self.output.clone()
    }

    pub fn sample_rate(&self) -> usize {
        self.sample_rate
    }

    pub fn nanos_per_sample(&self) -> Clock {
        1_000_000_000 as Clock / self.sample_rate as Clock
    }

    pub fn frame_size(&self) -> usize {
        //self.buffer.len()
        self.frame_size
    }

    pub fn sequence_num(&self) -> usize {
        self.sequence_num
    }

    pub fn resize_frame(&mut self, newlen: usize) {
        //if self.buffer.len() != newlen {
        //    self.buffer = vec![0.0; newlen];
        //}
        self.frame_size = newlen;
    }

    pub fn check_next_frame(&mut self) {
        if self.output.lock().unwrap().is_empty() {
            self.assemble_frame();
        }
    }

    pub fn assemble_frame(&mut self) {
        self.frame_size = self.output.lock().unwrap().frame_size;

        let nanos_per_sample = self.nanos_per_sample();
        let mut data: Vec<f32> = vec![0.0; self.frame_size];

        if self.buffer_underrun {
            self.buffer_underrun = false;
            self.clock += nanos_per_sample * data.len() as Clock;
            let empty_frame = AudioFrame { data };
            self.output.lock().unwrap().add_frame(empty_frame.clone());
            self.output.lock().unwrap().add_frame(empty_frame);
            return;
        }

        /*
        for i in (0..data.len()).step_by(2) {
            let sample = self.test.next().unwrap() * 0.5;
            data[i] = sample;
            data[i + 1] = sample;
        }
        */

        let lowest_clock = self.sources
            .iter()
            .fold(self.clock, |lowest_clock, source|
                source
                    .peek_clock()
                    .map_or(lowest_clock, |c| c.min(lowest_clock)));
        self.clock = self.clock.min(lowest_clock);

        for (id, source) in self.sources.iter_mut().enumerate() {
            let mut i = 0;
            while i < data.len() {
                let (clock, frame) = match source.pop_next() {
                    Some(frame) => frame,
                    None => {
                        println!("buffer underrun");
                        self.buffer_underrun = true;
                        break;
                    },
                };

//println!("clock: {} - {} = {}", clock, self.clock, clock - self.clock);
                //if clock > self.clock {
                let start = ((2 * (clock - self.clock) / nanos_per_sample) as usize).min(data.len() - 1);
                let length = frame.data.len().min(data.len() - start);
//println!("source: {}, clock: {}, start: {}, end: {}, length: {}", id, clock, start, start + length, length);
                data[start..start + length].iter_mut().zip(frame.data[..length].iter()).for_each(|(d, s)| *d = (*d + s).clamp(-1.0, 1.0));
                if length < frame.data.len() {
                    let adjusted_clock = clock + nanos_per_sample * (length / 2) as Clock;
                    //println!("unpopping {} {}", clock, adjusted_clock);
                    source.unpop(adjusted_clock, AudioFrame { data: frame.data[length..].to_vec() });
                }
                //}
                // TODO we need to handle the opposite case
                i += length;
            }
        }
        self.clock += nanos_per_sample * data.len() as Clock;

//println!("{:?}", data);
        self.output.lock().unwrap().add_frame(AudioFrame { data });
    }

/*
    pub fn assembly_frame(&mut self, data: &mut [f32]) {
        self.resize_frame(data.len());
        println!("assemble audio frame {}", self.sequence_num);

        //for i in 0..data.len() {
        //    data[i] = Sample::from(&self.buffer[i]);
        //    self.buffer[i] = 0.0;
        //}

        //self.sources
        //    .iter()
        //    .filter_map(|queue| queue.pop_latest())
        //    .fold(data, |data, frame| {
        //        data.iter_mut()
        //            .zip(frame.1.data.iter())
        //            .for_each(|(d, s)| *d = (*d + s).clamp(-1.0, 1.0));
        //        data
        //    });

        if let Some((_, last)) = self.output.pop_latest() {
            self.last_frame = Some(last);
        }
        if let Some(last) = &self.last_frame {
            data.copy_from_slice(&last.data);
        }

        println!("frame {} sent", self.sequence_num);
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
*/
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
    frame_size: usize,
    sequence_num: usize,
    last_frame: Option<AudioFrame>,
    output: VecDeque<AudioFrame>,
}

impl AudioOutput {
    pub fn new() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            frame_size: 1280,
            sequence_num: 0,
            last_frame: None,
            output: VecDeque::with_capacity(2),
        }))
    }

    pub fn add_frame(&mut self, frame: AudioFrame) {
        self.output.push_back(frame);
        self.sequence_num = self.sequence_num.wrapping_add(1);
        println!("added frame {}", self.sequence_num);
    }

    pub fn pop_next(&mut self) -> Option<AudioFrame> {
        println!("frame {} sent", self.sequence_num);
        self.output.pop_front()
    }

    pub fn pop_latest(&mut self) -> Option<AudioFrame> {
        self.output.drain(..).last()
    }

    pub fn is_empty(&self) -> bool {
        self.output.is_empty()
    }
}


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
                output.frame_size = data.len();
                output.pop_next()
            } else {
                return;
            };

            if let Some(frame) = result {
//println!("needs {}, gets {}", data.len(), frame.data.len());
//println!("{:?}", frame.data);
                let length = frame.data.len().min(data.len());
                data[..length].copy_from_slice(&frame.data[..length]);
            } else {
                println!("missed a frame");
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

        CpalAudioOutput {
            stream,
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

