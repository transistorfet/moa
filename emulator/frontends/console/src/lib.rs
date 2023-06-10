
use clap::{Command, Arg, ArgAction, ArgMatches};
use std::io::{self, Write};

use moa_core::{Error, ErrorType, System, ClockDuration, DebugControl, Debugger};
use moa_core::host::{Host, Tty, ControllerEvent, Audio, DummyAudio, FrameReceiver, EventSender};

pub struct ConsoleFrontend;

impl Host for ConsoleFrontend {
    fn add_pty(&self) -> Result<Box<dyn Tty>, Error> {
        use moa_common::tty::SimplePty;
        Ok(Box::new(SimplePty::open()?))
    }

    fn add_video_source(&mut self, _receiver: FrameReceiver) -> Result<(), Error> {
        println!("console: add_window() is not supported from the console; ignoring request...");
        Ok(())
    }

    fn register_controllers(&mut self, _sender: EventSender<ControllerEvent>) -> Result<(), Error> {
        println!("console: register_controller() is not supported from the console; ignoring request...");
        Ok(())
    }

    fn add_audio_source(&mut self) -> Result<Box<dyn Audio>, Error> {
        println!("console: create_audio_source() is not supported from the console; returning dummy device...");
        Ok(Box::new(DummyAudio()))
    }
}

impl ConsoleFrontend {
    pub fn new() -> Self {
        Self
    }

    pub fn args(application_name: &'static str) -> Command {
        Command::new(application_name)
            .arg(Arg::new("log-level")
                .short('l')
                .long("log-level")
                .help("Set the type of log messages to print"))
            .arg(Arg::new("debugger")
                .short('d')
                .long("debugger")
                .action(ArgAction::SetTrue)
                .help("Start the debugger before running machine"))
    }

    pub fn start(self, matches: ArgMatches, mut system: System) {
        let log_level = match matches.get_one("log-level").map(|s: &String| s.as_str()) {
            Some("trace") => log::Level::Trace,
            Some("debug") => log::Level::Debug,
            Some("info") => log::Level::Info,
            Some("warn") => log::Level::Warn,
            Some("error") => log::Level::Error,
            _ => log::Level::Info,
        };

        // Start the logger
        simple_logger::SimpleLogger::new()
            .with_level(log_level.to_level_filter())
            .without_timestamps()
            .init()
            .unwrap();

        // Run the main loop
        let mut debugger = Debugger::default();
        let mut run_debugger = matches.get_flag("debugger");
        loop {
            if run_debugger {
                run_debugger = false;

                loop {
                    debugger.print_step(&mut system).unwrap();
                    if debugger.check_auto_command(&mut system).unwrap() == DebugControl::Continue {
                        continue;
                    }

                    let mut buffer = String::new();
                    io::stdout().write_all(b"> ").unwrap();
                    io::stdin().read_line(&mut buffer).unwrap();
                    match debugger.run_command(&mut system, &buffer) {
                        Ok(DebugControl::Exit) => break,
                        Ok(_) => {},
                        Err(err) => {
                            println!("Error: {}", err.msg);
                        },
                    }
                }
            }

            match system.run_for_duration(ClockDuration::MAX - system.clock.as_duration()) {
                Ok(()) => {},
                Err(err) if err.err == ErrorType::Breakpoint => {
                    run_debugger = true;
                },
                Err(err) => {
                    panic!("{:?}", err);
                },
            }
        }
    }
}

