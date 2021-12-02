
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ControllerDevice {
    A,
    B,
    C,
    D,
}

pub struct Controller {
    //pub dpad_up: bool,
    //pub dpad_down: bool,
    //pub dpad_left: bool,
    //pub dpad_right: bool,

    // TODO this is temporary until I actually implement this properly
    pub bits: u16,
}

