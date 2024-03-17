use clap::Arg;

use moa_systems_genesis::{build_genesis, SegaGenesisOptions};

fn main() {
    let matches = moa_minifb::new("Sega Genesis/Mega Drive Emulator")
        .arg(Arg::new("ROM").help("ROM file to load (must be flat binary)"))
        .get_matches();

    let mut options = SegaGenesisOptions::default();
    if let Some(filename) = matches.get_one::<String>("ROM") {
        options.rom = filename.to_string();
    }

    moa_minifb::run(matches, |frontend| build_genesis(frontend, options));
}
