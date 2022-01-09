
use moa_minifb;
use moa::machines::genesis::{build_genesis, SegaGenesisOptions};

fn main() {
    let matches = moa_minifb::new("Sega Genesis/Mega Drive Emulator")
        .arg("<ROM>        'ROM file to load (must be flat binary)'")
        .get_matches();

    let mut options = SegaGenesisOptions::new();
    if let Some(filename) = matches.value_of("ROM") {
        options.rom = filename.to_string();
    }

    moa_minifb::run(matches, |frontend| {
        build_genesis(frontend, options)
    });
}

