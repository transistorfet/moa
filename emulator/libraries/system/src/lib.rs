#[macro_use]
mod error;

mod bus;
mod devices;
mod interrupts;
mod system;

pub use crate::devices::{
    Address, Device,
    DeviceInterface, MoaBus, MoaStep,
};
pub use crate::error::Error;
pub use crate::interrupts::InterruptController;
pub use crate::bus::{Bus, dump_slice, dump_memory};
pub use crate::system::System;

pub use emulator_hal;
pub use emulator_hal::BusAdapter;
