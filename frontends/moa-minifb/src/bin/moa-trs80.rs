
use moa::machines::trs80::build_trs80;
use moa_minifb::{run_inline, run_threaded};

fn main() {
    //run_inline(build_trs80);
    run_threaded(build_trs80);
}

