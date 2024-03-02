
use moa_host::Key;

#[inline(always)]
pub fn set_bit(data: &mut [u8; 8], index: usize, bit: u8, state: bool) {
    let mask = 1 << bit;
    data[index] = (data[index] & !mask) | (if state { mask } else { 0 });
}

pub fn record_key_press(data: &mut [u8; 8], key: Key, state: bool) {
    match key {
        Key::Backquote =>       set_bit(data, 0, 0, state),
        Key::A =>               set_bit(data, 0, 1, state),
        Key::B =>               set_bit(data, 0, 2, state),
        Key::C =>               set_bit(data, 0, 3, state),
        Key::D =>               set_bit(data, 0, 4, state),
        Key::E =>               set_bit(data, 0, 5, state),
        Key::F =>               set_bit(data, 0, 6, state),
        Key::G =>               set_bit(data, 0, 7, state),
        Key::H =>               set_bit(data, 1, 0, state),
        Key::I =>               set_bit(data, 1, 1, state),
        Key::J =>               set_bit(data, 1, 2, state),
        Key::K =>               set_bit(data, 1, 3, state),
        Key::L =>               set_bit(data, 1, 4, state),
        Key::M =>               set_bit(data, 1, 5, state),
        Key::N =>               set_bit(data, 1, 6, state),
        Key::O =>               set_bit(data, 1, 7, state),
        Key::P =>               set_bit(data, 2, 0, state),
        Key::Q =>               set_bit(data, 2, 1, state),
        Key::R =>               set_bit(data, 2, 2, state),
        Key::S =>               set_bit(data, 2, 3, state),
        Key::T =>               set_bit(data, 2, 4, state),
        Key::U =>               set_bit(data, 2, 5, state),
        Key::V =>               set_bit(data, 2, 6, state),
        Key::W =>               set_bit(data, 2, 7, state),
        Key::X =>               set_bit(data, 3, 0, state),
        Key::Y =>               set_bit(data, 3, 1, state),
        Key::Z =>               set_bit(data, 3, 2, state),
        Key::Num0 =>            set_bit(data, 4, 0, state),
        Key::Num1 =>            set_bit(data, 4, 1, state),
        Key::Num2 =>            set_bit(data, 4, 2, state),
        Key::Num3 =>            set_bit(data, 4, 3, state),
        Key::Num4 =>            set_bit(data, 4, 4, state),
        Key::Num5 =>            set_bit(data, 4, 5, state),
        Key::Num6 =>            set_bit(data, 4, 6, state),
        Key::Num7 =>            set_bit(data, 4, 7, state),
        Key::Num8 =>            set_bit(data, 5, 0, state),
        Key::Num9 =>            set_bit(data, 5, 1, state),
        Key::LeftBracket =>     set_bit(data, 5, 2, state),
        Key::RightBracket =>    set_bit(data, 5, 3, state),
        Key::Comma =>           set_bit(data, 5, 4, state),
        Key::Equals =>          set_bit(data, 5, 5, state),
        Key::Period =>          set_bit(data, 5, 6, state),
        Key::Enter =>           set_bit(data, 6, 0, state),
        Key::PrintScreen =>     set_bit(data, 6, 1, state),
        Key::Pause =>           set_bit(data, 6, 2, state),
        Key::Up =>              set_bit(data, 6, 3, state),
        Key::Down =>            set_bit(data, 6, 4, state),
        Key::Left =>            set_bit(data, 6, 5, state),
        Key::Right =>           set_bit(data, 6, 6, state),
        Key::Space =>           set_bit(data, 6, 7, state),
        Key::LeftShift | Key::RightShift => set_bit(data, 7, 0, state),
        _ => { },
    }
}

