
use minifb::Key as MiniKey;
use moa_core::host::ControllerEvent;

pub fn map_controller_a(key: MiniKey, state: bool) -> Option<ControllerEvent> {
    match key {
        MiniKey::A => { Some(ControllerEvent::ButtonA(state)) },
        MiniKey::O => { Some(ControllerEvent::ButtonB(state)) },
        MiniKey::E => { Some(ControllerEvent::ButtonC(state)) },
        MiniKey::Up => { Some(ControllerEvent::DpadUp(state)) },
        MiniKey::Down => { Some(ControllerEvent::DpadDown(state)) },
        MiniKey::Left => { Some(ControllerEvent::DpadLeft(state)) },
        MiniKey::Right => { Some(ControllerEvent::DpadRight(state)) },
        MiniKey::Enter => { Some(ControllerEvent::Start(state)) },
        MiniKey::M => { Some(ControllerEvent::Mode(state)) },
        _ => None,
    }
}

