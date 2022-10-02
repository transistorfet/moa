 
#[macro_use]
mod error;

mod debugger;
mod devices;
mod interrupts;
mod memory;
mod signals;
mod system;

pub mod host;
pub mod parser;
pub mod timers;

pub use crate::debugger::Debugger;
pub use crate::devices::{Clock, ClockElapsed, Address, Addressable, Steppable, Interruptable, Debuggable, Inspectable, Transmutable, TransmutableBox};
pub use crate::devices::{read_beu16, read_beu32, read_leu16, read_leu32, write_beu16, write_beu32, write_leu16, write_leu32, wrap_transmutable};
pub use crate::error::{Error, ErrorType, LogLevel, log_level};
pub use crate::interrupts::InterruptController;
pub use crate::memory::{MemoryBlock, AddressRightShifter, AddressRepeater, Bus, BusPort, dump_slice};
pub use crate::signals::{Observable, Signal, EdgeSignal, ObservableSignal, ObservableEdgeSignal};
pub use crate::system::System;

