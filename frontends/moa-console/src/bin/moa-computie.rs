
use moa_console::ConsoleFrontend;
use moa::machines::computie::build_computie;

fn main() {
    let mut frontend = ConsoleFrontend;

    let mut system = build_computie(&mut frontend).unwrap();
    system.run_loop();
}

