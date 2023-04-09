
use std::num::NonZeroU8;
use std::collections::VecDeque;

use moa_core::{debug, warn};
use moa_core::{System, Error, Clock, ClockElapsed, Address, Addressable, Steppable, Transmutable};
use moa_core::host::{Host, Audio};
use moa_core::host::audio::{SineWave, db_to_gain};

const DEV_NAME: &str = "ym2612";

const CHANNELS: usize = 8;

#[derive(Copy, Clone, Debug)]
enum EnvelopeState {
    Attack,
    Decay1,
    Decay2,
    Release,
}

#[derive(Copy, Clone, Debug)]
enum OperatorAlgorithm {
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
struct Operator {
    wave: SineWave,
    frequency: f32,
    multiplier: f32,
    total_level: f32,

    attack_rate: usize,
    first_decay_rate: usize,
    first_decay_level: usize,
    second_decay_rate: usize,
    release_rate: usize,

    envelope_state: EnvelopeState,
    last_event: Clock,
    envelope_gain: f32,
}

impl Operator {
    fn new(sample_rate: usize) -> Self {
        Self {
            wave: SineWave::new(400.0, sample_rate),
            frequency: 400.0,
            multiplier: 1.0,
            total_level: 0.0,

            attack_rate: 0,
            first_decay_rate: 0,
            first_decay_level: 0,
            second_decay_rate: 0,
            release_rate: 0,

            envelope_state: EnvelopeState::Attack,
            last_event: 0,
            envelope_gain: 0.0,
        }
    }

    fn set_frequency(&mut self, frequency: f32) {
        self.frequency = frequency;
    }

    fn set_total_level(&mut self, db: f32) {
        self.total_level = db_to_gain(db);
    }

    fn set_attack_rate(&mut self, rate: usize) {
        self.attack_rate = rate;
    }

    fn set_first_decay_rate(&mut self, rate: usize) {
        self.first_decay_rate = rate;
    }

    fn set_first_decay_level(&mut self, rate: usize) {
        self.first_decay_level = rate;
    }

    fn set_second_decay_rate(&mut self, rate: usize) {
        self.second_decay_rate = rate;
    }

    fn set_release_rate(&mut self, rate: usize) {
        self.release_rate = rate;
    }

    fn notify_state_change(&mut self, state: bool, event_clock: Clock) {
        self.last_event = event_clock;
        if state {
            self.envelope_state = EnvelopeState::Attack;
            self.envelope_gain = 0.0;
        } else {
            self.envelope_state = EnvelopeState::Release;
        }
    }

    fn reset(&mut self) {
        self.wave.reset();
    }

    fn set_multiplier(&mut self, _frequency: f32, multiplier: f32) {
        self.multiplier = multiplier;
    }

    fn get_sample(&mut self, modulator: f32, event_clock: Clock) -> f32 {
        self.wave.set_frequency((self.frequency * self.multiplier) + modulator);
        let sample = self.wave.next().unwrap();

        /*
        let time_since_last = event_clock - self.last_event;
        match self.envelope_state {
            EnvelopeState::Attack => {
                let gain = rate_to_gain(self.attack_rate, time_since_last).min(self.total_level);
                if gain == self.total_level {
                    self.envelope_state = EnvelopeState::Decay1;
                }
                sample / gain
            },
            EnvelopeState::Decay1 => {
                let gain = (self.total_level - rate_to_gain(self.first_decay_rate, time_since_last)).max(self.total_level / 2.0);
                if gain == self.total_level / 2.0 {
                    self.envelope_state = EnvelopeState::Decay2;
                }
                sample / gain
            },
            EnvelopeState::Decay2 => {
                let gain = (self.total_level / 2.0 - rate_to_gain(self.second_decay_rate, time_since_last)).max(0.0);
                sample / gain
            },
            EnvelopeState::Release => {
                let gain = (self.total_level / 2.0 - rate_to_gain(self.release_rate, time_since_last)).max(0.0);
                sample / gain
            },
        }
        */
        sample
    }
}

fn rate_to_gain(rate: usize, event_clock: Clock) -> f32 {
    event_clock as f32 * rate as f32
}

#[derive(Clone)]
struct Channel {
    operators: Vec<Operator>,
    on_state: u8,
    next_on_state: u8,
    base_frequency: f32,
    algorithm: OperatorAlgorithm,
}

impl Channel {
    fn new(sample_rate: usize) -> Self {
        Self {
            operators: vec![Operator::new(sample_rate); 4],
            on_state: 0,
            next_on_state: 0,
            base_frequency: 0.0,
            algorithm: OperatorAlgorithm::A0,
        }
    }

    fn set_frequency(&mut self, frequency: f32) {
        self.base_frequency = frequency;
        for operator in self.operators.iter_mut() {
            operator.set_frequency(frequency);
        }
    }

    fn reset(&mut self) {
        for operator in self.operators.iter_mut() {
            operator.reset();
        }
    }

    fn get_sample(&mut self, event_clock: Clock) -> f32 {
        if self.on_state != self.next_on_state {
            self.on_state = self.next_on_state;
            for (i, operator) in self.operators.iter_mut().enumerate() {
                operator.notify_state_change(((self.on_state >> i) & 0x01) != 0, event_clock);
            }
        }

        if self.on_state != 0 {
            self.get_algorithm_sample(event_clock)
        } else {
            0.0
        }
    }

    fn get_algorithm_sample(&mut self, event_clock: Clock) -> f32 {
        match self.algorithm {
            OperatorAlgorithm::A0 => {
                let modulator0 = self.operators[0].get_sample(0.0, event_clock);
                let modulator1 = self.operators[1].get_sample(modulator0, event_clock);
                let modulator2 = self.operators[2].get_sample(modulator1, event_clock);
                self.operators[3].get_sample(modulator2, event_clock)
            },
            OperatorAlgorithm::A1 => {
                let sample1 = self.operators[0].get_sample(0.0, event_clock) + self.operators[1].get_sample(0.0, event_clock);
                let sample2 = self.operators[2].get_sample(sample1, event_clock);
                self.operators[3].get_sample(sample2, event_clock)
            },
            OperatorAlgorithm::A2 => {
                let sample1 = self.operators[1].get_sample(0.0, event_clock);
                let sample2 = self.operators[2].get_sample(sample1, event_clock);
                let sample3 = self.operators[0].get_sample(0.0, event_clock) + sample2;
                self.operators[3].get_sample(sample3, event_clock)
            },
            OperatorAlgorithm::A3 => {
                let sample1 = self.operators[0].get_sample(0.0, event_clock);
                let sample2 = self.operators[1].get_sample(sample1, event_clock);
                let sample3 = self.operators[2].get_sample(0.0, event_clock);
                self.operators[3].get_sample(sample2 + sample3, event_clock)
            },
            OperatorAlgorithm::A4 => {
                let sample1 = self.operators[0].get_sample(0.0, event_clock);
                let sample2 = self.operators[1].get_sample(sample1, event_clock);
                let sample3 = self.operators[2].get_sample(0.0, event_clock);
                let sample4 = self.operators[3].get_sample(sample3, event_clock);
                sample2 + sample4
            },
            OperatorAlgorithm::A5 => {
                let sample1 = self.operators[0].get_sample(0.0, event_clock);
                self.operators[1].get_sample(sample1, event_clock) + self.operators[2].get_sample(sample1, event_clock) + self.operators[3].get_sample(sample1, event_clock)
            },
            OperatorAlgorithm::A6 => {
                let sample1 = self.operators[0].get_sample(0.0, event_clock);
                let sample2 = self.operators[1].get_sample(sample1, event_clock);
                sample2 + self.operators[2].get_sample(0.0, event_clock) + self.operators[3].get_sample(0.0, event_clock)
            },
            OperatorAlgorithm::A7 => {
                self.operators[0].get_sample(0.0, event_clock)
                + self.operators[1].get_sample(0.0, event_clock)
                + self.operators[2].get_sample(0.0, event_clock)
                + self.operators[3].get_sample(0.0, event_clock)
            },
        }
    }
}


struct Dac {
    enabled: bool,
    samples: VecDeque<f32>,
}

impl Default for Dac {
    fn default() -> Self {
        Self {
            enabled: false,
            samples: VecDeque::with_capacity(100),
        }
    }
}

impl Dac {
    fn add_sample(&mut self, sample: f32) {
        self.samples.push_back(sample);
    }

    fn get_sample(&mut self) -> f32 {
        if let Some(data) = self.samples.pop_front() {
            data
        } else {
            0.0
        }
    }
}


pub struct Ym2612 {
    source: Box<dyn Audio>,
    selected_reg_0: Option<NonZeroU8>,
    selected_reg_1: Option<NonZeroU8>,

    clock_frequency: u32,
    event_clock_period: ClockElapsed,
    channels: Vec<Channel>,
    channel_frequencies: [(u8, u16); CHANNELS],
    dac: Dac,

    timer_a_enable: bool,
    timer_a: u16,
    timer_a_current: u16,
    timer_a_overflow: bool,

    timer_b_enable: bool,
    timer_b: u8,
    timer_b_current: u8,
    timer_b_overflow: bool,

    registers: Vec<u8>,
}

impl Ym2612 {
    pub fn create<H: Host>(host: &mut H, clock_frequency: u32) -> Result<Self, Error> {
        let source = host.create_audio_source()?;
        let sample_rate = source.samples_per_second();
        Ok(Self {
            source,
            selected_reg_0: None,
            selected_reg_1: None,

            clock_frequency,
            event_clock_period: 3 * 144 * 1_000_000_000 / clock_frequency as ClockElapsed,
            channels: vec![Channel::new(sample_rate); 8],
            channel_frequencies: [(0, 0); CHANNELS],

            dac: Dac::default(),

            timer_a_enable: false,
            timer_a: 0,
            timer_a_current: 0,
            timer_a_overflow: false,

            timer_b_enable: false,
            timer_b: 0,
            timer_b_current: 0,
            timer_b_overflow: false,

            registers: vec![0; 512],
        })
    }

    pub fn set_register(&mut self, bank: u8, reg: u8, data: u8) {
        // Keep a copy for debugging purposes, and if the original values are needed
        self.registers[bank as usize * 256 + reg as usize] = data;

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
                self.channels[ch].next_on_state = data >> 4;
                self.channels[ch].reset();
            },

            0x2a => {
                if self.dac.enabled {
                    for _ in 0..3 {
                        self.dac.add_sample(((data as f32 - 128.0) / 255.0) * 2.0);
                    }
                }
            },

            0x2b => {
                self.dac.enabled = data & 0x80 != 0;
            },

            reg if is_reg_range(reg, 0x30) => {
                let (ch, op) = get_ch_op(bank, reg);
                let multiplier = if data == 0 { 0.5 } else { (data & 0x0F) as f32 };
                let frequency = self.channels[ch].base_frequency;
                debug!("{}: channel {} operator {} set to multiplier {}", DEV_NAME, ch + 1, op + 1, multiplier);
                self.channels[ch].operators[op].set_multiplier(frequency, multiplier)
            },

            reg if is_reg_range(reg, 0x40) => {
                let (ch, op) = get_ch_op(bank, reg);
                // 0-127 is the attenuation, where 0 is the highest volume and 127 is the lowest, in 0.75 dB intervals
                self.channels[ch].operators[op].set_total_level((data & 0x7F) as f32 * 0.75);
            },

            reg if is_reg_range(reg, 0x50)
                || is_reg_range(reg, 0x60)
                || is_reg_range(reg, 0x70)
                || is_reg_range(reg, 0x80)
                || is_reg_range(reg, 0x90)
            => {
                let (ch, op) = get_ch_op(bank, reg);
                self.update_rates(ch, op);
            },

            reg if (0xA0..=0xA2).contains(&reg) => {
                let ch = get_ch(bank, reg);
                self.channel_frequencies[ch].1 = (self.channel_frequencies[ch].1 & 0xFF00) | data as u16;

                let frequency = fnumber_to_frequency(self.channel_frequencies[ch]);
                debug!("{}: channel {} set to frequency {}", DEV_NAME, ch + 1, frequency);
                self.channels[ch].set_frequency(frequency);
            },

            reg if (0xA4..=0xA6).contains(&reg) => {
                let ch = ((reg as usize) & 0x07) - 4 + ((bank as usize) * 3);
                self.channel_frequencies[ch].1 = (self.channel_frequencies[ch].1 & 0xFF) | ((data as u16) & 0x07) << 8;
                self.channel_frequencies[ch].0 = (data & 0x38) >> 3;
            },

            reg if (0xB0..=0xB2).contains(&reg) => {
                let ch = get_ch(bank, reg);
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

    fn update_rates(&mut self, ch: usize, op: usize) {
        let index = get_index(ch, op);
        let keycode = self.registers[0xA0 + get_ch_index(ch)] >> 1;
        let rate_scaling = self.registers[0x50 + index] & 0xC0 >> 6;
        let attack_rate = self.registers[0x50 + index] & 0x1F;
        let first_decay_rate = self.registers[0x60 + index] & 0x1F;
        let first_decay_level = (self.registers[0x80 + index] & 0x0F) >> 4;
        let second_decay_rate = self.registers[0x70 + index] & 0x1F;
        let release_rate = self.registers[0x80 + index] & 0x0F;

        self.channels[ch].operators[op].set_attack_rate(calculate_rate(attack_rate, rate_scaling, keycode));
        self.channels[ch].operators[op].set_first_decay_rate(calculate_rate(first_decay_rate, rate_scaling, keycode));
        self.channels[ch].operators[op].set_first_decay_level(calculate_rate(first_decay_level, rate_scaling, keycode));
        self.channels[ch].operators[op].set_second_decay_rate(calculate_rate(second_decay_rate, rate_scaling, keycode));
        self.channels[ch].operators[op].set_release_rate(calculate_rate(release_rate, rate_scaling, keycode));
    }
}

#[inline]
fn fnumber_to_frequency(fnumber: (u8, u16)) -> f32 {
    (fnumber.1 as f32 * 0.0264) * 2_u32.pow(fnumber.0 as u32) as f32
}

#[inline]
fn calculate_rate(rate: u8, rate_scaling: u8, keycode: u8) -> usize {
    let scale = match rate_scaling {
        0 => 8,
        1 => 4,
        2 => 2,
        3 => 1,
        _ => 8, // this shouldn't be possible
    };

    (2 * rate as usize + (keycode as usize / scale)).min(64)
}

#[inline]
fn is_reg_range(reg: u8, base: u8) -> bool {
    // There is no 4th channel in each of the groupings
    reg >= base && reg <= base + 0x0F && (reg & 0x03) != 0x03
}

/// Get the channel and operator to target with a given register number
/// and bank number.  Bank 0 refers to operators for channels 1-3, and
/// bank 1 refers to operators for channels 4-6.
#[inline]
fn get_ch_op(bank: u8, reg: u8) -> (usize, usize) {
    let ch = ((reg as usize) & 0x03) + ((bank as usize) * 3);
    let op = ((reg as usize) & 0x0C) >> 2;
    (ch, op)
}

#[inline]
fn get_index(ch: usize, op: usize) -> usize {
    let (bank, ch_l) = if ch < 3 { (0, ch) } else { (1, ch - 3) };
    (bank << 8) | op << 2 | ch
}

#[inline]
fn get_ch(bank: u8, reg: u8) -> usize {
    ((reg as usize) & 0x07) + ((bank as usize) * 3)
}

#[inline]
fn get_ch_index(ch: usize) -> usize {
    if ch < 3 {
        ch
    } else {
        0x100 + ch - 3
    }
}


impl Steppable for Ym2612 {
    fn step(&mut self, system: &System) -> Result<ClockElapsed, Error> {
        let rate = self.source.samples_per_second();
        let available = self.source.space_available();
        let samples = if available < rate / 1000 { available } else { rate / 1000 };
        let nanos_per_sample = 1_000_000_000 / rate;

        //if self.source.space_available() >= samples {
            let mut buffer = vec![0.0; samples];
            for (i, buffered_sample) in buffer.iter_mut().enumerate().take(samples) {
                let event_clock = (system.clock + (i * nanos_per_sample) as Clock) / self.event_clock_period;
                let mut sample = 0.0;

                for ch in 0..6 {
                    sample += self.channels[ch].get_sample(event_clock);
                }

                if self.dac.enabled {
                    sample += self.dac.get_sample();
                } else {
                    sample += self.channels[6].get_sample(event_clock);
                }

                *buffered_sample = sample.clamp(-1.0, 1.0);
            }
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
                data[0] = ((self.timer_a_overflow as u8) << 1) | (self.timer_b_overflow as u8);
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
                    self.set_register(0, reg.get(), data[0]);
                }
            },
            2 => {
                self.selected_reg_1 = NonZeroU8::new(data[0]);
            },
            3 => {
                if let Some(reg) = self.selected_reg_1 {
                    self.set_register(1, reg.get(), data[0]);
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

