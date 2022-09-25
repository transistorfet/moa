
use moa_minifb;
use moa_macintosh::build_macintosh_512k;

fn main() {
    let matches = moa_minifb::new("Macintosh 512k Emulator")
        .get_matches();

    moa_minifb::run(matches, |frontend| {
        build_macintosh_512k(frontend)
    });
}

