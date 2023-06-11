
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
}

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
            Error::Assertion(msg) |
            Error::Breakpoint(msg) |
            Error::Emulator(_, msg) => msg.as_str(),
            Error::Processor(_) => "native exception",
        }
    }
}

