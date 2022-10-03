
mod settings;
mod frontend;
pub use crate::frontend::{PixelsFrontend, LoadSystemFn};

#[cfg(target_arch = "wasm32")]
pub mod web;
#[cfg(target_arch = "wasm32")]
pub use crate::web::{start};

#[cfg(not(target_arch = "wasm32"))]
pub mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use crate::native::{start};

