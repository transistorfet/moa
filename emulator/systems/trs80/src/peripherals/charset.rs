
const CHARACTERS: [[u8; 8]; 64] = [
// Blank
[ 0b000000,
  0b000000,
  0b000000,
  0b000000,
  0b000000,
  0b000000,
  0b000000,
  0b000000 ],

// Character !
[ 0b001000,
  0b001000,
  0b001000,
  0b001000,
  0b001000,
  0b000000,
  0b001000,
  0b000000 ],

// Character "
[ 0b010100,
  0b010100,
  0b010100,
  0b000000,
  0b000000,
  0b000000,
  0b000000,
  0b000000 ],

// Character #
[ 0b010100,
  0b010100,
  0b111110,
  0b010100,
  0b111110,
  0b010100,
  0b010100,
  0b000000 ],

// Character $
[ 0b011100,
  0b101010,
  0b101000,
  0b011100,
  0b001010,
  0b101010,
  0b011100,
  0b001000 ],


// Character %
[ 0b110010,
  0b110100,
  0b000100,
  0b001000,
  0b010000,
  0b010110,
  0b100110,
  0b000000 ],

// Character &
[ 0b001000,
  0b010100,
  0b010100,
  0b001000,
  0b011000,
  0b100110,
  0b011100,
  0b000000 ],

// Character '
[ 0b010000,
  0b010000,
  0b010000,
  0b000000,
  0b000000,
  0b000000,
  0b000000,
  0b000000 ],

// Character (
[ 0b000100,
  0b001000,
  0b010000,
  0b010000,
  0b010000,
  0b001000,
  0b000100,
  0b000000 ],

// Character )
[ 0b001000,
  0b000100,
  0b000010,
  0b000010,
  0b000010,
  0b000100,
  0b001000,
  0b000000 ],

// Character *
[ 0b000000,
  0b000000,
  0b010100,
  0b001000,
  0b010100,
  0b000000,
  0b000000,
  0b000000 ],

// Character +
[ 0b000000,
  0b000000,
  0b001000,
  0b011100,
  0b001000,
  0b000000,
  0b000000,
  0b000000 ],

// Character ,
[ 0b000000,
  0b000000,
  0b000000,
  0b000000,
  0b000100,
  0b000100,
  0b001000,
  0b000000 ],

// Character -
[ 0b000000,
  0b000000,
  0b000000,
  0b011100,
  0b000000,
  0b000000,
  0b000000,
  0b000000 ],

// Character .
[ 0b000000,
  0b000000,
  0b000000,
  0b000000,
  0b001100,
  0b001100,
  0b000000,
  0b000000 ],

// Character /
[ 0b000010,
  0b000100,
  0b000100,
  0b001000,
  0b010000,
  0b010000,
  0b100000,
  0b000000 ],

// Character 0
[ 0b011100,
  0b100010,
  0b110010,
  0b101010,
  0b100110,
  0b100010,
  0b011100,
  0b000000 ],

// Character 1
[ 0b001000,
  0b011000,
  0b001000,
  0b001000,
  0b001000,
  0b001000,
  0b011100,
  0b000000 ],

// Character 2
[ 0b011100,
  0b100010,
  0b000010,
  0b000100,
  0b001000,
  0b010000,
  0b111110,
  0b000000 ],

// Character 3
[ 0b111100,
  0b000010,
  0b000010,
  0b011100,
  0b000010,
  0b000010,
  0b111100,
  0b000000 ],

// Character 4
[ 0b000010,
  0b000110,
  0b001010,
  0b010010,
  0b111110,
  0b000010,
  0b000010,
  0b000000 ],

// Character 5
[ 0b111110,
  0b100000,
  0b100000,
  0b111110,
  0b000010,
  0b000010,
  0b111100,
  0b000000 ],

// Character 6
[ 0b011100,
  0b100010,
  0b100000,
  0b111100,
  0b100010,
  0b100010,
  0b011100,
  0b000000 ],

// Character 7
[ 0b111110,
  0b000010,
  0b000100,
  0b001000,
  0b010000,
  0b010000,
  0b010000,
  0b000000 ],

// Character 8
[ 0b011100,
  0b100010,
  0b100010,
  0b011100,
  0b100010,
  0b100010,
  0b011100,
  0b000000 ],

// Character 9
[ 0b011100,
  0b100010,
  0b100010,
  0b011110,
  0b000010,
  0b000010,
  0b011100,
  0b000000 ],

// Character :
[ 0b000000,
  0b000000,
  0b011000,
  0b011000,
  0b000000,
  0b011000,
  0b011000,
  0b000000 ],

// Character ;
[ 0b000000,
  0b000000,
  0b011000,
  0b011000,
  0b000000,
  0b001000,
  0b001000,
  0b000000 ],

// Character <
[ 0b000010,
  0b000100,
  0b001000,
  0b010000,
  0b001000,
  0b000100,
  0b000010,
  0b000000 ],

// Character =
[ 0b000000,
  0b000000,
  0b111110,
  0b000000,
  0b111110,
  0b000000,
  0b000000,
  0b000000 ],

// Character >
[ 0b010000,
  0b001000,
  0b000100,
  0b000010,
  0b000100,
  0b001000,
  0b010000,
  0b000000 ],

// Character ?
[ 0b011100,
  0b100010,
  0b000010,
  0b000100,
  0b001000,
  0b000000,
  0b001000,
  0b000000 ],

// Character @
[ 0b011100,
  0b100010,
  0b101010,
  0b101110,
  0b100000,
  0b100010,
  0b011100,
  0b000000 ],


// Letter A
[ 0b011100,
  0b100010,
  0b100010,
  0b111110,
  0b100010,
  0b100010,
  0b100010,
  0b000000 ],

// Letter B
[ 0b111100,
  0b100010,
  0b100010,
  0b111100,
  0b100010,
  0b100010,
  0b111100,
  0b000000 ],

// Letter C
[ 0b011100,
  0b100010,
  0b100000,
  0b100000,
  0b100000,
  0b100010,
  0b011100,
  0b000000 ],

// Letter D
[ 0b111100,
  0b100010,
  0b100010,
  0b100010,
  0b100010,
  0b100010,
  0b111100,
  0b000000 ],

// Letter E
[ 0b111110,
  0b100000,
  0b100000,
  0b111110,
  0b100000,
  0b100000,
  0b111110,
  0b000000 ],

// Letter F
[ 0b111110,
  0b100000,
  0b100000,
  0b111110,
  0b100000,
  0b100000,
  0b100000,
  0b000000 ],

// Letter G
[ 0b011100,
  0b100010,
  0b100000,
  0b100110,
  0b100010,
  0b100010,
  0b011110,
  0b000000 ],

// Letter H
[ 0b100010,
  0b100010,
  0b100010,
  0b111110,
  0b100010,
  0b100010,
  0b100010,
  0b000000 ],

// Letter I
[ 0b011100,
  0b001000,
  0b001000,
  0b001000,
  0b001000,
  0b001000,
  0b011100,
  0b000000 ],

// Letter J
[ 0b000010,
  0b000010,
  0b000010,
  0b000010,
  0b000010,
  0b100010,
  0b011100,
  0b000000 ],

// Letter K
[ 0b100010,
  0b100100,
  0b101000,
  0b110000,
  0b101000,
  0b100100,
  0b100010,
  0b000000 ],

// Letter L
[ 0b100000,
  0b100000,
  0b100000,
  0b100000,
  0b100000,
  0b100000,
  0b111100,
  0b000000 ],

// Letter M
[ 0b000000,
  0b100010,
  0b110110,
  0b101010,
  0b100010,
  0b100010,
  0b100010,
  0b000000 ],

// Letter N
[ 0b100010,
  0b110010,
  0b101010,
  0b101010,
  0b100110,
  0b100010,
  0b100010,
  0b000000 ],

// Letter O
[ 0b011100,
  0b100010,
  0b100010,
  0b100010,
  0b100010,
  0b100010,
  0b011100,
  0b000000 ],

// Letter P
[ 0b111100,
  0b100010,
  0b100010,
  0b111100,
  0b100000,
  0b100000,
  0b100000,
  0b000000 ],

// Letter Q
[ 0b011100,
  0b100010,
  0b100010,
  0b100010,
  0b100010,
  0b100110,
  0b011110,
  0b000000 ],

// Letter R
[ 0b111100,
  0b100010,
  0b100010,
  0b111100,
  0b100100,
  0b100010,
  0b100010,
  0b000000 ],

// Letter S
[ 0b011110,
  0b100000,
  0b100000,
  0b011100,
  0b000010,
  0b000010,
  0b111100,
  0b000000 ],

// Letter T
[ 0b111110,
  0b001000,
  0b001000,
  0b001000,
  0b001000,
  0b001000,
  0b001000,
  0b000000 ],

// Letter U
[ 0b100010,
  0b100010,
  0b100010,
  0b100010,
  0b100010,
  0b100010,
  0b011100,
  0b000000 ],

// Letter V
[ 0b100010,
  0b100010,
  0b100010,
  0b100010,
  0b100010,
  0b010100,
  0b001000,
  0b000000 ],

// Letter W
[ 0b100010,
  0b100010,
  0b100010,
  0b100010,
  0b101010,
  0b101010,
  0b010100,
  0b000000 ],

// Letter X
[ 0b000000,
  0b100010,
  0b010100,
  0b001000,
  0b001000,
  0b010100,
  0b100010,
  0b000000 ],

// Letter Y
[ 0b100010,
  0b100010,
  0b100010,
  0b010100,
  0b001000,
  0b001000,
  0b001000,
  0b000000 ],

// Letter Z
[ 0b111110,
  0b000010,
  0b000100,
  0b001000,
  0b010000,
  0b100000,
  0b111110,
  0b000000 ],


//////// LEFT TO DO ///////

// Blank
[ 0b000000,
  0b000000,
  0b000000,
  0b000000,
  0b000000,
  0b000000,
  0b000000,
  0b000000 ],

// Blank
[ 0b000000,
  0b000000,
  0b000000,
  0b000000,
  0b000000,
  0b000000,
  0b000000,
  0b000000 ],

// Blank
[ 0b000000,
  0b000000,
  0b000000,
  0b000000,
  0b000000,
  0b000000,
  0b000000,
  0b000000 ],

// Blank
[ 0b000000,
  0b000000,
  0b000000,
  0b000000,
  0b000000,
  0b000000,
  0b000000,
  0b000000 ],

// Blank
[ 0b000000,
  0b000000,
  0b000000,
  0b000000,
  0b000000,
  0b000000,
  0b000000,
  0b000000 ],



];


use moa_core::host::Pixel;

pub struct CharacterGenerator {
    pub row: i8,
    pub col: i8,
    pub data: &'static [u8; 8],
}

impl CharacterGenerator {
    pub fn new(ch: u8) -> Self {
        Self {
            row: 0,
            col: 5,
            data: &CHARACTERS[ch as usize],
        }
    }
}

impl Iterator for CharacterGenerator {
    type Item = Pixel;

    fn next(&mut self) -> Option<Self::Item> {
        if self.row >= 8 {
            None
        } else {
            let bit = (self.data[self.row as usize] & (1 << self.col)) != 0;

            self.col -= 1;
            if self.col < 0 {
                self.col = 5;
                self.row += 1;
            }

            if bit {
                Some(Pixel::Rgb(0xC0, 0xC0, 0xC0))
            } else {
                Some(Pixel::Rgb(0, 0, 0))
            }
        }
    }
}

