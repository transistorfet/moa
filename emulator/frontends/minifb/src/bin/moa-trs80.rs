use clap::{Arg, ArgAction};

use moa_systems_trs80::{build_trs80, Trs80Options};

fn main() {
    let matches = moa_minifb::new("TRS-80 Emulator")
        .arg(
            Arg::new("ROM")
                .short('r')
                .long("rom")
                .action(ArgAction::SetTrue)
                .value_name("FILE")
                .help("ROM file to load at the start of memory"),
        )
        .get_matches();

    let mut options = Trs80Options::default();
    if let Some(filename) = matches.get_one::<String>("ROM") {
        options.rom = filename.to_string();
    }

    moa_minifb::run(matches, |frontend| build_trs80(frontend, options));
}
