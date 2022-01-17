
use std::num::NonZeroU8;

use crate::error::Error;
use crate::system::System;
use crate::devices::{ClockElapsed, Address, Addressable, Steppable, Transmutable};
use crate::host::audio::{SineWave};
use crate::host::traits::{Host, Audio};

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
    pub multiplier: f32,
}

impl Operator {
    pub fn new(sample_rate: usize) -> Self {
        Self {
            wave: SineWave::new(400.0, sample_rate),
            multiplier: 1.0,
        }
    }

    pub fn set_frequency(&mut self, frequency: f32) {
        self.wave.frequency = frequency * self.multiplier;
    }

    pub fn set_multiplier(&mut self, frequency: f32, multiplier: f32) {
        self.multiplier = multiplier;
        self.set_frequency(frequency);
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

    pub fn get_sample(&mut self) -> f32 {
        match self.algorithm {
            OperatorAlgorithm::A0 => {
                let mut sample = 0.0;
                sample *= self.operators[0].wave.next().unwrap();
                sample *= self.operators[1].wave.next().unwrap();
                sample *= self.operators[2].wave.next().unwrap();
                sample *= self.operators[3].wave.next().unwrap();
                sample
            },
            OperatorAlgorithm::A1 => {
                let mut sample = self.operators[0].wave.next().unwrap() + self.operators[1].wave.next().unwrap();
                sample *= self.operators[2].wave.next().unwrap();
                sample *= self.operators[3].wave.next().unwrap();
                sample
            },
            OperatorAlgorithm::A2 => {
                let mut sample = self.operators[0].wave.next().unwrap() + (self.operators[1].wave.next().unwrap() * self.operators[2].wave.next().unwrap());
                sample *= self.operators[3].wave.next().unwrap();
                sample
            },
            OperatorAlgorithm::A3 => {
                let mut sample = (self.operators[0].wave.next().unwrap() * self.operators[1].wave.next().unwrap()) + self.operators[2].wave.next().unwrap();
                sample *= self.operators[3].wave.next().unwrap();
                sample
            },
            OperatorAlgorithm::A4 => {
                let sample1 = self.operators[0].wave.next().unwrap() * self.operators[1].wave.next().unwrap();
                let sample2 = self.operators[2].wave.next().unwrap() * self.operators[3].wave.next().unwrap();
                sample1 + sample2
            },
            OperatorAlgorithm::A5 => {
                let sample1 = self.operators[0].wave.next().unwrap();
                let sample2 = self.operators[1].wave.next().unwrap() + self.operators[2].wave.next().unwrap() + self.operators[3].wave.next().unwrap();
                sample1 * sample2
            },
            OperatorAlgorithm::A6 => {
                let sample1 = self.operators[0].wave.next().unwrap() * self.operators[1].wave.next().unwrap();
                sample1 + self.operators[2].wave.next().unwrap() + self.operators[3].wave.next().unwrap()
            },
            OperatorAlgorithm::A7 => {
                let mut sample = 0.0;
                sample += self.operators[0].wave.next().unwrap();
                sample += self.operators[1].wave.next().unwrap();
                sample += self.operators[2].wave.next().unwrap();
                sample += self.operators[3].wave.next().unwrap();
                sample / 4.0
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
        })
    }

    pub fn set_register(&mut self, bank: usize, reg: usize, data: u8) {
        match reg {
            0x28 => {
                let ch = (data as usize) & 0x07;
                self.channels[ch].on = data >> 4;
                println!("Note: {}: {:x}", ch, self.channels[ch].on);
            },
            0x30 => {
                let (ch, op) = get_ch_op(bank, reg);
                let multiplier = if data == 0 { 0.5 } else { (data & 0x0F) as f32 };
                let frequency = self.channels[ch].base_frequency;
                self.channels[ch].operators[op].set_multiplier(frequency, multiplier)
            },
            0xA4 | 0xA5 | 0xA6 => {
                let ch = (reg & 0x07) - 4 + (bank * 3);
                self.channel_frequencies[ch].1 = (self.channel_frequencies[ch].1 & 0xFF) | ((data as u16) & 0x07) << 8;
                self.channel_frequencies[ch].0 = (data & 0x1C) >> 3;
            },
            0xA0 | 0xA1 | 0xA2 => {
                let ch = (reg & 0x07) + (bank * 3);
                self.channel_frequencies[ch].1 = (self.channel_frequencies[ch].1 & 0xFF00) | data as u16;

                let frequency = fnumber_to_frequency(self.channel_frequencies[ch]);
                self.channels[ch].set_frequency(frequency);
            },
            0xB0 | 0xB1 | 0xB2 => {
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
            _ => warning!("{}: !!! unhandled write to register {:0x} with {:0x}", DEV_NAME, reg, data),
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
    fn step(&mut self, _system: &System) -> Result<ClockElapsed, Error> {
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

        let rate = self.source.samples_per_second() / 1000;
        let mut buffer = vec![0.0; rate];
        for i in 0..rate {
            let mut sample = 0.0;
            let mut count = 0;

            for ch in 0..7 {
                if self.channels[ch].on != 0 {
                    sample += self.channels[ch].get_sample();
                    count += 1;
                }
            }

            if count > 0 {
                buffer[i] = sample / count as f32;
            }
        }
        self.source.write_samples(&buffer);

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
                data[0] = 0;
            }
            _ => {
                warning!("{}: !!! unhandled read from {:0x}", DEV_NAME, addr);
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
                warning!("{}: !!! unhandled write {:0x} to {:0x}", DEV_NAME, data[0], addr);
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

