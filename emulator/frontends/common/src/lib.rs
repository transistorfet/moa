#[cfg(feature = "tty")]
pub mod tty;

pub mod audio;
pub use crate::audio::{AudioMixer, AudioSource};

#[cfg(feature = "audio")]
pub mod cpal;
#[cfg(feature = "audio")]
pub use crate::cpal::CpalAudioOutput;
