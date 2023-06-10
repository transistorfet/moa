
use std::time::Duration;
use std::ops::{Add, AddAssign, Sub, SubAssign, Mul, MulAssign, Div, DivAssign};

/// Type to use for storing femtoseconds
///
/// In webassembly, using u128 results in exceedingly slow runtimes, so we use u64 instead
/// which is enough for 5 hours of simulation time.
#[cfg(not(target_arch = "wasm32"))]
type Femtos = u128;
#[cfg(target_arch = "wasm32")]
type Femtos = u64;

/// Represents a duration of time in femtoseconds
///
/// The `ClockDuration` type is used to represent lengths of time and is
/// intentionally similar to `std::time::Duration`, but which records
/// time as femtoseconds to keep accurancy when dealing with partial
/// nanosecond clock divisons.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClockDuration {
    femtos: Femtos,
}

impl ClockDuration {
    pub const ZERO: Self = Self::from_femtos(0);
    pub const MAX: Self = Self::from_femtos(Femtos::MAX);

    pub const FEMTOS_PER_SEC: Femtos = 1_000_000_000_000_000;
    pub const FEMTOS_PER_MILLISEC: Femtos = 1_000_000_000_000;
    pub const FEMTOS_PER_MICROSEC: Femtos = 1_000_000_000;
    pub const FEMTOS_PER_NANOSEC: Femtos = 1_000_000;
    pub const FEMTOS_PER_PICOSEC: Femtos = 1_000;

    #[inline]
    pub const fn from_secs(secs: u64) -> Self {
        Self {
            femtos: secs as Femtos * Self::FEMTOS_PER_SEC,
        }
    }

    #[inline]
    pub const fn from_millis(millisecs: u64) -> Self {
        Self {
            femtos: millisecs as Femtos * Self::FEMTOS_PER_MILLISEC,
        }
    }

    #[inline]
    pub const fn from_micros(microsecs: u64) -> Self {
        Self {
            femtos: microsecs as Femtos * Self::FEMTOS_PER_MICROSEC,
        }
    }

    #[inline]
    pub const fn from_nanos(nanosecs: u64) -> Self {
        Self {
            femtos: nanosecs as Femtos * Self::FEMTOS_PER_NANOSEC,
        }
    }

    #[inline]
    pub const fn from_picos(picosecs: u128) -> Self {
        Self {
            femtos: picosecs as Femtos * Self::FEMTOS_PER_PICOSEC,
        }
    }

    #[inline]
    pub const fn from_femtos(femtos: Femtos) -> Self {
        Self {
            femtos,
        }
    }

    #[inline]
    pub const fn as_secs(self) -> u64 {
        (self.femtos / Self::FEMTOS_PER_SEC) as u64
    }

    #[inline]
    pub const fn as_millis(self) -> u64 {
        (self.femtos / Self::FEMTOS_PER_MILLISEC) as u64
    }

    #[inline]
    pub const fn as_micros(self) -> u64 {
        (self.femtos / Self::FEMTOS_PER_MICROSEC) as u64
    }

    #[inline]
    pub const fn as_nanos(self) -> u64 {
        (self.femtos / Self::FEMTOS_PER_NANOSEC) as u64
    }

    #[inline]
    pub const fn as_picos(self) -> u128 {
        (self.femtos / Self::FEMTOS_PER_PICOSEC) as u128
    }

    #[inline]
    pub const fn as_femtos(self) -> Femtos {
        self.femtos
    }

    #[inline]
    pub const fn checked_add(self, rhs: Self) -> Option<Self> {
        match self.femtos.checked_add(rhs.femtos) {
            Some(femtos) => Some(Self::from_femtos(femtos)),
            None => None,
        }
    }

    #[inline]
    pub const fn checked_sub(self, rhs: Self) -> Option<Self> {
        match self.femtos.checked_sub(rhs.femtos) {
            Some(femtos) => Some(Self::from_femtos(femtos)),
            None => None,
        }
    }
}

impl Add for ClockDuration {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        self.checked_add(rhs).expect("clock duration overflow during addition")
    }
}

impl AddAssign for ClockDuration {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for ClockDuration {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        self.checked_sub(rhs).expect("clock duration overflow during subtraction")
    }
}

impl SubAssign for ClockDuration {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl Mul<u64> for ClockDuration {
    type Output = Self;

    fn mul(self, rhs: u64) -> Self::Output {
        Self::from_femtos(self.femtos * rhs as Femtos)
    }
}

impl MulAssign<u64> for ClockDuration {
    fn mul_assign(&mut self, rhs: u64) {
        *self = Self::from_femtos(self.femtos * rhs as Femtos);
    }
}

impl Div<u64> for ClockDuration {
    type Output = Self;

    fn div(self, rhs: u64) -> Self::Output {
        Self::from_femtos(self.femtos / rhs as Femtos)
    }
}

impl DivAssign<u64> for ClockDuration {
    fn div_assign(&mut self, rhs: u64) {
        *self = Self::from_femtos(self.femtos / rhs as Femtos);
    }
}

impl Div<ClockDuration> for ClockDuration {
    type Output = u64;

    fn div(self, rhs: ClockDuration) -> Self::Output {
        (self.femtos / rhs.femtos) as u64
    }
}


impl From<ClockDuration> for Duration {
    fn from(value: ClockDuration) -> Self {
        Duration::from_nanos(value.as_nanos())
    }
}

impl From<Duration> for ClockDuration {
    fn from(value: Duration) -> Self {
        ClockDuration::from_nanos(value.as_nanos() as u64)
    }
}


/// Represents time from the start of the simulation
///
/// `ClockTime` is for representing the current running clock.  It uses a
/// duration to represent the time from simulation start, and is monotonic.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClockTime(ClockDuration);

impl ClockTime {
    pub const START: Self = Self(ClockDuration::ZERO);
    pub const FOREVER: Self = Self(ClockDuration::MAX);

    #[inline]
    pub const fn as_duration(self) -> ClockDuration {
        self.0
    }

    #[inline]
    pub fn duration_since(self, other: Self) -> ClockDuration {
        self.0 - other.0
    }

    #[inline]
    pub const fn checked_add(self, duration: ClockDuration) -> Option<Self> {
        match self.0.checked_add(duration) {
            Some(duration) => Some(Self(duration)),
            None => None,
        }
    }

    #[inline]
    pub const fn checked_sub(self, duration: ClockDuration) -> Option<Self> {
        match self.0.checked_sub(duration) {
            Some(duration) => Some(Self(duration)),
            None => None,
        }
    }
}

impl Add<ClockDuration> for ClockTime {
    type Output = Self;

    fn add(self, rhs: ClockDuration) -> Self::Output {
        Self(self.0.add(rhs))
    }
}

impl AddAssign<ClockDuration> for ClockTime {
    fn add_assign(&mut self, rhs: ClockDuration) {
        *self = Self(self.0.add(rhs));
    }
}

/// Represents a frequency in Hz
///
/// Clocks are usually given as a frequency, but durations are needed when dealing with clocks
/// and clock durations.  This type makes it easier to create a clock of a given frequency and
/// convert it to a `ClockDuration`
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Frequency {
    hertz: u32,
}

impl Frequency {
    #[inline]
    pub const fn from_hz(hertz: u32) -> Self {
        Self {
            hertz,
        }
    }

    #[inline]
    pub const fn from_khz(khz: u32) -> Self {
        Self {
            hertz: khz * 1_000,
        }
    }

    #[inline]
    pub const fn from_mhz(mhz: u32) -> Self {
        Self {
            hertz: mhz * 1_000_000,
        }
    }

    #[inline]
    pub const fn as_hz(self) -> u32 {
        self.hertz
    }

    #[inline]
    pub const fn as_khz(self) -> u32 {
        self.hertz / 1_000
    }

    #[inline]
    pub const fn as_mhz(self) -> u32 {
        self.hertz / 1_000_000
    }

    #[inline]
    pub const fn period_duration(self) -> ClockDuration {
        ClockDuration::from_femtos(ClockDuration::FEMTOS_PER_SEC / self.hertz as Femtos)
    }
}

impl Mul<u32> for Frequency {
    type Output = Self;

    fn mul(self, rhs: u32) -> Self::Output {
        Self::from_hz(self.hertz * rhs)
    }
}

impl MulAssign<u32> for Frequency {
    fn mul_assign(&mut self, rhs: u32) {
        *self = Self::from_hz(self.hertz * rhs);
    }
}

impl Div<u32> for Frequency {
    type Output = Self;

    fn div(self, rhs: u32) -> Self::Output {
        Self::from_hz(self.hertz / rhs)
    }
}

impl DivAssign<u32> for Frequency {
    fn div_assign(&mut self, rhs: u32) {
        *self = Self::from_hz(self.hertz / rhs);
    }
}

