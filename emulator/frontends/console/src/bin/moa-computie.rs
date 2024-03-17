use clap::Arg;

use moa_console::ConsoleFrontend;
use moa_systems_computie::{build_computie, ComputieOptions};

fn main() {
    let matches = ConsoleFrontend::args("Computie68k Emulator")
        .arg(
            Arg::new("ROM")
                .short('r')
                .long("rom")
                .value_name("FILE")
                .help("ROM file to load at the start of memory"),
        )
        .get_matches();

    let mut options = ComputieOptions::default();
    if let Some(filename) = matches.get_one::<String>("ROM") {
        options.rom = filename.to_string();
    }

    let frontend = ConsoleFrontend;

    let system = build_computie(&frontend, options).unwrap();
    frontend.start(matches, system);
}
