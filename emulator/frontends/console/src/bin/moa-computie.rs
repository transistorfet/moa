
use moa_console::ConsoleFrontend;
use moa_systems_computie::build_computie;

fn main() {
    simple_logger::SimpleLogger::new()
        .with_level(log::Level::Debug.to_level_filter())
        .without_timestamps()
        .init().unwrap();

    let mut frontend = ConsoleFrontend;

    let mut system = build_computie(&mut frontend).unwrap();
    system.run_loop();
}

