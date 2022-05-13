
use std::fs;
use std::env;

use moa::cpus::m68k::M68kType;
use moa::cpus::m68k::assembler::M68kAssembler;

fn main() {
    let mut assembler = M68kAssembler::new(M68kType::MC68000);

    let filename = env::args().nth(1).unwrap();
    let text = fs::read_to_string(filename).unwrap();

    let words = assembler.assemble_words(&text).unwrap();

    println!("Output:");
    for word in words.iter() {
        print!("{:04x} ", word);
    }
    println!("");
}
 
