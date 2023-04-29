//! Emulate the YM2612 FM Sound Synthesizer (used by the Sega Genesis)
//!
//! This implementation is mostly based on online references to the YM2612's registers and their
//! function, forum posts that describe the details of operation of the chip, and looking at
//! source code that emulates the chip.  It is still very much a work in progress
//!
//! Resources:
//! - Registers: https://www.smspower.org/maxim/Documents/YM2612
//! - Internal Implementation: https://gendev.spritesmind.net/forum/viewtopic.php?t=386 [Nemesis]
//!     * Envelope Generator and Corrections:
//!         http://gendev.spritesmind.net/forum/viewtopic.php?p=5716#5716
//!         http://gendev.spritesmind.net/forum/viewtopic.php?t=386&postdays=0&postorder=asc&start=417
//!     * Phase Generator and Output:
//!         http://gendev.spritesmind.net/forum/viewtopic.php?f=24&t=386&start=150
//!         http://gendev.spritesmind.net/forum/viewtopic.php?p=6224#6224

use std::f32;
use std::num::NonZeroU8;
use std::collections::VecDeque;
use lazy_static::lazy_static;

use moa_core::{debug, warn};
use moa_core::{System, Error, ClockTime, ClockDuration, Frequency, Address, Addressable, Steppable, Transmutable};
use moa_core::host::{Host, Audio};


/// Table of shift values for each possible rate angle
///
/// The value here is used to shift a bit to get the number of global cycles between each increment
/// of the envelope attenuation, based on the rate that's currently active
const COUNTER_SHIFT_VALUES: &[u16] = &[
    11, 11, 11, 11,
    10, 10, 10, 10,
     9,  9,  9,  9,
     8,  8,  8,  8,
     7,  7,  7,  7,
     6,  6,  6,  6,
     5,  5,  5,  5,
     4,  4,  4,  4,
     3,  3,  3,  3,
     2,  2,  2,  2,
     1,  1,  1,  1,
     0,  0,  0,  0,
     0,  0,  0,  0,
     0,  0,  0,  0,
     0,  0,  0,  0,
     0,  0,  0,  0,
];

/// Table of attenuation increments for each possible rate angle
///
/// The envelope rates (Attack, Decay, Sustain, and Release) are expressed as an "angle" rather
/// than attenuation, and the values will always be below 64.  This table maps each of the 64
/// possible angle values to a sequence of 8 cycles, and the amount to increment the attenuation
/// at each point in that cycle
const RATE_TABLE: &[u16] = &[
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 1, 0, 1, 0, 1, 0, 1,
    0, 1, 0, 1, 0, 1, 0, 1,
    0, 1, 0, 1, 0, 1, 0, 1,
    0, 1, 0, 1, 0, 1, 0, 1,
    0, 1, 1, 1, 0, 1, 1, 1,
    0, 1, 1, 1, 0, 1, 1, 1,
    0, 1, 0, 1, 0, 1, 0, 1,
    0, 1, 0, 1, 1, 1, 0, 1,
    0, 1, 1, 1, 0, 1, 1, 1,
    0, 1, 1, 1, 1, 1, 1, 1,
    0, 1, 0, 1, 0, 1, 0, 1,
    0, 1, 0, 1, 1, 1, 0, 1,
    0, 1, 1, 1, 0, 1, 1, 1,
    0, 1, 1, 1, 1, 1, 1, 1,
    0, 1, 0, 1, 0, 1, 0, 1,
    0, 1, 0, 1, 1, 1, 0, 1,
    0, 1, 1, 1, 0, 1, 1, 1,
    0, 1, 1, 1, 1, 1, 1, 1,
    0, 1, 0, 1, 0, 1, 0, 1,
    0, 1, 0, 1, 1, 1, 0, 1,
    0, 1, 1, 1, 0, 1, 1, 1,
    0, 1, 1, 1, 1, 1, 1, 1,
    0, 1, 0, 1, 0, 1, 0, 1,
    0, 1, 0, 1, 1, 1, 0, 1,
    0, 1, 1, 1, 0, 1, 1, 1,
    0, 1, 1, 1, 1, 1, 1, 1,
    0, 1, 0, 1, 0, 1, 0, 1,
    0, 1, 0, 1, 1, 1, 0, 1,
    0, 1, 1, 1, 0, 1, 1, 1,
    0, 1, 1, 1, 1, 1, 1, 1,
    0, 1, 0, 1, 0, 1, 0, 1,
    0, 1, 0, 1, 1, 1, 0, 1,
    0, 1, 1, 1, 0, 1, 1, 1,
    0, 1, 1, 1, 1, 1, 1, 1,
    0, 1, 0, 1, 0, 1, 0, 1,
    0, 1, 0, 1, 1, 1, 0, 1,
    0, 1, 1, 1, 0, 1, 1, 1,
    0, 1, 1, 1, 1, 1, 1, 1,
    0, 1, 0, 1, 0, 1, 0, 1,
    0, 1, 0, 1, 1, 1, 0, 1,
    0, 1, 1, 1, 0, 1, 1, 1,
    0, 1, 1, 1, 1, 1, 1, 1,
    0, 1, 0, 1, 0, 1, 0, 1,
    0, 1, 0, 1, 1, 1, 0, 1,
    0, 1, 1, 1, 0, 1, 1, 1,
    0, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 2, 1, 1, 1, 2,
    1, 2, 1, 2, 1, 2, 1, 2,
    1, 2, 2, 2, 1, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 2, 4, 2, 2, 2, 4,
    2, 4, 2, 4, 2, 4, 2, 4,
    2, 4, 4, 4, 2, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 8, 4, 4, 4, 8,
    4, 8, 4, 8, 4, 8, 4, 8,
    4, 8, 8, 8, 4, 8, 8, 8,
    8, 8, 8, 8, 8, 8, 8, 8,
    8, 8, 8, 8, 8, 8, 8, 8,
    8, 8, 8, 8, 8, 8, 8, 8,
    8, 8, 8, 8, 8, 8, 8, 8,
];

const SIN_TABLE_SIZE: usize = 512;
const POW_TABLE_SIZE: usize = 1 << 13;

lazy_static! {
    static ref SIN_TABLE: Vec<u16> = (0..SIN_TABLE_SIZE)
        .map(|i| {
            let sine = (((i * 2 + 1) as f32  / SIN_TABLE_SIZE as f32) * f32::consts::PI / 2.0).sin();
            let log_sine = -1.0 * sine.log2();
            (log_sine * (1 << 8) as f32) as u16
        })
        .collect();

    static ref POW_TABLE: Vec<u16> = (0..POW_TABLE_SIZE)
        .map(|i| {
            let linear = 2.0_f32.powf(-1.0 * (((i & 0xFF) + 1) as f32 / 256.0));
            let linear_fixed = (linear * (1 << 11) as f32) as u16;
            let shift = (i as i32 >> 8) - 2;
            if shift < 0 {
                linear_fixed << (0 - shift)
            } else if shift < 16 {
                linear_fixed >> shift
            } else {
                0
            }
        })
        .collect();
}

const DEV_NAME: &str = "ym2612";

const CHANNELS: usize = 6;
const OPERATORS: usize = 4;

const MAX_ENVELOPE: u16 = 0xFFC;
const MAX_PHASE: u32 = 0x000FFFFF;


type FmClock = u64;
type EnvelopeClock = u64;

#[repr(usize)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum EnvelopeState {
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Clone)]
struct EnvelopeGenerator {
    #[allow(dead_code)]
    debug_name: String,
    total_level: u16,
    sustain_level: u16,
    rates: [usize; 4],

    envelope_state: EnvelopeState,
    envelope: u16,
}

impl EnvelopeGenerator {
    fn new(debug_name: String) -> Self {
        Self {
            debug_name,
            total_level: 0,
            sustain_level: 0,
            rates: [0; 4],

            envelope_state: EnvelopeState::Attack,
            envelope: 0,
        }
    }

    fn set_total_level(&mut self, level: u16) {
        self.total_level = level;
    }

    fn set_sustain_level(&mut self, level: u16) {
        // Convert it to a fixed point decimal number of 4 bit : 8 bits, which will be the output
        self.sustain_level = level << 2;
    }

    fn set_rate(&mut self, etype: EnvelopeState, rate: usize) {
        self.rates[etype as usize] = rate;
    }

    fn notify_key_change(&mut self, state: bool, envelope_clock: EnvelopeClock) {
        if state {
            self.envelope = 0;
            if self.rates[EnvelopeState::Attack as usize] < 62 {
                self.envelope_state = EnvelopeState::Attack;
            } else {
                self.envelope_state = EnvelopeState::Decay;
            }
        } else {
            self.envelope_state = EnvelopeState::Release;
        }
    }

    fn update_envelope(&mut self, envelope_clock: EnvelopeClock) {
        if self.envelope_state == EnvelopeState::Decay && self.envelope >= self.sustain_level {
            self.envelope_state = EnvelopeState::Sustain;
        }

        let rate = self.rates[self.envelope_state as usize];
        let counter_shift = COUNTER_SHIFT_VALUES[rate];
        if envelope_clock % (1 << counter_shift) == 0 {
            let update_cycle = (envelope_clock >> counter_shift) & 0x07;
            let increment = RATE_TABLE[rate * 8 + update_cycle as usize];

            match self.envelope_state {
                EnvelopeState::Attack => {
                    let new_envelope = ((!self.envelope * increment) >> 4) & 0xFFFC;
                    if new_envelope > self.envelope {
                        self.envelope_state = EnvelopeState::Decay;
                        self.envelope = 0;
                    } else {
                        self.envelope = new_envelope.min(MAX_ENVELOPE);
                    }
                },
                EnvelopeState::Decay |
                EnvelopeState::Sustain |
                EnvelopeState::Release => {
                    // Convert it to a fixed point decimal number of 4 bit : 8 bits, which will be the output
                    self.envelope = (self.envelope + increment << 2).min(MAX_ENVELOPE);
                },
            }
        }
    }

    fn get_last_attenuation(&mut self) -> u16 {
        (self.envelope + self.total_level).min(MAX_ENVELOPE)
    }
}

#[derive(Clone, Debug)]
struct PhaseGenerator {
    #[allow(dead_code)]
    debug_name: String,

    block: u8,
    fnumber: u16,
    detune: u8,
    multiple: u32,

    counter: u32,
    increment: u32,
}

impl PhaseGenerator {
    fn new(debug_name: String) -> Self {
        Self {
            debug_name,

            block: 0,
            fnumber: 0,
            detune: 0,
            multiple: 1,

            counter: 0,
            increment: 0,
        }
    }

    fn reset(&mut self) {
        self.counter = 0;
    }

    fn set_block_and_fnumber(&mut self, block: u8, fnumber: u16) {
        self.block = block;
        self.fnumber = fnumber;
        self.calculate_phase_increment();
    }

    fn set_detune_and_multiple(&mut self, detune: u8, multiple: u8) {
        self.detune = detune;
        self.multiple = multiple as u32;
        self.calculate_phase_increment();
    }

    fn calculate_phase_increment(&mut self) {
        let increment = self.fnumber as u32;

        let increment = if self.block == 0 {
            increment >> 1
        } else {
            increment << self.block - 1
        };

        // TODO detune
        //let inc =

        let increment = if self.multiple == 0 {
            increment >> 1
        } else {
            (increment * self.multiple).min(MAX_PHASE)
        };

        self.increment = increment;
    }

    fn update_phase(&mut self, _fm_clock: FmClock) -> u16 {
        let phase = ((self.counter >> 10) & 0x3FF) as u16;
        self.counter += self.increment;
        phase
    }
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
    #[allow(dead_code)]
    debug_name: String,
    phase: PhaseGenerator,
    envelope: EnvelopeGenerator,
}

impl Operator {
    fn new(debug_name: String) -> Self {
        Self {
            debug_name: debug_name.clone(),
            phase: PhaseGenerator::new(debug_name.clone()),
            envelope: EnvelopeGenerator::new(debug_name),
        }
    }

    fn set_block_and_fnumber(&mut self, block: u8, fnumber: u16) {
        self.phase.set_block_and_fnumber(block, fnumber);
    }

    fn set_detune_and_multiple(&mut self, detune: u8, multiple: u8) {
        self.phase.set_detune_and_multiple(detune, multiple);
    }

    fn set_total_level(&mut self, level: u16) {
        self.envelope.set_total_level(level)
    }

    fn set_sustain_level(&mut self, level: u16) {
        self.envelope.set_sustain_level(level)
    }

    fn set_rate(&mut self, etype: EnvelopeState, rate: usize) {
        self.envelope.set_rate(etype, rate)
    }

    fn notify_key_change(&mut self, state: bool, envelope_clock: EnvelopeClock) {
        self.envelope.notify_key_change(state, envelope_clock);
        self.phase.reset();
    }

    fn get_output(&mut self, modulator: u16, clocks: (FmClock, EnvelopeClock)) -> u16 {
        let (fm_clock, envelope_clock) = clocks;
        self.envelope.update_envelope(envelope_clock);
        let envelope = self.envelope.get_last_attenuation();
        let phase = self.phase.update_phase(fm_clock);

        let mod_phase = phase + modulator;

        let mut output = POW_TABLE[(SIN_TABLE[(mod_phase & 0x1FF) as usize] + envelope) as usize];
//if self.debug_name == "ch 3, op 1" {
//print!("{:4x}", output);
//}
        if mod_phase & 0x200 != 0 {
            output = (output as i16 * -1) as u16
        }

        output
    }
}

#[inline]
fn fnumber_to_frequency(block: u8, fnumber: u16) -> f32 {
    (fnumber as f32 * 0.0264) * 2_u32.pow(block as u32) as f32
}

#[derive(Clone)]
struct Channel {
    #[allow(dead_code)]
    debug_name: String,
    operators: Vec<Operator>,
    key_state: u8,
    next_key_clock: FmClock,
    next_key_state: u8,
    base_frequency: f32,
    algorithm: OperatorAlgorithm,
}

impl Channel {
    fn new(debug_name: String) -> Self {
        Self {
            debug_name: debug_name.clone(),
            operators: (0..OPERATORS).map(|i| Operator::new(format!("{}, op {}", debug_name, i))).collect(),
            key_state: 0,
            next_key_clock: 0,
            next_key_state: 0,
            base_frequency: 0.0,
            algorithm: OperatorAlgorithm::A0,
        }
    }

    fn change_key_state(&mut self, fm_clock: FmClock, key: u8) {
        self.next_key_clock = fm_clock;
        self.next_key_state = key;
    }

    fn check_key_change(&mut self, clocks: (FmClock, EnvelopeClock)) {
        let (fm_clock, envelope_clock) = clocks;
        if self.key_state != self.next_key_state && fm_clock >= self.next_key_clock {
            self.key_state = self.next_key_state;
            for (i, operator) in self.operators.iter_mut().enumerate() {
                operator.notify_key_change(((self.key_state >> i) & 0x01) != 0, envelope_clock);
            }
        }
    }

    fn get_sample(&mut self, clocks: (FmClock, EnvelopeClock)) -> f32 {
        self.check_key_change(clocks);
        let mut output = self.get_algorithm_output(clocks);

        let output = if output & 0x2000 == 0 {
            output as i16
        } else {
            (output | 0xC000) as i16
        };
//if self.debug_name == "ch 0" {
//println!("{}", output);
//}
        let output = output as f32 / (1 << 14) as f32;
        output
    }

    fn get_algorithm_output(&mut self, clocks: (FmClock, EnvelopeClock)) -> u16 {
        match self.algorithm {
            OperatorAlgorithm::A0 => {
                let modulator0 = self.operators[0].get_output(0, clocks);
                let modulator1 = self.operators[1].get_output(modulator0, clocks);
                let modulator2 = self.operators[2].get_output(modulator1, clocks);
                self.operators[3].get_output(modulator2, clocks)
            },
            OperatorAlgorithm::A1 => {
                let output1 = self.operators[0].get_output(0, clocks) + self.operators[1].get_output(0, clocks);
                let output2 = self.operators[2].get_output(output1, clocks);
                self.operators[3].get_output(output2, clocks)
            },
            OperatorAlgorithm::A2 => {
                let output1 = self.operators[1].get_output(0, clocks);
                let output2 = self.operators[2].get_output(output1, clocks);
                let output3 = self.operators[0].get_output(0, clocks) + output2;
                self.operators[3].get_output(output3, clocks)
            },
            OperatorAlgorithm::A3 => {
                let output1 = self.operators[0].get_output(0, clocks);
                let output2 = self.operators[1].get_output(output1, clocks);
                let output3 = self.operators[2].get_output(0, clocks);
                self.operators[3].get_output(output2 + output3, clocks)
            },
            OperatorAlgorithm::A4 => {
                let output1 = self.operators[0].get_output(0, clocks);
                let output2 = self.operators[1].get_output(output1, clocks);
                let output3 = self.operators[2].get_output(0, clocks);
                let output4 = self.operators[3].get_output(output3, clocks);
                output2 + output4
            },
            OperatorAlgorithm::A5 => {
                let output1 = self.operators[0].get_output(0, clocks);
                self.operators[1].get_output(output1, clocks) + self.operators[2].get_output(output1, clocks) + self.operators[3].get_output(output1, clocks)
            },
            OperatorAlgorithm::A6 => {
                let output1 = self.operators[0].get_output(0, clocks);
                let output2 = self.operators[1].get_output(output1, clocks);
                output2 + self.operators[2].get_output(0, clocks) + self.operators[3].get_output(0, clocks)
            },
            OperatorAlgorithm::A7 => {
                self.operators[0].get_output(0, clocks)
                + self.operators[1].get_output(0, clocks)
                + self.operators[2].get_output(0, clocks)
                + self.operators[3].get_output(0, clocks)
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

    clock_frequency: Frequency,
    fm_clock_period: ClockDuration,
    next_fm_clock: FmClock,
    envelope_clock: EnvelopeClock,

    channels: Vec<Channel>,
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
    pub fn create<H: Host>(host: &mut H, clock_frequency: Frequency) -> Result<Self, Error> {
        let source = host.create_audio_source()?;
        let fm_clock = clock_frequency / (6 * 24);
        let fm_clock_period = fm_clock.period_duration();

        Ok(Self {
            source,
            selected_reg_0: None,
            selected_reg_1: None,

            clock_frequency,
            fm_clock_period,
            next_fm_clock: 0,
            envelope_clock: 0,

            channels: (0..CHANNELS).map(|i| Channel::new(format!("ch {}", i))).collect(),
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
}

impl Steppable for Ym2612 {
    fn step(&mut self, system: &System) -> Result<ClockDuration, Error> {
        let rate = self.source.samples_per_second();
        let available = self.source.space_available();
        let samples = if available < rate / 1000 { available } else { rate / 1000 };
        let sample_duration = ClockDuration::from_secs(1) / rate as u64;

        //if self.source.space_available() >= samples {
            let mut sample = 0.0;
            let mut buffer = vec![0.0; samples];
            for (i, buffered_sample) in buffer.iter_mut().enumerate().take(samples) {
                let sample_clock = system.clock + (sample_duration * i as u64);
                let fm_clock = sample_clock.as_duration() / self.fm_clock_period;

                // Simulate each clock cycle, even if we skip one due to aliasing from the unequal sampling rate of 53,267 Hz
                for clock in self.next_fm_clock..=fm_clock {
                    sample = self.get_sample(clock);
                }
                self.next_fm_clock = fm_clock + 1;

                // The DAC uses an 8000 Hz sample rate, so we don't want to skip clocks
                if self.dac.enabled {
                    sample += self.dac.get_sample();
                }
                *buffered_sample = sample.clamp(-1.0, 1.0);
            }
            self.source.write_samples(system.clock, &buffer);
        //}

        Ok(ClockDuration::from_millis(1))          // Every 1ms of simulated time
    }
}

impl Ym2612 {
    fn get_sample(&mut self, fm_clock: FmClock) -> f32 {
        if fm_clock % 3 == 0 {
            self.envelope_clock += 1;
        }
        let clocks = (fm_clock, self.envelope_clock);

        let mut sample = 0.0;

        for ch in 0..(CHANNELS - 1) {
            sample += self.channels[ch].get_sample(clocks);
        }

        if !self.dac.enabled {
            sample += self.channels[CHANNELS - 1].get_sample(clocks);
        }

        sample
    }
}

impl Ym2612 {
    pub fn set_register(&mut self, clock: ClockTime, bank: u8, reg: u8, data: u8) {
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
                if data >> 6 == 0x01 {
                    warn!("{}: ch 3 special mode requested, but not implemented", DEV_NAME);
                }
            },

            0x28 => {
                let num = (data as usize) & 0x07;
                let ch = match num {
                    0 | 1 | 2 => num,
                    4 | 5 | 6 => num - 1,
                    _ => {
                        warn!("{}: attempted key on/off to invalid channel {}", DEV_NAME, num);
                        return;
                    },
                };
                self.channels[ch].change_key_state(clock.as_duration() / self.fm_clock_period, data >> 4);
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
                let detune = (data & 0xF0) >> 4;
                let multiple = data & 0x0F;
                self.channels[ch].operators[op].set_detune_and_multiple(detune, multiple);
            },

            reg if is_reg_range(reg, 0x40) => {
                let (ch, op) = get_ch_op(bank, reg);
                // The total level (attenuation) is 0-127, where 0 is the highest volume and 127
                // is the lowest, in 0.75 dB intervals.  The 7-bit value is shifted left to
                // convert it to a 10-bit attenuation for the envelope generator, which is an
                // attenuation value in 0.09375 dB intervals
                self.channels[ch].operators[op].set_total_level(((data & 0x7F) as u16) << 3);
            },

            reg if is_reg_range(reg, 0x50)
                || is_reg_range(reg, 0x60)
                || is_reg_range(reg, 0x70)
                || is_reg_range(reg, 0x80)
                || is_reg_range(reg, 0x90)
            => {
                self.update_rates(bank, reg & 0x0F);
            },

            reg if (0xA0..=0xA2).contains(&reg) => {
                self.update_fnumber(bank, reg & 0x0F);
            },

            reg if (0xA4..=0xA6).contains(&reg) => {
                self.update_fnumber(bank, reg & 0x0F);
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

    fn update_fnumber(&mut self, bank: u8, lower_reg: u8) {
        let index = bank as usize * 256 + lower_reg as usize;
        let block = (self.registers[0xA4 + index] & 0x38) >> 3;
        let fnumber = ((self.registers[0xA4 + index] as u16 & 0x07) << 8) | self.registers[0xA0 + index] as u16;

        let (ch, op) = get_ch_op(bank, lower_reg);
        self.channels[ch].operators[op].set_block_and_fnumber(block, fnumber);
    }

    fn update_rates(&mut self, bank: u8, lower_reg: u8) {
        let index = bank as usize * 256 + lower_reg as usize;
        let (ch, op) = get_ch_op(bank, lower_reg);
        let keycode = self.registers[0xA0 + get_ch_index(ch)] >> 1;
        let rate_scaling = self.registers[0x50 + index] & 0xC0 >> 6;

        let attack_rate = self.registers[0x50 + index] & 0x1F;
        let first_decay_rate = self.registers[0x60 + index] & 0x1F;
        let second_decay_rate = self.registers[0x70 + index] & 0x1F;
        let release_rate = ((self.registers[0x80 + index] & 0x0F) << 1) + 1;    // register is only 4 bits, so it's adjusted to 5-bits with 1 in the LSB

        self.channels[ch].operators[op].set_rate(EnvelopeState::Attack, calculate_rate(attack_rate, rate_scaling, keycode));
        self.channels[ch].operators[op].set_rate(EnvelopeState::Decay, calculate_rate(first_decay_rate, rate_scaling, keycode));
        self.channels[ch].operators[op].set_rate(EnvelopeState::Sustain, calculate_rate(second_decay_rate, rate_scaling, keycode));
        self.channels[ch].operators[op].set_rate(EnvelopeState::Release, calculate_rate(release_rate, rate_scaling, keycode));

        let sustain_level = (self.registers[0x80 + index] as u16 & 0xF0) << 2;  // register is 4 bits, so it's adjusted to match total_level's scale
        let sustain_level = if sustain_level == (0xF0 << 2) { MAX_ENVELOPE } else { sustain_level };    // adjust the maximum storable value to be the max attenuation
        self.channels[ch].operators[op].set_sustain_level(sustain_level);
    }
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

    (2 * rate as usize + (keycode as usize / scale)).min(63)
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

impl Addressable for Ym2612 {
    fn len(&self) -> usize {
        0x04
    }

    fn read(&mut self, _clock: ClockTime, addr: Address, data: &mut [u8]) -> Result<(), Error> {
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

    fn write(&mut self, clock: ClockTime, addr: Address, data: &[u8]) -> Result<(), Error> {
        debug!("{}: write to register {:x} with {:x}", DEV_NAME, addr, data[0]);
        match addr {
            0 => {
                self.selected_reg_0 = NonZeroU8::new(data[0]);
            },
            1 => {
                if let Some(reg) = self.selected_reg_0 {
                    self.set_register(clock, 0, reg.get(), data[0]);
                }
            },
            2 => {
                self.selected_reg_1 = NonZeroU8::new(data[0]);
            },
            3 => {
                if let Some(reg) = self.selected_reg_1 {
                    self.set_register(clock, 1, reg.get(), data[0]);
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

