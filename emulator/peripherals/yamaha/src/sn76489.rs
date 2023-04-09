
use moa_core::{info, warn, debug};
use moa_core::{System, Error, ClockElapsed, Address, Addressable, Steppable, Transmutable};
use moa_core::host::{Host, Audio};
use moa_core::host::audio::{SquareWave};


const DEV_NAME: &str = "sn76489";

#[derive(Clone)]
struct ToneGenerator {
    on: bool,
    attenuation: f32,
    wave: SquareWave,
}

impl ToneGenerator {
    fn new(sample_rate: usize) -> Self {
        Self {
            on: false,
            attenuation: 0.0,
            wave: SquareWave::new(600.0, sample_rate),
        }
    }

    fn set_attenuation(&mut self, attenuation: u8) {
        if attenuation == 0x0F {
            self.on = false;
        } else {
            self.on = true;
            self.attenuation = (attenuation << 1) as f32;
        }
        info!("set attenuation to {} {}", self.attenuation, self.on);
    }

    fn set_counter(&mut self, count: usize) {
        let frequency = 3_579_545.0 / (count as f32 * 32.0);
        self.wave.set_frequency(frequency);
        info!("set frequency to {}", frequency);
    }

    fn get_sample(&mut self) -> f32 {
        self.wave.next().unwrap() / (self.attenuation + 1.0)
    }
}


#[derive(Clone)]
struct NoiseGenerator {
    on: bool,
    attenuation: f32,
}

impl Default for NoiseGenerator {
    fn default() -> Self {
        Self {
            on: false,
            attenuation: 0.0,
        }
    }
}

impl NoiseGenerator {
    fn set_attenuation(&mut self, attenuation: u8) {
        if attenuation == 0x0F {
            self.on = false;
        } else {
            self.on = true;
            self.attenuation = (attenuation << 1) as f32;
        }
        info!("set attenuation to {} {}", self.attenuation, self.on);
    }

    fn set_control(&mut self, _bits: u8) {
        //let frequency = 3_579_545.0 / (count as f32 * 32.0);
        //self.wave.set_frequency(frequency);
        //debug!("set frequency to {}", frequency);
    }

    fn get_sample(&mut self) -> f32 {
        // TODO this isn't implemented yet
        0.0
    }
}



pub struct Sn76489 {
    clock_frequency: u32,
    first_byte: Option<u8>,
    source: Box<dyn Audio>,
    tones: Vec<ToneGenerator>,
    noise: NoiseGenerator,
}

impl Sn76489 {
    pub fn create<H: Host>(host: &mut H, clock_frequency: u32) -> Result<Self, Error> {
        let source = host.create_audio_source()?;
        let sample_rate = source.samples_per_second();

        Ok(Self {
            clock_frequency,
            first_byte: None,
            source,
            tones: vec![ToneGenerator::new(sample_rate); 3],
            noise: NoiseGenerator::default(),
        })
    }
}

impl Steppable for Sn76489 {
    fn step(&mut self, system: &System) -> Result<ClockElapsed, Error> {
        let rate = self.source.samples_per_second();
        let available = self.source.space_available();
        let samples = if available < rate / 1000 { available } else { rate / 1000 };

        if samples > 0 {
        //if available >= rate / 1000 {
            let mut buffer = vec![0.0; samples];
            for buffered_sample in buffer.iter_mut().take(samples) {
                let mut sample = 0.0;

                for ch in 0..3 {
                    if self.tones[ch].on {
                        sample += self.tones[ch].get_sample();
                    }
                }

                if self.noise.on {
                    sample += self.noise.get_sample();
                }

                *buffered_sample = sample.clamp(-1.0, 1.0);
            }
            self.source.write_samples(system.clock, &buffer);
        } else {
            self.source.flush();
        }

        Ok(1_000_000)          // Every 1ms of simulated time
    }
}

impl Addressable for Sn76489 {
    fn len(&self) -> usize {
        0x01
    }

    fn read(&mut self, _addr: Address, _data: &mut [u8]) -> Result<(), Error> {
        warn!("{}: !!! device can't be read", DEV_NAME);
        Ok(())
    }

    fn write(&mut self, addr: Address, data: &[u8]) -> Result<(), Error> {
        if addr != 0 {
            warn!("{}: !!! unhandled write {:0x} to {:0x}", DEV_NAME, data[0], addr);
            return Ok(());
        }

        if (data[0] & 0x80) != 0 {
            let reg = (data[0] & 0x70) >> 4;
            let value = data[0] & 0x0F;
            match reg {
                1 => self.tones[0].set_attenuation(value),
                3 => self.tones[1].set_attenuation(value),
                5 => self.tones[2].set_attenuation(value),
                6 => self.noise.set_control(value),
                7 => self.noise.set_attenuation(value),
                _ => { self.first_byte = Some(data[0]); },
            }
        } else {
            let first = self.first_byte.unwrap_or(0);
            let reg = (first & 0x70) >> 4;
            let value = ((data[0] as usize & 0x3F) << 4) | (first as usize & 0x0F);
            match reg {
                0 => self.tones[0].set_counter(value),
                2 => self.tones[1].set_counter(value),
                4 => self.tones[2].set_counter(value),
                _ => { },
            }
        }
        debug!("{}: write to register {:x} with {:x}", DEV_NAME, addr, data[0]);
        Ok(())
    }
}

impl Transmutable for Sn76489 {
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        Some(self)
    }

    fn as_steppable(&mut self) -> Option<&mut dyn Steppable> {
        Some(self)
    }
}

