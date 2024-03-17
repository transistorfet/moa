use minifb::Key as MiniKey;
use moa_host::ControllerInput;

pub fn map_controller_a(key: MiniKey, state: bool) -> Option<ControllerInput> {
    match key {
        MiniKey::A => Some(ControllerInput::ButtonA(state)),
        MiniKey::O => Some(ControllerInput::ButtonB(state)),
        MiniKey::E => Some(ControllerInput::ButtonC(state)),
        MiniKey::Up => Some(ControllerInput::DpadUp(state)),
        MiniKey::Down => Some(ControllerInput::DpadDown(state)),
        MiniKey::Left => Some(ControllerInput::DpadLeft(state)),
        MiniKey::Right => Some(ControllerInput::DpadRight(state)),
        MiniKey::Enter => Some(ControllerInput::Start(state)),
        MiniKey::M => Some(ControllerInput::Mode(state)),
        _ => None,
    }
}
