
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ControllerDevice {
    A,
    B,
    C,
    D,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ControllerEvent {
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

