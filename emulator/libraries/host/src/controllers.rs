#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ControllerDevice {
    A,
    B,
    C,
    D,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ControllerInput {
    DpadUp(bool),
    DpadDown(bool),
    DpadLeft(bool),
    DpadRight(bool),
    ButtonA(bool),
    ButtonB(bool),
    ButtonC(bool),
    ButtonX(bool),
    ButtonY(bool),
    ButtonZ(bool),
    Start(bool),
    Mode(bool),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ControllerEvent {
    pub device: ControllerDevice,
    pub input: ControllerInput,
}

impl ControllerEvent {
    pub fn new(device: ControllerDevice, input: ControllerInput) -> Self {
        Self {
            device,
            input,
        }
    }
}
