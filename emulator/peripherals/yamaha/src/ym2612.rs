
use std::num::NonZeroU8;
use std::collections::VecDeque;

use moa_core::{debug, warn};
use moa_core::{System, Error, ClockElapsed, Address, Addressable, Steppable, Transmutable};
use moa_core::host::{Host, Audio};
use moa_core::host::audio::{SineWave};

const DEV_NAME: &'static str = "ym2612";

const CHANNELS: usize = 8;

#[derive(Copy, Clone, Debug)]
pub enum OperatorAlgorithm {
    A0,
    A1,
    A2,
    A3,
    A4,
    A5,
    A6,
    A7,
}


#[derive(Clone)]
pub struct Operator {
    pub wave: SineWave,
    pub frequency: f32,
    pub multiplier: f32,
}

impl Operator {
    pub fn new(sample_rate: usize) -> Self {
        Self {
            wave: SineWave::new(400.0, sample_rate),
            frequency: 400.0,
            multiplier: 1.0,
        }
    }

    pub fn set_frequency(&mut self, frequency: f32) {
        self.frequency = frequency;
    }

    pub fn reset(&mut self) {
        self.wave.reset();
    }

    pub fn set_multiplier(&mut self, _frequency: f32, multiplier: f32) {
        self.multiplier = multiplier;
    }

    pub fn get_sample(&mut self, modulator: f32) -> f32 {
        // TODO this would need to take into account the volume and envelope
        self.wave.set_frequency((self.frequency * self.multiplier) + modulator);
        self.wave.next().unwrap()
    }
}

#[derive(Clone)]
pub struct Channel {
    pub operators: Vec<Operator>,
    pub on: u8,
    pub base_frequency: f32,
    pub algorithm: OperatorAlgorithm,
}

impl Channel {
    pub fn new(sample_rate: usize) -> Self {
        Self {
            operators: vec![Operator::new(sample_rate); 4],
            on: 0,
            base_frequency: 0.0,
            algorithm: OperatorAlgorithm::A0,
        }
    }

    pub fn set_frequency(&mut self, frequency: f32) {
        self.base_frequency = frequency;
        for operator in self.operators.iter_mut() {
            operator.set_frequency(frequency);
        }
    }

    pub fn reset(&mut self) {
        for operator in self.operators.iter_mut() {
            operator.reset();
        }
    }

    pub fn get_sample(&mut self) -> f32 {
        match self.algorithm {
            OperatorAlgorithm::A0 => {
                let modulator0 = self.operators[0].get_sample(0.0);
                let modulator1 = self.operators[1].get_sample(modulator0);
                let modulator2 = self.operators[2].get_sample(modulator1);
                self.operators[3].get_sample(modulator2)
            },
            OperatorAlgorithm::A1 => {
                let sample1 = self.operators[0].get_sample(0.0) + self.operators[1].get_sample(0.0);
                let sample2 = self.operators[2].get_sample(sample1);
                let sample3 = self.operators[3].get_sample(sample2);
                sample3
            },
            OperatorAlgorithm::A2 => {
                let sample1 = self.operators[1].get_sample(0.0);
                let sample2 = self.operators[2].get_sample(sample1);
                let sample3 = self.operators[0].get_sample(0.0) + sample2;
                let sample4 = self.operators[3].get_sample(sample3);
                sample4
            },
            OperatorAlgorithm::A3 => {
                let sample1 = self.operators[0].get_sample(0.0);
                let sample2 = self.operators[1].get_sample(sample1);
                let sample3 = self.operators[2].get_sample(0.0);
                let sample4 = self.operators[3].get_sample(sample2 + sample3);
                sample4
            },
            OperatorAlgorithm::A4 => {
                let sample1 = self.operators[0].get_sample(0.0);
                let sample2 = self.operators[1].get_sample(sample1);
                let sample3 = self.operators[2].get_sample(0.0);
                let sample4 = self.operators[3].get_sample(sample3);
                sample2 + sample4
            },
            OperatorAlgorithm::A5 => {
                let sample1 = self.operators[0].get_sample(0.0);
                let sample2 = self.operators[1].get_sample(sample1) + self.operators[2].get_sample(sample1) + self.operators[3].get_sample(sample1);
                sample2
            },
            OperatorAlgorithm::A6 => {
                let sample1 = self.operators[0].get_sample(0.0);
                let sample2 = self.operators[1].get_sample(sample1);
                sample2 + self.operators[2].get_sample(0.0) + self.operators[3].get_sample(0.0)
            },
            OperatorAlgorithm::A7 => {
                let sample = self.operators[0].get_sample(0.0)
                + self.operators[1].get_sample(0.0)
                + self.operators[2].get_sample(0.0)
                + self.operators[3].get_sample(0.0);
                sample
            },
        }
    }
}



pub struct Ym2612 {
    pub source: Box<dyn Audio>,
    pub selected_reg_0: Option<NonZeroU8>,
    pub selected_reg_1: Option<NonZeroU8>,

    pub channels: Vec<Channel>,
    pub channel_frequencies: [(u8, u16); CHANNELS],

    pub dac_enabled: bool,
    pub dac: VecDeque<u8>,

    pub timer_a_enable: bool,
    pub timer_a: u16,
    pub timer_a_current: u16,
    pub timer_a_overflow: bool,

    pub timer_b_enable: bool,
    pub timer_b: u8,
    pub timer_b_current: u8,
    pub timer_b_overflow: bool,
}

impl Ym2612 {
    pub fn create<H: Host>(host: &mut H) -> Result<Self, Error> {
        let source = host.create_audio_source()?;
        let sample_rate = source.samples_per_second();
        Ok(Self {
            source,
            selected_reg_0: None,
            selected_reg_1: None,

            channels: vec![Channel::new(sample_rate); 8],
            channel_frequencies: [(0, 0); CHANNELS],

            dac_enabled: false,
            dac: VecDeque::with_capacity(100),

            timer_a_enable: false,
            timer_a: 0,
            timer_a_current: 0,
            timer_a_overflow: false,

            timer_b_enable: false,
            timer_b: 0,
            timer_b_current: 0,
            timer_b_overflow: false,
        })
    }

    pub fn set_register(&mut self, bank: usize, reg: usize, data: u8) {
        //warn!("{}: set reg {}{:x} to {:x}", DEV_NAME, bank, reg, data);
        match reg {
            0x24 => {
                self.timer_a = (self.timer_a & 0x3) | ((data as u16) << 2);
            },
            0x25 => {
                self.timer_a = (self.timer_a & 0xFFFC) | ((data as u16) & 0x03);
            },
            0x26 => {
                self.timer_b = data;
            },
            0x27 => {
                //if (data >> 5) & 0x1 {
                //    self.timer_b
            },

            0x28 => {
                let ch = (data as usize) & 0x07;
                self.channels[ch].on = data >> 4;
                self.channels[ch].reset();
            },

            0x2a => {
                if self.dac_enabled {
                    for _ in 0..3 {
                        self.dac.push_back(data);
                    }
                }
            },

            0x2b => {
                self.dac_enabled = data & 0x80 != 0;
            },

            reg if (reg & 0xF0) == 0x30 => {
                let (ch, op) = get_ch_op(bank, reg);
                let multiplier = if data == 0 { 0.5 } else { (data & 0x0F) as f32 };
                let frequency = self.channels[ch].base_frequency;
                debug!("{}: channel {} operator {} set to multiplier {}", DEV_NAME, ch + 1, op + 1, multiplier);
                self.channels[ch].operators[op].set_multiplier(frequency, multiplier)
            },

            reg if reg >= 0xA0 && reg <= 0xA2 => {
                let ch = (reg & 0x07) + (bank * 3);
                self.channel_frequencies[ch].1 = (self.channel_frequencies[ch].1 & 0xFF00) | data as u16;

                let frequency = fnumber_to_frequency(self.channel_frequencies[ch]);
                debug!("{}: channel {} set to frequency {}", DEV_NAME, ch + 1, frequency);
                self.channels[ch].set_frequency(frequency);
            },

            reg if reg >= 0xA4 && reg <= 0xA6 => {
                let ch = (reg & 0x07) - 4 + (bank * 3);
                self.channel_frequencies[ch].1 = (self.channel_frequencies[ch].1 & 0xFF) | ((data as u16) & 0x07) << 8;
                self.channel_frequencies[ch].0 = (data & 0x38) >> 3;
            },

            reg if reg >= 0xB0 && reg <= 0xB2 => {
                let ch = (reg & 0x07) + (bank * 3);
                self.channels[ch].algorithm = match data & 0x07 {
                    0 => OperatorAlgorithm::A0,
                    1 => OperatorAlgorithm::A1,
                    2 => OperatorAlgorithm::A2,
                    3 => OperatorAlgorithm::A3,
                    4 => OperatorAlgorithm::A4,
                    5 => OperatorAlgorithm::A5,
                    6 => OperatorAlgorithm::A6,
                    7 => OperatorAlgorithm::A7,
                    _ => OperatorAlgorithm::A0,
                };
            },

            _ => {
                warn!("{}: !!! unhandled write to register {:0x} with {:0x}", DEV_NAME, reg, data);
            },
        }
    }
}

#[inline(always)]
pub fn fnumber_to_frequency(fnumber: (u8, u16)) -> f32 {
    (fnumber.1 as f32 * 0.0264) * (2 as u32).pow(fnumber.0 as u32) as f32
}

#[inline(always)]
pub fn get_ch_op(bank: usize, reg: usize) -> (usize, usize) {
    let ch = (reg & 0x03) + (bank * 3);
    let op = (reg & 0xC0) >> 2;
    (ch, op)
}


impl Steppable for Ym2612 {
    fn step(&mut self, system: &System) -> Result<ClockElapsed, Error> {
        // TODO since you expect this step function to be called every 1ms of simulated time
        //      you could assume that you should produce (sample_rate / 1000) samples

        //if self.sine.frequency < 2000.0 { 
        //    self.sine.frequency += 1.0;
        //}

        //let rate = self.source.samples_per_second();
        //self.source.write_samples(rate / 1000, &mut self.sine);
        //println!("{}", self.sine.frequency);

        //if self.on {
        //    let rate = self.source.samples_per_second();
        //    self.source.write_samples(rate / 1000, &mut self.sine);
        //}

        let rate = self.source.samples_per_second();
        let available = self.source.space_available();
        let samples = if available < rate / 1000 { available } else { rate / 1000 };
        //if self.source.space_available() >= samples {
            let mut buffer = vec![0.0; samples];
            for i in 0..samples {
                let mut sample = 0.0;

                for ch in 0..6 {
                    if self.channels[ch].on != 0 {
                        sample += self.channels[ch].get_sample();
                    }
                }

                if self.dac_enabled {
                    if let Some(data) = self.dac.pop_front() {
                        sample += ((data as f32 - 128.0) / 255.0) * 2.0;
                    }
                } else if self.channels[6].on != 0 {
                    sample += self.channels[6].get_sample();
                }

                buffer[i] = sample.clamp(-1.0, 1.0);
            }
//println!("synthesized: {:?}", buffer);
            self.source.write_samples(system.clock, &buffer);
        //}

        Ok(1_000_000)          // Every 1ms of simulated time
    }
}

impl Addressable for Ym2612 {
    fn len(&self) -> usize {
        0x04
    }

    fn read(&mut self, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        match addr {
            0 | 1 | 2 | 3 => {
                // Read the status byte (busy/overflow)
                data[0] = 0 | ((self.timer_a_overflow as u8) << 1) | (self.timer_b_overflow as u8);
            }
            _ => {
                warn!("{}: !!! unhandled read from {:0x}", DEV_NAME, addr);
            },
        }
        debug!("{}: read from register {:x} of {:?}", DEV_NAME, addr, data);
        Ok(())
    }

    fn write(&mut self, addr: Address, data: &[u8]) -> Result<(), Error> {
        debug!("{}: write to register {:x} with {:x}", DEV_NAME, addr, data[0]);
        match addr {
            0 => {
                self.selected_reg_0 = NonZeroU8::new(data[0]);
            },
            1 => {
                if let Some(reg) = self.selected_reg_0 {
                    self.set_register(0, reg.get() as usize, data[0]);
                }
            },
            2 => {
                self.selected_reg_1 = NonZeroU8::new(data[0]);
            },
            3 => {
                if let Some(reg) = self.selected_reg_1 {
                    self.set_register(1, reg.get() as usize, data[0]);
                }
            },
            _ => {
                warn!("{}: !!! unhandled write {:0x} to {:0x}", DEV_NAME, data[0], addr);
            },
        }
        Ok(())
    }
}

impl Transmutable for Ym2612 {
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        Some(self)
    }

    fn as_steppable(&mut self) -> Option<&mut dyn Steppable> {
        Some(self)
    }
}

