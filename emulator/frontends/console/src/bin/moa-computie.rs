
use moa_console::ConsoleFrontend;
use moa_systems_computie::build_computie;

fn main() {
    let matches = ConsoleFrontend::args("Computie68k Emulator")
        .get_matches();

    let mut frontend = ConsoleFrontend::new();

    let mut system = build_computie(&mut frontend).unwrap();
    frontend.start(matches, system);
}

