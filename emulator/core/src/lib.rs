#[macro_use]
mod error;

mod devices;
mod interrupts;
mod memory;
mod system;

pub use crate::devices::{
    Address, Addressable, Steppable, Interruptable, Debuggable, Inspectable, Transmutable, DynDevice, Device,
};
pub use crate::devices::{
    read_beu16, read_beu32, read_leu16, read_leu32, write_beu16, write_beu32, write_leu16, write_leu32, wrap_device
};
pub use crate::error::Error;
pub use crate::interrupts::InterruptController;
pub use crate::memory::{MemoryBlock, AddressTranslator, AddressRepeater, Bus, BusPort, dump_slice, dump_memory};
pub use crate::system::{System, DeviceSettings};

pub use emulator_hal::bus::BusAccess;
