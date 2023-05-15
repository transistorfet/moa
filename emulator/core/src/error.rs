
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ErrorType {
    Assertion,
    Emulator(EmulatorErrorKind),
    Processor,
    Breakpoint,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EmulatorErrorKind {
    Misc,
    MemoryAlignment,
}


#[derive(Debug)]
pub struct Error {
    pub err: ErrorType,
    pub native: u32,
    pub msg: String,
}

impl Error {
    pub fn new<S>(msg: S) -> Error
    where
        S: Into<String>,
    {
        Error {
            err: ErrorType::Emulator(EmulatorErrorKind::Misc),
            native: 0,
            msg: msg.into(),
        }
    }

    pub fn emulator<S>(kind: EmulatorErrorKind, msg: S) -> Error
    where
        S: Into<String>,
    {
        Error {
            err: ErrorType::Emulator(kind),
            native: 0,
            msg: msg.into(),
        }
    }

    pub fn processor(native: u32) -> Error {
        Error {
            err: ErrorType::Processor,
            native,
            msg: "".to_string(),
        }
    }

    pub fn breakpoint<S>(msg: S) -> Error
    where
        S: Into<String>,
    {
        Error {
            err: ErrorType::Breakpoint,
            native: 0,
            msg: msg.into(),
        }
    }

    pub fn assertion<S>(msg: S) -> Error
    where
        S: Into<String>,
    {
        Error {
            err: ErrorType::Assertion,
            native: 0,
            msg: msg.into(),
        }
    }
}

