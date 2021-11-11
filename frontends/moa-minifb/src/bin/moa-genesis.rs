

use moa_minifb;
use moa::machines::genesis::build_genesis;

fn main() {
    let matches = moa_minifb::new("Sega Genesis/Mega Drive Emulator")
        .get_matches();

    moa_minifb::run(matches, |frontends| {
        build_genesis(frontend)
    });
}

