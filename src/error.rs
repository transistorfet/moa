
#[derive(Debug)]
pub enum ErrorType {
    Emulator,
    Processor,
    Internal,
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
}

macro_rules! debug {
    ($($arg:tt)*) => ({
        println!($($arg)*);
    })
}

