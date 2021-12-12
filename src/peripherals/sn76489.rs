
use crate::error::Error;
use crate::system::System;
use crate::devices::{ClockElapsed, Address, Addressable, Steppable, Transmutable};
use crate::host::audio::{SineWave, SquareWave};
use crate::host::traits::{Host, Audio};


const DEV_NAME: &'static str = "sn76489";

/*
pub struct Sn76489Updater(HostData<SineWave>);

impl AudioUpdater for Sn76489Updater {
    fn update_audio_frame(&mut self, samples: usize, sample_rate: usize, buffer: &mut [f32]) {
        let mut sine = self.0.lock();
        //for i in 0..samples {
        //    buffer[i] = sine.next().unwrap();
        //}
    }
}
*/


pub struct Sn76489 {
    pub regs: [u8; 8],
    pub first_byte: Option<u8>,
    pub source: Box<dyn Audio>,
    pub sine: SquareWave,
}

impl Sn76489 {
    pub fn create<H: Host>(host: &mut H) -> Result<Self, Error> {
        let source = host.create_audio_source()?;
        let sine = SquareWave::new(600.0, source.samples_per_second());

        Ok(Self {
            regs: [0; 8],
            first_byte: None,
            source,
            sine,
        })
    }
}

impl Steppable for Sn76489 {
    fn step(&mut self, _system: &System) -> Result<ClockElapsed, Error> {
        // TODO since you expect this step function to be called every 1ms of simulated time
        //      you could assume that you should produce (sample_rate / 1000) samples

        if self.sine.frequency > 200.0 { 
            self.sine.frequency -= 1.0;
        }

        let rate = self.source.samples_per_second();
        self.source.write_samples(rate / 1000, &mut self.sine);
        //println!("{}", self.sine.frequency);
        Ok(1_000_000)          // Every 1ms of simulated time
    }
}

impl Addressable for Sn76489 {
    fn len(&self) -> usize {
        0x01
    }

    fn read(&mut self, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        warning!("{}: !!! device can't be read", DEV_NAME);
        Ok(())
    }

    fn write(&mut self, addr: Address, data: &[u8]) -> Result<(), Error> {
        if addr != 0 {
            warning!("{}: !!! unhandled write {:0x} to {:0x}", DEV_NAME, data[0], addr);
            return Ok(());
        }

        if (data[0] & 0x80) == 0 {
            // TODO update noise byte
        } else {
            let reg = (data[0] & 0x70) >> 4;
            if reg == 6 {
                self.first_byte = Some(data[0]);
            } else {
                self.regs[reg as usize] = data[0] & 0x0F;
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


