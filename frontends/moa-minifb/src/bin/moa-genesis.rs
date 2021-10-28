
use std::thread;
use std::sync::Arc;

use moa::machines::genesis::build_genesis;
use moa_minifb::MiniFrontend;

fn main() {
    /*
    let mut frontend = Arc::new(MiniFrontend::init_frontend());

    {
        let frontend = frontend.clone();
        thread::spawn(move || {
            let mut system = build_genesis(&*frontend).unwrap();
            system.run_loop();
        });
    }

    frontend.start();
    */

    let mut frontend = MiniFrontend::init_frontend();
    let mut system = build_genesis(&frontend).unwrap();

    frontend.start(system);
}

