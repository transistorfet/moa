
use moa_console::ConsoleFrontend;
use moa::machines::computie::run_computie;

fn main() {
    let mut frontend = ConsoleFrontend;

    run_computie(&mut frontend);
}

