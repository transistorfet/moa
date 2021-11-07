
use std::thread;
use std::time::Duration;
use std::sync::{Arc, Mutex};

use moa::machines::trs80::build_trs80;
use moa_minifb::MiniFrontendBuilder;

fn main() {
    /*
    let mut frontend = Arc::new(Mutex::new(MiniFrontendBuilder::new()));

    {
        let frontend = frontend.clone();
        thread::spawn(move || {
            let mut system = build_trs80(&mut *(frontend.lock().unwrap())).unwrap();
            frontend.lock().unwrap().finalize();
            system.run_loop();
        });
    }

    wait_until_initialized(frontend.clone());

    frontend
        .lock().unwrap()
        .build()
        .start(None);
    */

    let mut frontend = MiniFrontendBuilder::new();
    let mut system = build_trs80(&mut frontend).unwrap();

    frontend
        .build()
        .start(Some(system));
}

fn wait_until_initialized(frontend: Arc<Mutex<MiniFrontendBuilder>>) {
    while frontend.lock().unwrap().finalized == false {
        thread::sleep(Duration::from_millis(10));
    }
}

