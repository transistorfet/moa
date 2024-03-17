use std::thread;
use std::sync::mpsc;
use std::time::Duration;
use std::io::{Read, Write};
use std::os::unix::io::AsRawFd;

use nix::fcntl::OFlag;
use nix::pty::{self, PtyMaster};
use nix::fcntl::{fcntl, FcntlArg};

use moa_host::Tty;


#[derive(Debug, PartialEq, Eq)]
pub enum SimplePtyError {
    Open,
    PtsName,
}

pub struct SimplePty {
    pub name: String,
    input: mpsc::Receiver<u8>,
    output: mpsc::Sender<u8>,
}

impl SimplePty {
    pub fn new(name: String, input: mpsc::Receiver<u8>, output: mpsc::Sender<u8>) -> SimplePty {
        SimplePty {
            name,
            input,
            output,
        }
    }

    pub fn open() -> Result<SimplePty, SimplePtyError> {
        let pty = pty::posix_openpt(OFlag::O_RDWR)
            .and_then(|pty| {
                pty::grantpt(&pty)?;
                pty::unlockpt(&pty)?;
                Ok(pty)
            })
            .map_err(|_| SimplePtyError::Open)?;

        let name = unsafe { pty::ptsname(&pty).map_err(|_| SimplePtyError::PtsName)? };
        let (input_tx, input_rx) = mpsc::channel();
        let (output_tx, output_rx) = mpsc::channel();
        let shared = SimplePty::new(name.clone(), input_rx, output_tx);

        SimplePty::spawn_poller(pty, name, input_tx, output_rx);
        Ok(shared)
    }

    fn spawn_poller(mut pty: PtyMaster, name: String, input_tx: mpsc::Sender<u8>, output_rx: mpsc::Receiver<u8>) {
        thread::spawn(move || {
            println!("pty: spawned reader for {}", name);

            fcntl(pty.as_raw_fd(), FcntlArg::F_SETFL(OFlag::O_NONBLOCK)).unwrap();

            let mut buf = [0; 1];
            loop {
                match pty.read(&mut buf) {
                    Ok(_) => {
                        input_tx.send(buf[0]).unwrap();
                    },
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {},
                    Err(err) => {
                        println!("ERROR: {:?}", err);
                    },
                }

                while let Ok(data) = output_rx.try_recv() {
                    pty.write_all(&[data]).unwrap();
                }

                thread::sleep(Duration::from_millis(10));
            }
        });
    }
}

impl Tty for SimplePty {
    fn device_name(&self) -> String {
        self.name.clone()
    }

    fn read(&mut self) -> Option<u8> {
        match self.input.try_recv() {
            Ok(data) => Some(data),
            _ => None,
        }
    }

    fn write(&mut self, output: u8) -> bool {
        self.output.send(output).unwrap();
        true
    }
}
