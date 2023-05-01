
use clap::Arg;

use moa_systems_trs80::{build_trs80, Trs80Options};

fn main() {
    let matches = moa_minifb::new("TRS-80 Emulator")
        .arg(Arg::new("ROM")
            .short('r')
            .long("rom")
            .takes_value(true)
            .value_name("FILE")
            .help("ROM file to load at the start of memory"))
        .get_matches();

    let mut options = Trs80Options::default();
    if let Some(filename) = matches.value_of("ROM") {
        options.rom = filename.to_string();
    }

    moa_minifb::run(matches, |frontend| {
        build_trs80(frontend, options)
    });
}

