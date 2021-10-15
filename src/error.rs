
#[derive(Debug)]
pub enum ErrorType {
    Emulator,
    Processor,
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

static mut DEBUG_ENABLE: bool = true;

pub fn debug_enabled() -> bool {
    unsafe { DEBUG_ENABLE }
}

macro_rules! debug {
    ($($arg:tt)*) => ({
        if crate::error::debug_enabled() {
            println!($($arg)*);
        }
    })
}

