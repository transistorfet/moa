#![doc = include_str!("../README.md")]
#![warn(missing_docs)]
#![no_std]

use core::time;
use core::ops::{Add, AddAssign, Sub, SubAssign, Mul, MulAssign, Div, DivAssign};

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
/// The `Duration` type is used to represent lengths of time and is
/// intentionally similar to `std::time::Duration`, but which records
/// time as femtoseconds to keep accurancy when dealing with partial
/// nanosecond clock divisons.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Duration {
    femtos: Femtos,
}

impl Duration {
    /// A duration of zero (0) time
    pub const ZERO: Self = Self::from_femtos(0);

    /// A duration of the maximum possible length in femtoseconds (`Femtos::MAX`)
    ///
    /// This will be equivalent to either u64::MAX or u128::MAX femtoseconds
    pub const MAX: Self = Self::from_femtos(Femtos::MAX);

    /// The number of femtoseconds in 1 second as `Femtos`
    pub const FEMTOS_PER_SEC: Femtos = 1_000_000_000_000_000;

    /// The number of femtoseconds in 1 millisecond as `Femtos`
    pub const FEMTOS_PER_MILLISEC: Femtos = 1_000_000_000_000;

    /// The number of femtoseconds in 1 microsecond as `Femtos`
    pub const FEMTOS_PER_MICROSEC: Femtos = 1_000_000_000;

    /// The number of femtoseconds in 1 nanosecond as `Femtos`
    pub const FEMTOS_PER_NANOSEC: Femtos = 1_000_000;

    /// The number of femtoseconds in 1 picosecond as `Femtos`
    pub const FEMTOS_PER_PICOSEC: Femtos = 1_000;

    /// Creates a new `Duration` from the specified number of seconds
    ///
    /// # Examples
    ///
    /// ```
    /// use femtos::Duration;
    ///
    /// let duration = Duration::from_secs(123);
    ///
    /// assert_eq!(123, duration.as_secs());
    /// ```
    #[inline]
    pub const fn from_secs(secs: u64) -> Self {
        Self {
            femtos: secs as Femtos * Self::FEMTOS_PER_SEC,
        }
    }

    /// Creates a new `Duration` from the specified number of milliseconds
    ///
    /// # Examples
    ///
    /// ```
    /// use femtos::Duration;
    ///
    /// let duration = Duration::from_millis(123);
    ///
    /// assert_eq!(123, duration.as_millis());
    /// ```
    #[inline]
    pub const fn from_millis(millisecs: u64) -> Self {
        Self {
            femtos: millisecs as Femtos * Self::FEMTOS_PER_MILLISEC,
        }
    }

    /// Creates a new `Duration` from the specified number of microseconds
    ///
    /// # Examples
    ///
    /// ```
    /// use femtos::Duration;
    ///
    /// let duration = Duration::from_micros(123);
    ///
    /// assert_eq!(123, duration.as_micros());
    /// ```
    #[inline]
    pub const fn from_micros(microsecs: u64) -> Self {
        Self {
            femtos: microsecs as Femtos * Self::FEMTOS_PER_MICROSEC,
        }
    }

    /// Creates a new `Duration` from the specified number of nanoseconds
    ///
    /// # Examples
    ///
    /// ```
    /// use femtos::Duration;
    ///
    /// let duration = Duration::from_nanos(123);
    ///
    /// assert_eq!(123, duration.as_nanos());
    /// ```
    #[inline]
    pub const fn from_nanos(nanosecs: u64) -> Self {
        Self {
            femtos: nanosecs as Femtos * Self::FEMTOS_PER_NANOSEC,
        }
    }

    /// Creates a new `Duration` from the specified number of picoseconds
    ///
    /// # Examples
    ///
    /// ```
    /// use femtos::Duration;
    ///
    /// let duration = Duration::from_picos(123);
    ///
    /// assert_eq!(123, duration.as_picos());
    /// ```
    #[inline]
    pub const fn from_picos(picosecs: u128) -> Self {
        Self {
            femtos: picosecs as Femtos * Self::FEMTOS_PER_PICOSEC,
        }
    }

    /// Creates a new `Duration` from the specified number of femtoseconds
    ///
    /// # Examples
    ///
    /// ```
    /// use femtos::Duration;
    ///
    /// let duration = Duration::from_femtos(123);
    ///
    /// assert_eq!(123, duration.as_femtos());
    /// ```
    #[inline]
    pub const fn from_femtos(femtos: Femtos) -> Self {
        Self {
            femtos,
        }
    }

    /// Returns the number of _whole_ seconds contained by this `Duration`.
    ///
    /// # Examples
    ///
    /// ```
    /// use femtos::Duration;
    ///
    /// let duration = Duration::from_femtos(123_465_789_012_345_678);
    /// assert_eq!(duration.as_secs(), 123);
    /// ```
    #[inline]
    pub const fn as_secs(self) -> u64 {
        (self.femtos / Self::FEMTOS_PER_SEC) as u64
    }

    /// Returns the number of _whole_ milliseconds contained by this `Duration`.
    ///
    /// # Examples
    ///
    /// ```
    /// use femtos::Duration;
    ///
    /// let duration = Duration::from_femtos(123_465_789_012_345_678);
    /// assert_eq!(duration.as_millis(), 123_465);
    /// ```
    #[inline]
    pub const fn as_millis(self) -> u64 {
        (self.femtos / Self::FEMTOS_PER_MILLISEC) as u64
    }

    /// Returns the number of _whole_ microseconds contained by this `Duration`.
    ///
    /// # Examples
    ///
    /// ```
    /// use femtos::Duration;
    ///
    /// let duration = Duration::from_femtos(123_465_789_012_345_678);
    /// assert_eq!(duration.as_micros(), 123_465_789);
    /// ```
    #[inline]
    pub const fn as_micros(self) -> u64 {
        (self.femtos / Self::FEMTOS_PER_MICROSEC) as u64
    }

    /// Returns the number of _whole_ nanoseconds contained by this `Duration`.
    ///
    /// # Examples
    ///
    /// ```
    /// use femtos::Duration;
    ///
    /// let duration = Duration::from_femtos(123_465_789_012_345_678);
    /// assert_eq!(duration.as_nanos(), 123_465_789_012);
    /// ```
    #[inline]
    pub const fn as_nanos(self) -> u64 {
        (self.femtos / Self::FEMTOS_PER_NANOSEC) as u64
    }

    /// Returns the number of _whole_ picoseconds contained by this `Duration`.
    ///
    /// # Examples
    ///
    /// ```
    /// use femtos::Duration;
    ///
    /// let duration = Duration::from_femtos(123_465_789_012_345_678);
    /// assert_eq!(duration.as_picos(), 123_465_789_012_345);
    /// ```
    #[inline]
    #[allow(clippy::unnecessary_cast)]
    pub const fn as_picos(self) -> u128 {
        (self.femtos / Self::FEMTOS_PER_PICOSEC) as u128
    }

    /// Returns the number of _whole_ femtoseconds contained by this `Duration`.
    ///
    /// # Examples
    ///
    /// ```
    /// use femtos::Duration;
    ///
    /// let duration = Duration::from_femtos(123_465_789_012_345_678);
    /// assert_eq!(duration.as_femtos(), 123_465_789_012_345_678);
    /// ```
    #[inline]
    pub const fn as_femtos(self) -> Femtos {
        self.femtos
    }

    /// Checked `Duration` addition.  Computes `self + rhs`, returning [`None`]
    /// if an overflow occured.
    ///
    /// # Examples
    ///
    /// ```
    /// use femtos::Duration;
    ///
    /// assert_eq!(Duration::from_secs(1).checked_add(Duration::from_secs(1)), Some(Duration::from_secs(2)))
    /// assert_eq!(Duration::from_secs(1).checked_add(Duration::from_femtos(Femtos::MAX)), None)
    /// ```
    #[inline]
    pub const fn checked_add(self, rhs: Self) -> Option<Self> {
        match self.femtos.checked_add(rhs.femtos) {
            Some(femtos) => Some(Self::from_femtos(femtos)),
            None => None,
        }
    }

    /// Checked `Duration` subtraction.  Computes `self - rhs`, returning [`None`]
    /// if an overflow occured.
    ///
    /// # Examples
    ///
    /// ```
    /// use femtos::Duration;
    ///
    /// assert_eq!(Duration::from_secs(1).checked_sub(Duration::from_secs(1)), Some(Duration::ZERO))
    /// assert_eq!(Duration::from_secs(1).checked_sub(Duration::from_femtos(2), None)
    /// ```
    #[inline]
    pub const fn checked_sub(self, rhs: Self) -> Option<Self> {
        match self.femtos.checked_sub(rhs.femtos) {
            Some(femtos) => Some(Self::from_femtos(femtos)),
            None => None,
        }
    }
}

impl Add for Duration {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        self.checked_add(rhs).expect("clock duration overflow during addition")
    }
}

impl AddAssign for Duration {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for Duration {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        self.checked_sub(rhs).expect("clock duration overflow during subtraction")
    }
}

impl SubAssign for Duration {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl Mul<u64> for Duration {
    type Output = Self;

    fn mul(self, rhs: u64) -> Self::Output {
        Self::from_femtos(self.femtos * rhs as Femtos)
    }
}

impl MulAssign<u64> for Duration {
    fn mul_assign(&mut self, rhs: u64) {
        *self = Self::from_femtos(self.femtos * rhs as Femtos);
    }
}

impl Div<u64> for Duration {
    type Output = Self;

    fn div(self, rhs: u64) -> Self::Output {
        Self::from_femtos(self.femtos / rhs as Femtos)
    }
}

impl DivAssign<u64> for Duration {
    fn div_assign(&mut self, rhs: u64) {
        *self = Self::from_femtos(self.femtos / rhs as Femtos);
    }
}

impl Div<Duration> for Duration {
    type Output = u64;

    fn div(self, rhs: Duration) -> Self::Output {
        (self.femtos / rhs.femtos) as u64
    }
}


impl From<Duration> for time::Duration {
    fn from(value: Duration) -> Self {
        time::Duration::from_nanos(value.as_nanos())
    }
}

impl From<time::Duration> for Duration {
    fn from(value: time::Duration) -> Self {
        Duration::from_nanos(value.as_nanos() as u64)
    }
}


/// Represents time from the start of the simulation
///
/// `Instant` is for representing the current running clock.  It uses a
/// duration to represent the time from simulation start, and is monotonic.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Instant(Duration);

impl Instant {
    /// An `Instant` representing the start of time (t = 0)
    pub const START: Self = Self(Duration::ZERO);
    /// An `Instant` representing the greatest possible time (t = `Femtos::MAX`)
    pub const FOREVER: Self = Self(Duration::MAX);

    /// Returns a `Duration` equivalent to the amount of time elapsed since the earliest
    /// possible time (t = 0).
    #[inline]
    pub const fn as_duration(self) -> Duration {
        self.0
    }

    /// Returns the `Duration` that has elapsed between this `Instant` and `other`.
    ///
    /// # Examples
    ///
    /// ```
    /// use femtos::{Instant, Duration};
    ///
    /// let now = Instant::START + Duration::from_secs(1);
    /// assert_eq!(now.duration_since(Instant::START), Duration::from_secs(1));
    /// ```
    #[inline]
    pub fn duration_since(self, other: Self) -> Duration {
        self.0 - other.0
    }

    /// Checked `Instant` addition.  Computes `self + duration`, returning [`None`]
    /// if an overflow occured.
    ///
    /// # Examples
    ///
    /// ```
    /// use femtos::Instant;
    ///
    /// assert_eq!(Instant::START.checked_add(Duration::from_secs(1)).as_duration(), Some(Duration::from_secs(1)))
    /// assert_eq!(Instant::START.checked_add(Duration::from_femtos(Femtos::MAX)).as_duration(), None)
    /// ```
    #[inline]
    pub const fn checked_add(self, duration: Duration) -> Option<Self> {
        match self.0.checked_add(duration) {
            Some(duration) => Some(Self(duration)),
            None => None,
        }
    }

    /// Checked `Instant` subtraction.  Computes `self - duration`, returning [`None`]
    /// if an overflow occured.
    ///
    /// # Examples
    ///
    /// ```
    /// use femtos::Instant;
    ///
    /// assert_eq!(Instant::FOREVER.checked_sub(Duration::from_secs(1)).as_duration(), Some(Duration::from_femtos(Femtos::MAX - 1)))
    /// assert_eq!(Instant::START.checked_sub(Duration::from_secs(1)).as_duration(), None)
    /// ```
    #[inline]
    pub const fn checked_sub(self, duration: Duration) -> Option<Self> {
        match self.0.checked_sub(duration) {
            Some(duration) => Some(Self(duration)),
            None => None,
        }
    }
}

impl Add<Duration> for Instant {
    type Output = Self;

    fn add(self, rhs: Duration) -> Self::Output {
        Self(self.0.add(rhs))
    }
}

impl AddAssign<Duration> for Instant {
    fn add_assign(&mut self, rhs: Duration) {
        *self = Self(self.0.add(rhs));
    }
}

/// Represents a frequency in Hz
///
/// Clocks are usually given as a frequency, but durations are needed when dealing with clocks
/// and clock durations.  This type makes it easier to create a clock of a given frequency and
/// convert it to a `Duration`
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Frequency {
    hertz: u32,
}

impl Frequency {
    /// Creates a new `Frequency` from the specified number of hertz
    ///
    /// # Examples
    ///
    /// ```
    /// use femtos::Frequency;
    ///
    /// Frequency::from_hz(123);
    /// ```
    #[inline]
    pub const fn from_hz(hertz: u32) -> Self {
        Self {
            hertz,
        }
    }

    /// Creates a new `Frequency` from the specified number of kilohertz
    ///
    /// # Examples
    ///
    /// ```
    /// use femtos::Frequency;
    ///
    /// Frequency::from_khz(123);
    /// ```
    #[inline]
    pub const fn from_khz(khz: u32) -> Self {
        Self {
            hertz: khz * 1_000,
        }
    }

    /// Creates a new `Frequency` from the specified number of megahertz
    ///
    /// # Examples
    ///
    /// ```
    /// use femtos::Frequency;
    ///
    /// Frequency::from_mhz(123);
    /// ```
    #[inline]
    pub const fn from_mhz(mhz: u32) -> Self {
        Self {
            hertz: mhz * 1_000_000,
        }
    }

    /// Returns the frequency is hertz
    #[inline]
    pub const fn as_hz(self) -> u32 {
        self.hertz
    }

    /// Returns the frequency is kilohertz
    #[inline]
    pub const fn as_khz(self) -> u32 {
        self.hertz / 1_000
    }

    /// Returns the frequency is megahertz
    #[inline]
    pub const fn as_mhz(self) -> u32 {
        self.hertz / 1_000_000
    }

    /// Returns the `Duration` equivalent to the time period between cycles of
    /// the given `Frequency`
    ///
    /// # Examples
    ///
    /// ```
    /// use femtos::Frequency;
    ///
    /// assert_eq!(Frequency::from_hz(1).period_duration(), Duration::from_secs(1));
    /// ```
    #[inline]
    pub const fn period_duration(self) -> Duration {
        Duration::from_femtos(Duration::FEMTOS_PER_SEC / self.hertz as Femtos)
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

