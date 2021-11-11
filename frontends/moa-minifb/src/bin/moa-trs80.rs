
use moa_minifb;
use moa::machines::trs80::{build_trs80, Trs80Options};

fn main() {
    let matches = moa_minifb::new("TRS-80 Emulator")
        .arg("-r, --rom=[FILE]        'ROM file to load at the start of memory'")
        .get_matches();

    let mut options = Trs80Options::new();
    if let Some(filename) = matches.value_of("rom") {
        options.rom = filename.to_string();
    }

    moa_minifb::run(matches, |frontend| {
        build_trs80(frontend, options)
    });
}

