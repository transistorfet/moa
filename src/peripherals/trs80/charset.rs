
const CHARACTERS: [[u8; 8]; 64] = [
// Blank
[ 0b00000,
  0b00000,
  0b00000,
  0b00000,
  0b00000,
  0b00000,
  0b00000,
  0b00000 ],

// Character !
[ 0b00100,
  0b00100,
  0b00100,
  0b00100,
  0b00100,
  0b00000,
  0b00100,
  0b00000 ],

// Character "
[ 0b01010,
  0b01010,
  0b01010,
  0b00000,
  0b00000,
  0b00000,
  0b00000,
  0b00000 ],

// Character #
[ 0b01010,
  0b01010,
  0b11111,
  0b01010,
  0b11111,
  0b01010,
  0b01010,
  0b00000 ],

// Character $
[ 0b01110,
  0b10101,
  0b10100,
  0b01110,
  0b00101,
  0b10101,
  0b01110,
  0b00100 ],


// Character %
[ 0b11001,
  0b11010,
  0b00010,
  0b00100,
  0b01000,
  0b01011,
  0b10011,
  0b00000 ],

// Character &
[ 0b00100,
  0b01010,
  0b01010,
  0b00100,
  0b01100,
  0b10011,
  0b01110,
  0b00000 ],

// Character '
[ 0b01000,
  0b01000,
  0b01000,
  0b00000,
  0b00000,
  0b00000,
  0b00000,
  0b00000 ],

// Character (
[ 0b00100,
  0b01000,
  0b10000,
  0b10000,
  0b10000,
  0b01000,
  0b00100,
  0b00000 ],

// Character )
[ 0b00100,
  0b00010,
  0b00001,
  0b00001,
  0b00001,
  0b00010,
  0b00100,
  0b00000 ],

// Character *
[ 0b00000,
  0b00000,
  0b01010,
  0b00100,
  0b01010,
  0b00000,
  0b00000,
  0b00000 ],

// Character +
[ 0b00000,
  0b00000,
  0b00100,
  0b01110,
  0b00100,
  0b00000,
  0b00000,
  0b00000 ],

// Character ,
[ 0b00000,
  0b00000,
  0b00000,
  0b00000,
  0b00010,
  0b00010,
  0b00100,
  0b00000 ],

// Character -
[ 0b00000,
  0b00000,
  0b00000,
  0b01110,
  0b00000,
  0b00000,
  0b00000,
  0b00000 ],

// Character .
[ 0b00000,
  0b00000,
  0b00000,
  0b00000,
  0b00110,
  0b00110,
  0b00000,
  0b00000 ],

// Character /
[ 0b00010,
  0b00010,
  0b00100,
  0b00100,
  0b01000,
  0b01000,
  0b10000,
  0b00000 ],

// Character 0
[ 0b01100,
  0b10010,
  0b11010,
  0b10110,
  0b10010,
  0b10010,
  0b01100,
  0b00000 ],

// Character 1
[ 0b00100,
  0b01100,
  0b00100,
  0b00100,
  0b00100,
  0b00100,
  0b01110,
  0b00000 ],

// Character 2
[ 0b01100,
  0b10010,
  0b00010,
  0b00100,
  0b01000,
  0b10000,
  0b11110,
  0b00000 ],

// Character 3
[ 0b11100,
  0b00010,
  0b00010,
  0b01100,
  0b00010,
  0b00010,
  0b11100,
  0b00000 ],

// Character 4
[ 0b00010,
  0b00110,
  0b01010,
  0b10010,
  0b11110,
  0b00010,
  0b00010,
  0b00000 ],

// Character 5
[ 0b11110,
  0b10000,
  0b10000,
  0b11110,
  0b00010,
  0b00010,
  0b11100,
  0b00000 ],

// Character 6
[ 0b01100,
  0b10010,
  0b10000,
  0b11110,
  0b10010,
  0b10010,
  0b01100,
  0b00000 ],

// Character 7
[ 0b11110,
  0b00010,
  0b00100,
  0b01000,
  0b01000,
  0b01000,
  0b01000,
  0b00000 ],

// Character 8
[ 0b01100,
  0b10010,
  0b10010,
  0b01100,
  0b10010,
  0b10010,
  0b01100,
  0b00000 ],

// Character 9
[ 0b01100,
  0b10010,
  0b10010,
  0b01110,
  0b00010,
  0b00010,
  0b01100,
  0b00000 ],

// Character :
[ 0b00000,
  0b00000,
  0b01100,
  0b01100,
  0b00000,
  0b01100,
  0b01100,
  0b00000 ],

// Character ;
[ 0b00000,
  0b00000,
  0b01100,
  0b01100,
  0b00000,
  0b00100,
  0b00100,
  0b00000 ],

// Character <
[ 0b00010,
  0b00100,
  0b01000,
  0b10000,
  0b01000,
  0b00100,
  0b00010,
  0b00000 ],

// Character =
[ 0b00000,
  0b00000,
  0b11110,
  0b00000,
  0b11110,
  0b00000,
  0b00000,
  0b00000 ],

// Character >
[ 0b10000,
  0b01000,
  0b00100,
  0b00010,
  0b00100,
  0b01000,
  0b10000,
  0b00000 ],

// Character ?
[ 0b01100,
  0b10010,
  0b00010,
  0b00100,
  0b01000,
  0b00000,
  0b01000,
  0b00000 ],

// Character @
[ 0b01100,
  0b10010,
  0b10010,
  0b10110,
  0b11110,
  0b11110,
  0b01100,
  0b00000 ],


// Letter A
[ 0b01100,
  0b10010,
  0b10010,
  0b11110,
  0b10010,
  0b10010,
  0b10010,
  0b00000 ],

// Letter B
[ 0b11100,
  0b10010,
  0b10010,
  0b11100,
  0b10010,
  0b10010,
  0b11100,
  0b00000 ],

// Letter C
[ 0b01100,
  0b10010,
  0b10000,
  0b10000,
  0b10000,
  0b10010,
  0b01100,
  0b00000 ],

// Letter D
[ 0b11100,
  0b10010,
  0b10010,
  0b10010,
  0b10010,
  0b10010,
  0b11100,
  0b00000 ],

// Letter E
[ 0b11110,
  0b10000,
  0b10000,
  0b11110,
  0b10000,
  0b10000,
  0b11110,
  0b00000 ],

// Letter F
[ 0b11110,
  0b10000,
  0b10000,
  0b11110,
  0b10000,
  0b10000,
  0b10000,
  0b00000 ],

// Letter G
[ 0b01100,
  0b10010,
  0b10000,
  0b10110,
  0b10010,
  0b10010,
  0b01110,
  0b00000 ],

// Letter H
[ 0b10010,
  0b10010,
  0b10010,
  0b11110,
  0b10010,
  0b10010,
  0b10010,
  0b00000 ],

// Letter I
[ 0b00100,
  0b00100,
  0b00100,
  0b00100,
  0b00100,
  0b00100,
  0b00100,
  0b00000 ],

// Letter J
[ 0b00010,
  0b00010,
  0b00010,
  0b00010,
  0b00010,
  0b10010,
  0b01100,
  0b00000 ],

// Letter K
[ 0b10010,
  0b10010,
  0b10100,
  0b11000,
  0b10100,
  0b10010,
  0b10010,
  0b00000 ],

// Letter L
[ 0b10000,
  0b10000,
  0b10000,
  0b10000,
  0b10000,
  0b10000,
  0b11110,
  0b00000 ],

// Letter M
[ 0b00000,
  0b10001,
  0b11011,
  0b10101,
  0b10001,
  0b10001,
  0b10001,
  0b00000 ],

// Letter N
[ 0b00000,
  0b10010,
  0b11010,
  0b10110,
  0b10010,
  0b10010,
  0b10010,
  0b00000 ],

// Letter O
[ 0b01100,
  0b10010,
  0b10010,
  0b10010,
  0b10010,
  0b10010,
  0b01100,
  0b00000 ],

// Letter P
[ 0b11100,
  0b10010,
  0b10010,
  0b11100,
  0b10000,
  0b10000,
  0b10000,
  0b00000 ],

// Letter Q
[ 0b01100,
  0b10010,
  0b10010,
  0b10010,
  0b10010,
  0b10110,
  0b01110,
  0b00000 ],

// Letter R
[ 0b11100,
  0b10010,
  0b10010,
  0b11100,
  0b10010,
  0b10010,
  0b10010,
  0b00000 ],

// Letter S
[ 0b01110,
  0b10000,
  0b10000,
  0b01100,
  0b00010,
  0b00010,
  0b11100,
  0b00000 ],

// Letter T
[ 0b11111,
  0b00100,
  0b00100,
  0b00100,
  0b00100,
  0b00100,
  0b00100,
  0b00000 ],

// Letter U
[ 0b10010,
  0b10010,
  0b10010,
  0b10010,
  0b10010,
  0b10010,
  0b01100,
  0b00000 ],

// Letter V
[ 0b10001,
  0b10001,
  0b10001,
  0b10001,
  0b10001,
  0b01010,
  0b00100,
  0b00000 ],

// Letter W
[ 0b10001,
  0b10001,
  0b10001,
  0b10001,
  0b10101,
  0b10101,
  0b01010,
  0b00000 ],

// Letter X
[ 0b00000,
  0b10001,
  0b01010,
  0b00100,
  0b00100,
  0b01010,
  0b10001,
  0b00000 ],

// Letter Y
[ 0b10001,
  0b10001,
  0b10001,
  0b01010,
  0b00100,
  0b00100,
  0b00100,
  0b00000 ],

// Letter Z
[ 0b11110,
  0b00010,
  0b00100,
  0b01000,
  0b10000,
  0b10000,
  0b11110,
  0b00000 ],


//////// LEFT TO DO ///////

// Blank
[ 0b00000,
  0b00000,
  0b00000,
  0b00000,
  0b00000,
  0b00000,
  0b00000,
  0b00000 ],

// Blank
[ 0b00000,
  0b00000,
  0b00000,
  0b00000,
  0b00000,
  0b00000,
  0b00000,
  0b00000 ],

// Blank
[ 0b00000,
  0b00000,
  0b00000,
  0b00000,
  0b00000,
  0b00000,
  0b00000,
  0b00000 ],

// Blank
[ 0b00000,
  0b00000,
  0b00000,
  0b00000,
  0b00000,
  0b00000,
  0b00000,
  0b00000 ],

// Blank
[ 0b00000,
  0b00000,
  0b00000,
  0b00000,
  0b00000,
  0b00000,
  0b00000,
  0b00000 ],



];



pub struct CharacterGenerator {
    pub row: i8,
    pub col: i8,
    pub data: &'static [u8; 8],
}

impl CharacterGenerator {
    pub fn new(ch: u8) -> Self {
        Self {
            row: 0,
            col: 4,
            data: &CHARACTERS[ch as usize],
        }
    }
}

impl Iterator for CharacterGenerator {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.row >= 8 {
            None
        } else {
            let bit = (self.data[self.row as usize] & (1 << self.col)) != 0;

            self.col -= 1;
            if self.col < 0 {
                self.col = 4;
                self.row += 1;
            }

            if bit {
                Some(0xC0C0C0)
            } else {
                Some(0)
            }
        }
    }
}

