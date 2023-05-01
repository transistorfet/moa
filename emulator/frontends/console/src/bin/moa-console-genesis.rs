
use clap::{App, Arg};

use moa_console::ConsoleFrontend;
use moa_systems_genesis::{build_genesis, SegaGenesisOptions};

fn main() {
    let matches = App::new("Sega Genesis/Mega Drive Emulator")
        .arg(Arg::new("ROM")
            .help("ROM file to load (must be flat binary)"))
        .get_matches();

    let mut frontend = ConsoleFrontend;

    let mut options = SegaGenesisOptions::default();
    if let Some(filename) = matches.value_of("ROM") {
        options.rom = filename.to_string();
    }

    let mut system = build_genesis(&mut frontend, options).unwrap();
    system.run_loop();
}

