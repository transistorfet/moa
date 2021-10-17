
use std::thread;
use std::time::Duration;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::os::unix::io::AsRawFd;

use nix::fcntl::OFlag;
use nix::pty::{self, PtyMaster};
use nix::fcntl::{fcntl, FcntlArg};

use crate::error::Error;


pub struct SimplePty {
    pub name: String,
    input: Option<u8>,
    output: Vec<u8>,
}

pub type SharedSimplePty = Arc<Mutex<SimplePty>>;

impl SimplePty {
    pub fn new_shared(name: String) -> SharedSimplePty {
        Arc::new(Mutex::new(SimplePty {
            name,
            input: None,
            output: vec![],
        }))
    }

    pub fn open() -> Result<SharedSimplePty, Error> {
        let pty = pty::posix_openpt(OFlag::O_RDWR).and_then(|pty| {
            pty::grantpt(&pty)?;
            pty::unlockpt(&pty)?;
            fcntl(pty.as_raw_fd(), FcntlArg::F_SETFL(OFlag::O_NONBLOCK))?;
            Ok(pty)
        }).map_err(|_| Error::new("Error opening new pseudoterminal"))?;

        let name = unsafe { pty::ptsname(&pty).map_err(|_| Error::new("Unable to get pty name"))? };
        let shared = SimplePty::new_shared(name);
        SimplePty::spawn_poller(pty, shared.clone());
        Ok(shared)
    }

    pub fn read(&mut self) -> Option<u8> {
        if self.input.is_some() {
            let input = self.input;
            self.input = None;
            input
        } else {
            None
        }
    }

    pub fn write(&mut self, output: u8) -> bool {
        self.output.push(output);
        true
    }

    fn spawn_poller(mut pty: PtyMaster, shared: SharedSimplePty) {
        thread::spawn(move || {
            println!("pty: spawned reader for {}", shared.lock().unwrap().name);

            let mut buf = [0; 1];
            loop {
                {
                    let mut value = shared.lock().unwrap();
                    if value.input.is_none() {
                        match pty.read(&mut buf) {
                            Ok(_) => {
                                (*value).input = Some(buf[0]);
                            },
                            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => { },
                            Err(err) => {
                                println!("ERROR: {:?}", err);
                            }
                        }
                    }

                    if !value.output.is_empty() {
                        match pty.write_all(value.output.as_slice()) {
                            Ok(()) => { },
                            _ => panic!(""),
                        }
                        (*value).output.clear();
                    }
                }

                thread::sleep(Duration::from_millis(10));
            }
        });
    }
}

