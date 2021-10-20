
use std::sync::{Arc, Mutex};

use crate::error::Error;

pub trait Canvas {
    fn draw_bitmap(&mut self, x: u32, y: u32, bitmap: &[u8]);
}

pub trait Audio {

}

pub type SharedCanvas = Arc<Mutex<Box<dyn Canvas>>>;
pub type SharedAudio = Arc<Mutex<Box<dyn Audio>>>;

pub trait Frontend {
    fn get_canvas(&mut self) -> Result<SharedCanvas, Error>;
    fn get_audio(&mut self) -> Result<SharedAudio, Error>;
}

