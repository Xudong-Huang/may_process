//! Windows asynchronous process handling.
//!
//! Like with Unix we don't actually have a way of registering a process with an
//! IOCP object. As a result we similarly need another mechanism for getting a
//! signal when a process has exited. For now this is implemented with the
//! `RegisterWaitForSingleObject` function in the kernel32.dll.
//!
//! This strategy is the same that libuv takes and essentially just queues up a
//! wait for the process in a kernel32-specific thread pool. Once the object is
//! notified (e.g. the process exits) then we have a callback that basically
//! just completes a `Oneshot`.
//!
//! The `poll_exit` implementation will attempt to wait for the process in a
//! nonblocking fashion, but failing that it'll fire off a
//! `RegisterWaitForSingleObject` and then wait on the other end of the oneshot
//! from then on out.

extern crate winapi;

use std::fmt;
use std::io;
use std::os::windows::prelude::*;
use std::os::windows::process::ExitStatusExt;
use std::process::{self, ExitStatus};

use may::sync::mpsc;
use self::winapi::shared::minwindef::*;
use self::winapi::shared::winerror::*;
use self::winapi::um::handleapi::*;
use self::winapi::um::processthreadsapi::*;
use self::winapi::um::synchapi::*;
use self::winapi::um::threadpoollegacyapiset::*;
use self::winapi::um::winbase::*;
use self::winapi::um::winnt::*;

struct Waiter {
    wait_object: HANDLE,
    rx: mpsc::Receiver<()>,
    tx: *mut Option<mpsc::Sender<()>>,
}

unsafe impl Sync for Waiter {}
unsafe impl Send for Waiter {}

impl Drop for Waiter {
    fn drop(&mut self) {
        unsafe {
            let rc = UnregisterWaitEx(self.wait_object, INVALID_HANDLE_VALUE);
            if rc == 0 {
                panic!("failed to unregister: {}", io::Error::last_os_error());
            }
            drop(Box::from_raw(self.tx));
        }
    }
}

pub struct Child {
    pub child: process::Child,
}

impl fmt::Debug for Child {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        self.child.fmt(fmt)
    }
}

impl Child {
    pub fn new(child: process::Child) -> Child {
        Child { child: child }
    }

    pub fn id(&self) -> u32 {
        self.child.id()
    }

    pub fn kill(&mut self) -> io::Result<()> {
        self.child.kill()
    }

    fn register(&mut self) -> io::Result<Waiter> {
        let (tx, rx) = mpsc::channel();
        let ptr = Box::into_raw(Box::new(Some(tx)));
        let mut wait_object = 0 as *mut _;
        let rc = unsafe {
            RegisterWaitForSingleObject(
                &mut wait_object,
                self.child.as_raw_handle(),
                Some(callback),
                ptr as *mut _,
                INFINITE,
                WT_EXECUTEINWAITTHREAD | WT_EXECUTEONLYONCE,
            )
        };
        if rc == 0 {
            let err = io::Error::last_os_error();
            drop(unsafe { Box::from_raw(ptr) });
            return Err(err);
        }

        Ok(Waiter {
            rx: rx,
            tx: ptr,
            wait_object: wait_object,
        })
    }

    // this is blocking API
    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        drop(self.child.stdin.take());

        // try wait first
        if let Some(e) = self.try_wait()? {
            return Ok(e);
        }

        // register the waiter
        let waiter = self.register()?;

        // wait for the completion
        waiter.rx.recv().map_err(|e| {
            let msg = format!("can't recv completion, err={}", e);
            io::Error::new(io::ErrorKind::Other, msg)
        })?;

        // get the result
        self.try_wait()?
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "can't get exitstatus"))
    }

    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        unsafe {
            match WaitForSingleObject(self.child.as_raw_handle(), 0) {
                WAIT_OBJECT_0 => {}
                WAIT_TIMEOUT => return Ok(None),
                _ => return Err(io::Error::last_os_error()),
            }
            let mut status = 0;
            let rc = GetExitCodeProcess(self.child.as_raw_handle(), &mut status);
            if rc == FALSE {
                Err(io::Error::last_os_error())
            } else {
                Ok(Some(ExitStatus::from_raw(status)))
            }
        }
    }
}

unsafe extern "system" fn callback(ptr: PVOID, _timer_fired: BOOLEAN) {
    let complete = &mut *(ptr as *mut Option<mpsc::Sender<()>>);
    drop(complete.take().unwrap().send(()));
}
