
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ErrorType {
    Assertion,
    Emulator,
    Processor,
    Breakpoint,
}

#[derive(Debug)]
pub struct Error {
    pub err: ErrorType,
    pub native: u32,
    pub msg: String,
}

impl Error {
    pub fn new(msg: &str) -> Error {
        Error {
            err: ErrorType::Emulator,
            native: 0,
            msg: msg.to_string(),
        }
    }

    pub fn processor(native: u32) -> Error {
        Error {
            err: ErrorType::Processor,
            native,
            msg: "".to_string(),
        }
    }

    pub fn breakpoint(msg: &str) -> Error {
        Error {
            err: ErrorType::Breakpoint,
            native: 0,
            msg: msg.to_string(),
        }
    }

    pub fn assertion(msg: &str) -> Error {
        Error {
            err: ErrorType::Assertion,
            native: 0,
            msg: msg.to_string(),
        }
    }
}

