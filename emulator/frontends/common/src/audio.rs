use std::sync::{Arc, Mutex, MutexGuard};
use femtos::{Instant, Duration};

use moa_host::{Audio, Sample, AudioFrame, ClockedQueue};


pub const SAMPLE_RATE: usize = 48000;

pub struct AudioSource {
    id: usize,
    sample_rate: usize,
    queue: ClockedQueue<AudioFrame>,
}

impl AudioSource {
    // TODO should you move this to AudioMixer to make the interface easier to use?
    // ie. let source: AudioSource = mixer.new_source();
    pub fn new(mixer: AudioMixer) -> Self {
        let queue = ClockedQueue::new(5000);
        let (id, sample_rate) = {
            let mut mixer = mixer.borrow_mut();
            let id = mixer.add_source(queue.clone());
            (id, mixer.sample_rate())
        };

        Self {
            id,
            sample_rate,
            queue,
        }
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn add_frame(&mut self, clock: Instant, buffer: &[Sample]) {
        let mut data = Vec::with_capacity(buffer.len());
        for sample in buffer.iter() {
            data.push(*sample);
        }

        let frame = AudioFrame::new(self.sample_rate, data);

        self.queue.push(clock, frame);
    }
}


impl Audio for AudioSource {
    fn samples_per_second(&self) -> usize {
        self.sample_rate
    }

    fn write_samples(&mut self, clock: Instant, buffer: &[Sample]) {
        self.add_frame(clock, buffer);
    }
}

#[derive(Clone)]
pub struct AudioMixer(Arc<Mutex<AudioMixerInner>>);

pub struct AudioMixerInner {
    sample_rate: usize,
    sources: Vec<ClockedQueue<AudioFrame>>,
    output: AudioOutput,
}

impl AudioMixer {
    pub fn new(sample_rate: usize) -> AudioMixer {
        AudioMixer(Arc::new(Mutex::new(AudioMixerInner {
            sample_rate,
            sources: vec![],
            output: AudioOutput::default(),
        })))
    }

    pub fn with_default_rate() -> AudioMixer {
        AudioMixer::new(SAMPLE_RATE)
    }

    pub fn borrow_mut(&self) -> MutexGuard<'_, AudioMixerInner> {
        self.0.lock().unwrap()
    }
}

impl AudioMixerInner {
    pub fn add_source(&mut self, source: ClockedQueue<AudioFrame>) -> usize {
        self.sources.push(source);
        self.sources.len() - 1
    }

    pub fn num_sources(&self) -> usize {
        self.sources.len()
    }

    pub fn get_sink(&mut self) -> AudioOutput {
        self.output.clone()
    }

    pub fn sample_rate(&self) -> usize {
        self.sample_rate
    }

    pub fn sample_duration(&self) -> Duration {
        Duration::from_secs(1) / self.sample_rate as u64
    }

    fn assemble_frame(&mut self, frame_start: Instant, frame_duration: Duration) {
        let sample_duration = self.sample_duration();
        let samples = (frame_duration / sample_duration) as usize;

        let mut data = vec![Sample(0.0, 0.0); samples];

        for source in &self.sources {
            let mut index = 0;
            while index < data.len() {
                if let Some((clock, mut frame)) = source.pop_next() {
                    index = (clock.duration_since(frame_start) / sample_duration) as usize;
                    let size = frame.data.len().min(data.len() - index);
                    frame
                        .data
                        .iter()
                        .zip(&mut data[index..index + size])
                        .for_each(|(source, dest)| {
                            dest.0 += source.0;
                            dest.1 += source.1;
                        });
                    index += size;
                    if size < frame.data.len() {
                        frame.data.drain(0..size);
                        source.put_back(clock, frame);
                    }
                }
            }
        }

        // Average each sample, and clamp it to the 1 to -1 range
        for sample in data.iter_mut() {
            sample.0 = (sample.0 / self.sources.len() as f32).clamp(-1.0, 1.0);
            sample.1 = (sample.1 / self.sources.len() as f32).clamp(-1.0, 1.0);
        }

        self.output.add_frame(frame_start, AudioFrame::new(self.sample_rate, data));
    }
}

use moa_core::{Transmutable, Steppable, Error, System};

impl Steppable for AudioMixer {
    fn step(&mut self, system: &System) -> Result<Duration, Error> {
        let duration = Duration::from_millis(1);
        // TODO should you make the clock be even further back to ensure the data is already written
        if let Some(start) = system.clock.checked_sub(duration) {
            self.borrow_mut().assemble_frame(start, duration);
        }
        Ok(duration)
    }
}

impl Transmutable for AudioMixer {
    fn as_steppable(&mut self) -> Option<&mut dyn Steppable> {
        Some(self)
    }
}


// TODO this should be split up into a sender/receiver
#[derive(Clone)]
pub struct AudioOutput {
    queue: ClockedQueue<AudioFrame>,
}

impl Default for AudioOutput {
    fn default() -> Self {
        Self {
            queue: ClockedQueue::new(5000),
        }
    }
}

impl AudioOutput {
    pub fn add_frame(&self, clock: Instant, frame: AudioFrame) {
        self.queue.push(clock, frame);
    }

    pub fn put_back(&self, clock: Instant, frame: AudioFrame) {
        self.queue.put_back(clock, frame);
    }

    pub fn receive(&self) -> Option<(Instant, AudioFrame)> {
        self.queue.pop_next()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}
