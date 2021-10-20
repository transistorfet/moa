
use moa::error::Error;
use moa::host::frontend::{Frontend, SharedCanvas, SharedAudio};

pub struct ConsoleFrontend;

impl Frontend for ConsoleFrontend {
    fn get_canvas(&mut self) -> Result<SharedCanvas, Error> {
        Err(Error::new("Console frontend doesn't support canvas"))
    }

    fn get_audio(&mut self) -> Result<SharedAudio, Error> {
        Err(Error::new("Console frontend doesn't support audio"))
    }
}

