use core::fmt;
use core::convert::Infallible;
use moa_host::HostError;
use emulator_hal::Error as EmuError;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EmulatorErrorKind {
    Misc,
    MemoryAlignment,
}

#[derive(Clone, Debug)]
pub enum Error {
    Assertion(String),
    Breakpoint(String),
    Emulator(EmulatorErrorKind, String),
    Processor(u32),
    Other(String),
}

impl Default for Error {
    fn default() -> Self {
        Self::Other("default".to_string())
    }
}

impl From<Infallible> for Error {
    fn from(err: Infallible) -> Self {
        Error::new("infallible".to_string())
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

/*
impl<E> From<E> for Error
where
    E: EmuError,
{
    fn from(err: E) -> Self {
        Error::new(format!("{}", err))
    }
}
*/

impl Error {
    pub fn new<S>(msg: S) -> Error
    where
        S: Into<String>,
    {
        Error::Emulator(EmulatorErrorKind::Misc, msg.into())
    }

    pub fn emulator<S>(kind: EmulatorErrorKind, msg: S) -> Error
    where
        S: Into<String>,
    {
        Error::Emulator(kind, msg.into())
    }

    pub fn processor(native: u32) -> Error {
        Error::Processor(native)
    }

    pub fn breakpoint<S>(msg: S) -> Error
    where
        S: Into<String>,
    {
        Error::Breakpoint(msg.into())
    }

    pub fn assertion<S>(msg: S) -> Error
    where
        S: Into<String>,
    {
        Error::Assertion(msg.into())
    }

    pub fn msg(&self) -> &str {
        match self {
            Error::Assertion(msg) | Error::Breakpoint(msg) | Error::Other(msg) | Error::Emulator(_, msg) => msg.as_str(),
            Error::Processor(_) => "native exception",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Assertion(msg) | Error::Breakpoint(msg) | Error::Other(msg) | Error::Emulator(_, msg) => write!(f, "{}", msg),
            Error::Processor(_) => write!(f, "native exception"),
        }
    }
}

impl<E> From<HostError<E>> for Error {
    fn from(_err: HostError<E>) -> Self {
        Self::Other("other".to_string())
    }
}
