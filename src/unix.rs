//! Unix handling of child processes
//!
//! Right now the only "fancy" thing about this is how we implement the
//! wait on `Child` to get the exit status. Unix offers no way to register
//! a child with epoll, and the only real way to get a notification when a
//! process exits is the SIGCHLD signal.
//!
//! Signal handling in general is *super* hairy and complicated, and it's even
//! more complicated here with the fact that signals are coalesced, so we may
//! not get a SIGCHLD-per-child.
//!
//! Our best approximation here is to check *all spawned processes* for all
//! SIGCHLD signals received. To do that we create a `Signal`, implemented in
//! the `may_signal` crate, which is a stream over signals being received.
//!

extern crate libc;
extern crate may_signal;

use std::fmt;
use std::io;
use std::os::unix::prelude::*;
use std::process::{self, ExitStatus};

use self::libc::c_int;
use self::may_signal::unix::Signal;

pub struct Child {
    pub child: process::Child,
    sigchld: Signal,
}

impl fmt::Debug for Child {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        self.child.fmt(fmt)
    }
}

impl Child {
    pub fn new(child: process::Child) -> Child {
        Child {
            child: child,
            sigchld: Signal::new(libc::SIGCHLD).expect("can't create signal strema"),
        }
    }

    pub fn id(&self) -> u32 {
        self.child.id()
    }

    pub fn kill(&mut self) -> io::Result<()> {
        self.child.kill()
    }

    // this is blocking API
    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        drop(self.child.stdin.take());

        loop {
            // try wait first
            if let Some(e) = self.try_wait()? {
                return Ok(e);
            }

            match self.sigchld.recv() {
                Ok(_) => {
                    // the signal may be other child exist signal
                    // so we need to check again in the loop
                    continue;
                }
                Err(e) => {
                    let msg = format!("failed to recv signal, err={}", e);
                    return Err(io::Error::new(io::ErrorKind::Other, msg));
                }
            }
        }
    }

    pub fn try_wait(&self) -> io::Result<Option<ExitStatus>> {
        let id = self.id() as c_int;
        let mut status = 0;
        loop {
            match unsafe { libc::waitpid(id, &mut status, libc::WNOHANG) } {
                0 => return Ok(None),
                n if n < 0 => {
                    let err = io::Error::last_os_error();
                    if err.kind() == io::ErrorKind::Interrupted {
                        continue;
                    }
                    return Err(err);
                }
                n => {
                    assert_eq!(n, id);
                    return Ok(Some(ExitStatus::from_raw(status)));
                }
            }
        }
    }
}
