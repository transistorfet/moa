
use std::num::NonZeroU8;

use crate::error::Error;
use crate::system::System;
use crate::devices::{ClockElapsed, Address, Addressable, Steppable, Transmutable};
use crate::host::audio::{SquareWave};
use crate::host::traits::{Host, Audio};

const DEV_NAME: &'static str = "ym2612";

#[derive(Clone)]
pub struct Operator {
    pub wave: SquareWave,
}

impl Operator {
    pub fn new(sample_rate: usize) -> Self {
        Self {
            wave: SquareWave::new(400.0, sample_rate)
        }
    }
}

#[derive(Clone)]
pub struct Channel {
    pub operators: Vec<Operator>,
    pub on: u8,
}

impl Channel {
    pub fn new(sample_rate: usize) -> Self {
        Self {
            operators: vec![Operator::new(sample_rate); 4],
            on: 0,
        }
    }
}



pub struct Ym2612 {
    pub source: Box<dyn Audio>,
    pub selected_reg: Option<NonZeroU8>,

    pub channels: Vec<Channel>,
}

impl Ym2612 {
    pub fn create<H: Host>(host: &mut H) -> Result<Self, Error> {
        let source = host.create_audio_source()?;
        let sample_rate = source.samples_per_second();
        Ok(Self {
            source,
            selected_reg: None,
            channels: vec![Channel::new(sample_rate); 7],
        })
    }

    pub fn set_register(&mut self, bank: u8, reg: usize, data: u8) {
        match reg {
            0x28 => {
                let ch = (data as usize) & 0x07;
                self.channels[ch].on = data >> 4;
                println!("Note: {}: {:x}", ch, self.channels[ch].on);
            },
            0x30 => {
                let _op = if bank == 0 { 0 } else { 3 };
            }
            _ => warning!("{}: !!! unhandled write to register {:0x} with {:0x}", DEV_NAME, reg, data),
        }
    }
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
                self.selected_reg = NonZeroU8::new(data[0]);
            },
            1 => {
                match self.selected_reg {
                    None => {},
                    Some(reg) => self.set_register(0, reg.get() as usize, data[0]),
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

