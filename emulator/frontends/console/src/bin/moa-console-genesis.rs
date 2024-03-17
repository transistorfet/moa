use clap::{Arg};

use moa_console::ConsoleFrontend;
use moa_systems_genesis::{build_genesis, SegaGenesisOptions};

fn main() {
    let matches = ConsoleFrontend::args("Sega Genesis/Mega Drive Emulator")
        .arg(Arg::new("ROM").help("ROM file to load (must be flat binary)"))
        .get_matches();

    let mut frontend = ConsoleFrontend;

    let mut options = SegaGenesisOptions::default();
    if let Some(filename) = matches.get_one::<String>("ROM") {
        options.rom = filename.to_string();
    }

    let system = build_genesis(&mut frontend, options).unwrap();
    frontend.start(matches, system);
}
