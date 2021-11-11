
use moa::machines::genesis::build_genesis;
use moa_minifb::{run_inline, run_threaded};

fn main() {
    //run_inline(build_genesis);
    run_threaded(build_genesis);
}

