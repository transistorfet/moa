 
#[macro_use]
mod error;

mod debugger;
mod devices;
mod interrupts;
mod memory;
mod signals;
mod system;

pub mod host;

pub use crate::debugger::{DebugControl, Debugger};
pub use crate::devices::{Address, Addressable, Steppable, Interruptable, Debuggable, Inspectable, Transmutable, TransmutableBox, Device};
pub use crate::devices::{read_beu16, read_beu32, read_leu16, read_leu32, write_beu16, write_beu32, write_leu16, write_leu32, wrap_transmutable};
pub use crate::error::Error;
pub use crate::interrupts::InterruptController;
pub use crate::memory::{MemoryBlock, AddressTranslator, AddressRepeater, Bus, BusPort, dump_slice};
pub use crate::signals::{Observable, Signal, EdgeSignal, ObservableSignal, ObservableEdgeSignal};
pub use crate::system::System;

