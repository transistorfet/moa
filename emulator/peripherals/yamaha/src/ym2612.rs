//! Emulate the YM2612 FM Sound Synthesizer (used by the Sega Genesis)
//!
//! This implementation is mostly based on online references to the YM2612's registers and their
//! function, forum posts that describe the details of operation of the chip, and looking at
//! source code that emulates the chip.  It is still very much a work in progress
//!
//! Resources:
//! - Registers: <https://www.smspower.org/maxim/Documents/YM2612>
//! - Internal Implementation: <https://gendev.spritesmind.net/forum/viewtopic.php?t=386> (Nemesis)
//!     * Envelope Generator and Corrections:
//!         <http://gendev.spritesmind.net/forum/viewtopic.php?p=5716#5716>
//!         <http://gendev.spritesmind.net/forum/viewtopic.php?t=386&postdays=0&postorder=asc&start=417>
//!     * Phase Generator and Output:
//!         <http://gendev.spritesmind.net/forum/viewtopic.php?f=24&t=386&start=150>
//!         <http://gendev.spritesmind.net/forum/viewtopic.php?p=6224#6224>

use std::f32;
use std::num::NonZeroU8;
use std::collections::VecDeque;
use lazy_static::lazy_static;
use femtos::{Instant, Duration, Frequency};

use moa_core::{System, Error, Address, Addressable, Steppable, Transmutable};
use moa_host::{Host, HostError, Audio, Sample};


/// Table of shift values for each possible rate angle
///
/// The value here is used to shift a bit to get the number of global cycles between each increment
/// of the envelope attenuation, based on the rate that's currently active
#[rustfmt::skip]
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
#[rustfmt::skip]
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

#[rustfmt::skip]
const DETUNE_TABLE: &[u8] = &[
    0,  0,  1,  2,
    0,  0,  1,  2,
    0,  0,  1,  2,
    0,  0,  1,  2,
    0,  1,  2,  2,
    0,  1,  2,  3,
    0,  1,  2,  3,
    0,  1,  2,  3,
    0,  1,  2,  4,
    0,  1,  3,  4,
    0,  1,  3,  4,
    0,  1,  3,  5,
    0,  2,  4,  5,
    0,  2,  4,  6,
    0,  2,  4,  6,
    0,  2,  5,  7,
    0,  2,  5,  8,
    0,  3,  6,  8,
    0,  3,  6,  9,
    0,  3,  7, 10,
    0,  4,  8, 11,
    0,  4,  8, 12,
    0,  4,  9, 13,
    0,  5, 10, 14,
    0,  5, 11, 16,
    0,  6, 12, 17,
    0,  6, 13, 19,
    0,  7, 14, 20,
    0,  8, 16, 22,
    0,  8, 16, 22,
    0,  8, 16, 22,
    0,  8, 16, 22,
];

const SIN_TABLE_SIZE: usize = 512;
const POW_TABLE_SIZE: usize = 1 << 13;

lazy_static! {
    static ref SIN_TABLE: Vec<u16> = (0..SIN_TABLE_SIZE)
        .map(|i| {
            let sine = (((i * 2 + 1) as f32  / SIN_TABLE_SIZE as f32) * f32::consts::PI / 2.0).sin();
            let log_sine = -1.0 * sine.log2();
            // Convert to fixed decimal notation with 4.8 bit format
            (log_sine * (1 << 8) as f32) as u16
        })
        .collect();

    static ref POW_TABLE: Vec<i16> = (0..POW_TABLE_SIZE)
        .map(|i| {
            let linear = 2.0_f32.powf(-1.0 * (((i & 0xFF) + 1) as f32 / 256.0));
            let linear_fixed = (linear * (1 << 11) as f32) as i16;
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
const ENVELOPE_CENTER: u16 = 0x800;
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
    rates: [u8; 4],

    envelope_state: EnvelopeState,
    envelope: u16,
    last_envelope_clock: EnvelopeClock,
}

impl EnvelopeGenerator {
    fn new(debug_name: String) -> Self {
        Self {
            debug_name,
            total_level: 0,
            sustain_level: 0,
            rates: [0; 4],

            envelope_state: EnvelopeState::Release,
            envelope: MAX_ENVELOPE,
            last_envelope_clock: 0,
        }
    }

    fn set_total_level(&mut self, level: u16) {
        self.total_level = level;
    }

    fn set_sustain_level(&mut self, level: u16) {
        self.sustain_level = level;
    }

    fn set_rate(&mut self, etype: EnvelopeState, rate: u8) {
        self.rates[etype as usize] = rate;
    }

    fn get_scaled_rate(&self, etype: EnvelopeState, rate_adjust: usize) -> usize {
        calculate_rate(self.rates[etype as usize], rate_adjust)
    }

    fn notify_key_change(&mut self, state: bool, _envelope_clock: EnvelopeClock, rate_adjust: usize) {
        if state {
            let rate = self.get_scaled_rate(EnvelopeState::Attack, rate_adjust);
            if rate < 62 {
                self.envelope_state = EnvelopeState::Attack;
            } else {
                self.envelope = 0;
                self.envelope_state = EnvelopeState::Decay;
            }
        } else {
            self.envelope_state = EnvelopeState::Release;
        }
    }

    fn update_envelope(&mut self, envelope_clock: EnvelopeClock, rate_adjust: usize) {
        if self.envelope_state == EnvelopeState::Decay && self.envelope >= self.sustain_level {
            self.envelope_state = EnvelopeState::Sustain;
        }

        let rate = self.get_scaled_rate(self.envelope_state, rate_adjust);
        let counter_shift = COUNTER_SHIFT_VALUES[rate];

        if envelope_clock % (1 << counter_shift) == 0 {
            let update_cycle = (envelope_clock >> counter_shift) & 0x07;
            let increment = RATE_TABLE[rate * 8 + update_cycle as usize];

            match self.envelope_state {
                EnvelopeState::Attack => {
                    // NOTE: the adjustment added to the envelope is negative, but the envelope is an unsigned number, so
                    // it's converted to signed to ensure an arithmetic (sign-extending) shift right is used.  The addition
                    // will work the same regardless due to the magic of two's complement numbers.  It would have also worked
                    // to bitwise-and with 0xFFC instead, which will wrap the number to a 12-bit signed number, which when
                    // clamped to MAX_ENVELOPE will produce the same results
                    let new_envelope = self.envelope + (((!self.envelope * increment) as i16) >> 4) as u16;
                    if new_envelope > self.envelope {
                        self.envelope_state = EnvelopeState::Decay;
                        self.envelope = 0;
                    } else {
                        self.envelope = new_envelope.min(MAX_ENVELOPE);
                    }
                },
                EnvelopeState::Decay | EnvelopeState::Sustain | EnvelopeState::Release => {
                    // Convert it to a fixed point decimal number of 4 bit : 8 bits, which will be the output
                    self.envelope += increment << 2;
                    if self.envelope > MAX_ENVELOPE
                        || self.envelope_state == EnvelopeState::Release && self.envelope >= ENVELOPE_CENTER
                    {
                        self.envelope = MAX_ENVELOPE;
                    }
                },
            }
        }
    }

    fn get_envelope(&mut self, envelope_clock: EnvelopeClock, rate_adjust: usize) -> u16 {
        if envelope_clock != self.last_envelope_clock {
            self.update_envelope(envelope_clock, rate_adjust);
            self.last_envelope_clock = envelope_clock;
        }
        (self.envelope + self.total_level).min(MAX_ENVELOPE)
    }
}

#[inline]
fn calculate_rate(rate: u8, rate_adjust: usize) -> usize {
    if rate == 0 {
        0
    } else {
        (2 * rate as usize + rate_adjust).min(63)
    }
}

#[inline]
fn get_rate_adjust(rate_scaling: u8, keycode: usize) -> usize {
    keycode >> rate_scaling
}

#[derive(Clone, Debug)]
struct PhaseGenerator {
    #[allow(dead_code)]
    debug_name: String,

    block: u8,
    fnumber: u16,
    detune: u8,
    multiple: u32,
    rate_scaling: u8,

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
            rate_scaling: 0,

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

    fn set_rate_scaling(&mut self, rate_scaling: u8) {
        self.rate_scaling = rate_scaling;
    }

    fn get_rate_adjust(&self) -> usize {
        let keycode = get_keycode(self.block, self.fnumber);
        get_rate_adjust(self.rate_scaling, keycode)
    }

    fn calculate_phase_increment(&mut self) {
        // Start with the Fnumber
        let increment = self.fnumber as u32;

        // Shift according to the block (octave)
        let increment = if self.block == 0 {
            increment >> 1
        } else {
            increment << (self.block - 1)
        };

        // Apply detune
        let keycode = get_keycode(self.block, self.fnumber);
        let sign = self.detune >> 2;
        let detune_index = (self.detune & 0x03) as usize;
        let detune = DETUNE_TABLE[keycode * 4 + detune_index] as u32;
        let increment = if sign == 0 {
            increment.saturating_add(detune)
        } else {
            increment.saturating_sub(detune)
        }
        .min(0x1FFFF);

        // Apply multiple
        let increment = if self.multiple == 0 {
            increment >> 1
        } else {
            (increment * self.multiple).min(MAX_PHASE)
        };

        // Cache the value for use later, since it only changes when the input registers are set
        self.increment = increment;
    }

    fn update_phase(&mut self, _fm_clock: FmClock) -> i16 {
        let phase = ((self.counter >> 10) & 0x3FF) as i16;
        self.counter += self.increment;
        phase
    }
}

/// Map the upper 5 bits of the fnumber to the lower 2 bits of the keycode
///
/// The upper of the two bits is bit 11 of the fnumber, and the lower bit is follows
/// the formula F11 & (F10 | F9 | F8) | !F11 & (F10 & F9 & F8), where the bit numbers
/// of the fnumber value start from 1 instead of 0.  It's easier to map this with an
/// lookup table than to calculate this.
///
/// K1 = F11
/// K0 = F11 & (F10 | F9 | F8) | !F11 & (F10 & F9 & F8)
#[rustfmt::skip]
const FNUMBER_TO_KEYCODE: &[u8] = &[
    0, 0, 0, 0, 0, 0, 0, 1,
    2, 3, 3, 3, 3, 3, 3, 3,
];

/// Generate the keycode required for detune calculations using the block and fnumber
fn get_keycode(block: u8, fnumber: u16) -> usize {
    ((block as usize) << 2) | FNUMBER_TO_KEYCODE[(fnumber as usize) >> 7] as usize
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
    output: i16,
}

impl Operator {
    fn new(debug_name: String) -> Self {
        Self {
            debug_name: debug_name.clone(),
            phase: PhaseGenerator::new(debug_name.clone()),
            envelope: EnvelopeGenerator::new(debug_name),
            output: 0,
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

    fn set_rate(&mut self, etype: EnvelopeState, rate: u8) {
        self.envelope.set_rate(etype, rate)
    }

    fn set_rate_scaling(&mut self, rate_scaling: u8) {
        self.phase.set_rate_scaling(rate_scaling)
    }

    fn notify_key_change(&mut self, state: bool, envelope_clock: EnvelopeClock) {
        self.envelope
            .notify_key_change(state, envelope_clock, self.phase.get_rate_adjust());
        self.phase.reset();
    }

    fn get_output(&mut self, modulator: i16, clocks: (FmClock, EnvelopeClock)) -> i16 {
        let (fm_clock, envelope_clock) = clocks;

        let envelope = self.envelope.get_envelope(envelope_clock, self.phase.get_rate_adjust());
        let phase = self.phase.update_phase(fm_clock);

        let mod_phase = phase + modulator;

        // The sine table contains the first half of the wave as an attenuation value
        // Use the phase with the sign truncated to get the attenuation, plus the
        // attenuation from the envelope, to get the total attenuation at this point
        let attenuation = SIN_TABLE[(mod_phase & 0x1FF) as usize] + envelope;

        // The power table contains the 13-bit output (not including the sign) for a
        // 12 bit attenuation value, which is then negated based on the sign of the phase
        let mut output = POW_TABLE[attenuation as usize];

        // If the original phase was in the negative portion, invert the output
        // since the sine wave's second half is a mirror of the first half
        if mod_phase & 0x200 != 0 {
            output *= -1;
        }
        // The output is now represented with a 16-bit signed number in the range of -0x1FFF and 0x1FFF

        // Save the output for use as feedback later
        self.output = output;

        output
    }
}


#[derive(Clone)]
struct Channel {
    #[allow(dead_code)]
    debug_name: String,
    enabled: (bool, bool),
    operators: Vec<Operator>,
    algorithm: OperatorAlgorithm,
    feedback: u8,

    key_state: u8,
    next_key_clock: FmClock,
    next_key_state: u8,
    op1_output: [i16; 2],
}

impl Channel {
    fn new(debug_name: String) -> Self {
        Self {
            debug_name: debug_name.clone(),
            enabled: (true, true),
            operators: (0..OPERATORS)
                .map(|i| Operator::new(format!("{}, op {}", debug_name, i)))
                .collect(),
            algorithm: OperatorAlgorithm::A0,
            feedback: 0,

            key_state: 0,
            next_key_clock: 0,
            next_key_state: 0,
            op1_output: [0; 2],
        }
    }

    fn set_enabled(&mut self, left: bool, right: bool) {
        self.enabled = (left, right);
    }

    fn set_algorithm_and_feedback(&mut self, algorithm: OperatorAlgorithm, feedback: u8) {
        self.algorithm = algorithm;
        self.feedback = feedback;
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

    fn get_sample(&mut self, clocks: (FmClock, EnvelopeClock)) -> (f32, f32) {
        self.check_key_change(clocks);
        let feedback = if self.feedback != 0 {
            (self.op1_output[0] + self.op1_output[1]) >> (10 - self.feedback)
        } else {
            0
        };

        let output = self.get_algorithm_output(clocks, feedback);

        self.op1_output[0] = self.op1_output[1];
        self.op1_output[1] = self.operators[0].output;

        //let output = sign_extend_u16(output, 14);

        let sample = output as f32 / (1 << 13) as f32;

        let left = if self.enabled.0 { sample } else { 0.0 };
        let right = if self.enabled.1 { sample } else { 0.0 };
        (left, right)
    }

    fn get_algorithm_output(&mut self, clocks: (FmClock, EnvelopeClock), feedback: i16) -> i16 {
        match self.algorithm {
            OperatorAlgorithm::A0 => {
                let modulator0 = self.operators[0].get_output(feedback, clocks);
                let modulator1 = self.operators[1].get_output(modulator0, clocks);
                let modulator2 = self.operators[2].get_output(modulator1, clocks);
                self.operators[3].get_output(modulator2, clocks)
            },
            OperatorAlgorithm::A1 => {
                let output1 = self.operators[0].get_output(feedback, clocks) + self.operators[1].get_output(0, clocks);
                let output2 = self.operators[2].get_output(output1, clocks);
                self.operators[3].get_output(output2, clocks)
            },
            OperatorAlgorithm::A2 => {
                let output1 = self.operators[0].get_output(feedback, clocks);
                let output2 = self.operators[1].get_output(0, clocks);
                let output3 = self.operators[2].get_output(output2, clocks);
                let output4 = output1 + output3;
                self.operators[3].get_output(output4, clocks)
            },
            OperatorAlgorithm::A3 => {
                let output1 = self.operators[0].get_output(feedback, clocks);
                let output2 = self.operators[1].get_output(output1, clocks);
                let output3 = self.operators[2].get_output(0, clocks);
                self.operators[3].get_output(output2 + output3, clocks)
            },
            OperatorAlgorithm::A4 => {
                let output1 = self.operators[0].get_output(feedback, clocks);
                let output2 = self.operators[1].get_output(output1, clocks);
                let output3 = self.operators[2].get_output(0, clocks);
                let output4 = self.operators[3].get_output(output3, clocks);
                output2 + output4
            },
            OperatorAlgorithm::A5 => {
                let output1 = self.operators[0].get_output(feedback, clocks);
                self.operators[1].get_output(output1, clocks)
                    + self.operators[2].get_output(output1, clocks)
                    + self.operators[3].get_output(output1, clocks)
            },
            OperatorAlgorithm::A6 => {
                let output1 = self.operators[0].get_output(feedback, clocks);
                let output2 = self.operators[1].get_output(output1, clocks);
                output2 + self.operators[2].get_output(0, clocks) + self.operators[3].get_output(0, clocks)
            },
            OperatorAlgorithm::A7 => {
                self.operators[0].get_output(feedback, clocks)
                    + self.operators[1].get_output(0, clocks)
                    + self.operators[2].get_output(0, clocks)
                    + self.operators[3].get_output(0, clocks)
            },
        }
    }
}


struct Dac {
    enabled: bool,
    samples: VecDeque<(FmClock, f32)>,
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
    fn add_sample(&mut self, clock: FmClock, sample: f32) {
        self.samples.push_back((clock, sample));
    }

    fn get_sample_after(&mut self, clock: FmClock) -> f32 {
        if let Some((sample_clock, data)) = self.samples.front().cloned() {
            if clock > sample_clock {
                self.samples.pop_front();
                return data;
            }
        }

        0.0
    }
}


pub struct Ym2612 {
    source: Box<dyn Audio>,
    selected_reg_0: Option<NonZeroU8>,
    selected_reg_1: Option<NonZeroU8>,

    fm_clock_period: Duration,
    next_fm_clock: FmClock,
    envelope_clock: EnvelopeClock,

    channels: Vec<Channel>,
    dac: Dac,

    // TODO the timer hasn't been implemented yet
    #[allow(dead_code)]
    timer_a_enable: bool,
    timer_a: u16,
    #[allow(dead_code)]
    timer_a_current: u16,
    timer_a_overflow: bool,

    #[allow(dead_code)]
    timer_b_enable: bool,
    timer_b: u8,
    #[allow(dead_code)]
    timer_b_current: u8,
    timer_b_overflow: bool,

    registers: Vec<u8>,
}

impl Ym2612 {
    pub fn new<H, E>(host: &mut H, clock_frequency: Frequency) -> Result<Self, HostError<E>>
    where
        H: Host<Error = E>,
    {
        let source = host.add_audio_source()?;
        let fm_clock = clock_frequency / (6 * 24);
        let fm_clock_period = fm_clock.period_duration();

        Ok(Self {
            source,
            selected_reg_0: None,
            selected_reg_1: None,

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
    fn step(&mut self, system: &System) -> Result<Duration, Error> {
        let rate = self.source.samples_per_second();
        let samples = rate / 1000;
        let sample_duration = Duration::from_secs(1) / rate as u64;

        let mut sample = 0.0;
        let mut buffer = vec![Sample(0.0, 0.0); samples];
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
                sample += self.dac.get_sample_after(fm_clock);
            }

            // TODO add stereo output, which is supported by ym2612
            let sample = sample.clamp(-1.0, 1.0);
            *buffered_sample = Sample(sample, sample);
        }
        self.source.write_samples(system.clock, &buffer);

        Ok(Duration::from_millis(1)) // Every 1ms of simulated time
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
            sample += self.channels[ch].get_sample(clocks).0;
        }

        if !self.dac.enabled {
            sample += self.channels[CHANNELS - 1].get_sample(clocks).0;
        }

        sample
    }
}

impl Ym2612 {
    pub fn set_register(&mut self, clock: Instant, bank: u8, reg: u8, data: u8) {
        // Keep a copy for debugging purposes, and if the original values are needed
        self.registers[bank as usize * 256 + reg as usize] = data;
        println!("set {:x} to {:x}", bank as usize * 256 + reg as usize, data);

        //log::warn!("{}: set reg {}{:x} to {:x}", DEV_NAME, bank, reg, data);
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
                    log::warn!("{}: ch 3 special mode requested, but not implemented", DEV_NAME);
                }
            },

            0x28 => {
                let num = (data as usize) & 0x07;
                let ch = match num {
                    0..=2 => num,
                    4..=6 => num - 1,
                    _ => {
                        log::warn!("{}: attempted key on/off to invalid channel {}", DEV_NAME, num);
                        return;
                    },
                };
                self.channels[ch].change_key_state(clock.as_duration() / self.fm_clock_period, data >> 4);
            },

            0x2a => {
                if self.dac.enabled {
                    let fm_clock = clock.as_duration() / self.fm_clock_period;
                    for i in 0..3 {
                        self.dac.add_sample(fm_clock + i, ((data as f32 - 128.0) / 255.0) * 2.0);
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
                self.channels[ch].operators[op].set_total_level(((data & 0x7F) as u16) << 5);
            },

            reg if is_reg_range(reg, 0x50) => {
                let (ch, op) = get_ch_op(bank, reg);
                let index = get_index(bank, reg);

                let rate_scaling = self.registers[0x50 + index] & 0xC0 >> 6;
                self.channels[ch].operators[op].set_rate_scaling(3 - rate_scaling);

                let attack_rate = self.registers[0x50 + index] & 0x1F;
                self.channels[ch].operators[op].set_rate(EnvelopeState::Attack, attack_rate);
            },

            reg if is_reg_range(reg, 0x60) => {
                let (ch, op) = get_ch_op(bank, reg);
                let index = get_index(bank, reg);

                let first_decay_rate = self.registers[0x60 + index] & 0x1F;
                self.channels[ch].operators[op].set_rate(EnvelopeState::Decay, first_decay_rate);
            },

            reg if is_reg_range(reg, 0x70) => {
                let (ch, op) = get_ch_op(bank, reg);
                let index = get_index(bank, reg);

                let second_decay_rate = self.registers[0x70 + index] & 0x1F;
                self.channels[ch].operators[op].set_rate(EnvelopeState::Sustain, second_decay_rate);
            },

            reg if is_reg_range(reg, 0x80) => {
                let (ch, op) = get_ch_op(bank, reg);
                let index = get_index(bank, reg);

                // Register is only 4 bits, so adjust it to 5-bits with 1 in the LSB
                let release_rate = ((self.registers[0x80 + index] & 0x0F) << 1) + 1;
                self.channels[ch].operators[op].set_rate(EnvelopeState::Release, release_rate);

                // Register is 4 bits, so adjust it to match total_level's scale
                let sustain_level = (self.registers[0x80 + index] as u16 & 0xF0) << 3;
                // Adjust the maximum storable value to be the max attenuation
                let sustain_level = if sustain_level == (0x00F0 << 3) {
                    MAX_ENVELOPE
                } else {
                    sustain_level
                };
                self.channels[ch].operators[op].set_sustain_level(sustain_level);
            },

            reg if (0xA0..=0xA2).contains(&reg) => {
                self.update_fnumber(bank, reg & 0x0F);
            },

            reg if (0xA4..=0xA6).contains(&reg) => {
                self.update_fnumber(bank, reg & 0x0F);
            },

            reg if (0xB0..=0xB2).contains(&reg) => {
                let ch = get_ch(bank, reg);

                let feedback = (data >> 3) & 0x07;

                let algorithm = match data & 0x07 {
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

                self.channels[ch].set_algorithm_and_feedback(algorithm, feedback);
            },

            reg if (0xB4..=0xB6).contains(&reg) => {
                let ch = get_ch(bank, reg - 4);
                // TODO add AMS and FMS (which only apply to the LFO)
                self.channels[ch].set_enabled(data & 0x80 != 0, data & 0x40 != 0);
            },

            _ => {
                log::warn!("{}: !!! unhandled write to register {:0x} with {:0x}", DEV_NAME, reg, data);
            },
        }
    }

    #[inline]
    fn get_block_and_fnumber(&self, bank: u8, lower_reg: u8) -> (u8, u16) {
        let index = bank as usize * 256 + lower_reg as usize;
        let block = (self.registers[0xA4 + index] & 0x38) >> 3;
        let fnumber = ((self.registers[0xA4 + index] as u16 & 0x07) << 8) | self.registers[0xA0 + index] as u16;
        (block, fnumber)
    }

    fn update_fnumber(&mut self, bank: u8, lower_reg: u8) {
        let (block, fnumber) = self.get_block_and_fnumber(bank, lower_reg);
        let (ch, op) = get_ch_op(bank, lower_reg);
        self.channels[ch].operators[op].set_block_and_fnumber(block, fnumber);
    }
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
fn get_index(bank: u8, reg: u8) -> usize {
    bank as usize * 256 + (reg & 0x0F) as usize
}

#[inline]
fn get_ch(bank: u8, reg: u8) -> usize {
    ((reg as usize) & 0x07) + ((bank as usize) * 3)
}

impl Addressable for Ym2612 {
    fn size(&self) -> usize {
        0x04
    }

    fn read(&mut self, _clock: Instant, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        match addr {
            0..=3 => {
                // Read the status byte (busy/overflow)
                data[0] = ((self.timer_a_overflow as u8) << 1) | (self.timer_b_overflow as u8);
            },
            _ => {
                log::warn!("{}: !!! unhandled read from {:0x}", DEV_NAME, addr);
            },
        }
        log::debug!("{}: read from register {:x} of {:?}", DEV_NAME, addr, data);
        Ok(())
    }

    fn write(&mut self, clock: Instant, addr: Address, data: &[u8]) -> Result<(), Error> {
        log::debug!("{}: write to register {:x} with {:x}", DEV_NAME, addr, data[0]);
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
                log::warn!("{}: !!! unhandled write {:0x} to {:0x}", DEV_NAME, data[0], addr);
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
