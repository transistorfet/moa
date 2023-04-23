
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

use moa_core::{ClockTime, ClockDuration};
use moa_core::host::{Audio, ClockedQueue};


pub const SAMPLE_RATE: usize = 48000;

#[derive(Clone, Default)]
pub struct AudioFrame {
    pub data: Vec<(f32, f32)>,
}

pub struct AudioSource {
    id: usize,
    sample_rate: usize,
    frame_size: usize,
    mixer: Arc<Mutex<AudioMixer>>,
    queue: ClockedQueue<AudioFrame>,
}

impl AudioSource {
    pub fn new(mixer: Arc<Mutex<AudioMixer>>) -> Self {
        let queue = ClockedQueue::default();
        let (id, sample_rate, frame_size) = {
            let mut mixer = mixer.lock().unwrap();
            let id = mixer.add_source(queue.clone());
            (
                id,
                mixer.sample_rate(),
                mixer.frame_size(),
            )
        };

        Self {
            id,
            sample_rate,
            frame_size,
            mixer,
            queue,
        }
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn space_available(&self) -> usize {
        self.frame_size / 2
    }

    pub fn add_frame(&mut self, clock: ClockTime, buffer: &[f32]) {
        let mut data = vec![];
        for sample in buffer.iter() {
            // TODO this is here to keep it quiet for testing, but should be removed later
            let sample = 0.5 * *sample;
            data.push((sample, sample));
        }

        let frame = AudioFrame {
            data,
        };

        self.queue.push(clock, frame);
    }

    pub fn flush(&mut self) {
        self.mixer.lock().unwrap().check_next_frame();
    }
}


impl Audio for AudioSource {
    fn samples_per_second(&self) -> usize {
        self.sample_rate
    }

    fn space_available(&self) -> usize {
        self.space_available()
    }

    fn write_samples(&mut self, clock: ClockTime, buffer: &[f32]) {
        self.add_frame(clock, buffer);
        self.flush();
    }

    fn flush(&mut self) {
        self.mixer.lock().unwrap().check_next_frame();
    }
}

#[derive(Clone)]
pub struct AudioMixer {
    sample_rate: usize,
    frame_size: usize,
    sequence_num: usize,
    clock: ClockTime,
    sources: Vec<ClockedQueue<AudioFrame>>,
    buffer_underrun: bool,
    output: Arc<Mutex<AudioOutput>>,
}

impl AudioMixer {
    pub fn new(sample_rate: usize) -> Arc<Mutex<AudioMixer>> {
        Arc::new(Mutex::new(AudioMixer {
            sample_rate,
            frame_size: 1280,
            sequence_num: 0,
            clock: ClockTime::START,
            sources: vec![],
            buffer_underrun: false,
            output: AudioOutput::new(),
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

    pub fn sample_duration(&self) -> ClockDuration {
        ClockDuration::from_secs(1) / self.sample_rate as u64
    }

    pub fn frame_size(&self) -> usize {
        self.frame_size
    }

    pub fn sequence_num(&self) -> usize {
        self.sequence_num
    }

    pub fn resize_frame(&mut self, newlen: usize) {
        self.frame_size = newlen;
    }

    pub fn check_next_frame(&mut self) {
        if self.output.lock().unwrap().is_empty() {
            self.assemble_frame();
        }
    }

    pub fn assemble_frame(&mut self) {
        self.frame_size = self.output.lock().unwrap().frame_size;

        let sample_duration = self.sample_duration();
        let mut data: Vec<(f32, f32)> = vec![(0.0, 0.0); self.frame_size];

        if self.buffer_underrun {
            self.buffer_underrun = false;
            self.clock += sample_duration * data.len() as u64;
            let empty_frame = AudioFrame { data };
            self.output.lock().unwrap().add_frame(empty_frame.clone());
            self.output.lock().unwrap().add_frame(empty_frame);
            return;
        }

        let lowest_clock = self.sources
            .iter()
            .fold(self.clock, |lowest_clock, source|
                source
                    .peek_clock()
                    .map_or(lowest_clock, |c| c.min(lowest_clock)));
        self.clock = self.clock.min(lowest_clock);

        for source in &mut self.sources {
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

                let start = ((clock.duration_since(self.clock) / sample_duration) as usize).min(data.len() - 1);
                let length = frame.data.len().min(data.len() - start);

                data[start..start + length].iter_mut()
                    .zip(frame.data[..length].iter())
                    .for_each(|(d, s)|
                        *d = (
                            (d.0 + s.0).clamp(-1.0, 1.0),
                            (d.1 + s.1).clamp(-1.0, 1.0)
                        )
                    );
                if length < frame.data.len() {
                    let adjusted_clock = clock + sample_duration * length as u64;
                    //println!("unpopping at clock {}, length {}", adjusted_clock, frame.data.len() - length);
                    source.unpop(adjusted_clock, AudioFrame { data: frame.data[length..].to_vec() });
                }
                i = start + length;
            }
        }
        self.clock += sample_duration * data.len() as u64;

        self.output.lock().unwrap().add_frame(AudioFrame { data });
    }
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
            frame_size: 0,
            sequence_num: 0,
            last_frame: None,
            output: VecDeque::with_capacity(2),
        }))
    }

    pub fn set_frame_size(&mut self, frame_size: usize) {
        self.frame_size = frame_size
    }

    pub fn add_frame(&mut self, frame: AudioFrame) {
        self.output.push_back(frame);
        self.sequence_num = self.sequence_num.wrapping_add(1);
        //println!("added frame {}", self.sequence_num);
    }

    pub fn pop_next(&mut self) -> Option<AudioFrame> {
        //println!("frame {} sent", self.sequence_num);
        self.output.pop_front()
    }

    pub fn pop_latest(&mut self) -> Option<AudioFrame> {
        self.output.drain(..).last()
    }

    pub fn is_empty(&self) -> bool {
        self.output.is_empty()
    }
}

