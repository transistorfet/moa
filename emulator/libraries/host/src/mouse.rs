#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MouseEventType {
    Down(MouseButton),
    Up(MouseButton),
    Move,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MouseEvent {
    pub etype: MouseEventType,
    pub pos: (u32, u32),
}

#[derive(Copy, Clone, Default, PartialEq, Eq)]
pub struct MouseState {
    pub buttons: [bool; 3],
    pub pos: (u32, u32),
}

impl MouseEvent {
    pub fn new(etype: MouseEventType, pos: (u32, u32)) -> Self {
        Self {
            etype,
            pos,
        }
    }
}

impl From<usize> for MouseButton {
    fn from(index: usize) -> MouseButton {
        match index {
            0 => MouseButton::Left,
            1 => MouseButton::Right,
            2 => MouseButton::Middle,
            _ => panic!("unexpected mouse button index: {:?}", index),
        }
    }
}

impl From<MouseButton> for usize {
    fn from(button: MouseButton) -> usize {
        match button {
            MouseButton::Left => 0,
            MouseButton::Right => 1,
            MouseButton::Middle => 2,
        }
    }
}

impl MouseState {
    pub fn with(left: bool, right: bool, middle: bool, x: u32, y: u32) -> Self {
        Self {
            buttons: [left, right, middle],
            pos: (x, y),
        }
    }

    pub fn to_events(&mut self, next_state: MouseState) -> Vec<MouseEvent> {
        if *self != next_state {
            self.pos = next_state.pos;

            let events: Vec<MouseEvent> = self
                .buttons
                .into_iter()
                .zip(next_state.buttons)
                .enumerate()
                .filter_map(|(i, (prev, next))| {
                    if prev != next {
                        self.buttons[i] = next;
                        let button = MouseButton::from(i);
                        let etype = if next {
                            MouseEventType::Down(button)
                        } else {
                            MouseEventType::Up(button)
                        };
                        Some(MouseEvent::new(etype, next_state.pos))
                    } else {
                        None
                    }
                })
                .collect();

            if !events.is_empty() {
                events
            } else {
                vec![MouseEvent::new(MouseEventType::Move, next_state.pos)]
            }
        } else {
            vec![]
        }
    }

    pub fn update_with(&mut self, event: MouseEvent) {
        self.pos = event.pos;
        match event.etype {
            MouseEventType::Up(button) => self.buttons[usize::from(button)] = false,
            MouseEventType::Down(button) => self.buttons[usize::from(button)] = true,
            MouseEventType::Move => {},
        }
    }
}
