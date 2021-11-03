
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ErrorType {
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
}


#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub enum LogLevel {
    Error,
    Warning,
    Info,
    Debug,
}

static mut LOG_LEVEL: LogLevel = LogLevel::Info;

pub fn log_level() -> LogLevel {
    unsafe { LOG_LEVEL }
}

macro_rules! printlog {
    ($level:expr, $($arg:tt)*) => ({
        if $level <= crate::error::log_level() {
            println!($($arg)*);
        }
    })
}

macro_rules! error {
    ($($arg:tt)*) => ({
        printlog!(crate::error::LogLevel::Error, $($arg)*);
    })
}

macro_rules! warning {
    ($($arg:tt)*) => ({
        printlog!(crate::error::LogLevel::Warning, $($arg)*);
    })
}

macro_rules! info {
    ($($arg:tt)*) => ({
        printlog!(crate::error::LogLevel::Info, $($arg)*);
    })
}

macro_rules! debug {
    ($($arg:tt)*) => ({
        printlog!(crate::error::LogLevel::Debug, $($arg)*);
    })
}

