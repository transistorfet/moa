#[macro_use]
mod error;

mod devices;
mod interrupts;
mod memory;
mod system;

pub use crate::devices::{
    Address, Device,
    DeviceInterface, MoaBus, MoaStep,
};
pub use crate::error::Error;
pub use crate::interrupts::InterruptController;
pub use crate::memory::{MemoryBlock, Bus, dump_slice, dump_memory};
pub use crate::system::System;

pub use emulator_hal;
pub use emulator_hal::BusAdapter;
