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

#[doc(hiden)]
extern crate winapi;

use std::fmt;
use std::io;
use std::os::windows::prelude::*;
use std::os::windows::process::ExitStatusExt;
use std::process::{self, ExitStatus};
use std::sync::Arc;

use self::winapi::shared::minwindef::*;
use self::winapi::shared::winerror::*;
use self::winapi::um::handleapi::*;
use self::winapi::um::processthreadsapi::*;
use self::winapi::um::synchapi::*;
use self::winapi::um::threadpoollegacyapiset::*;
use self::winapi::um::winbase::*;
use self::winapi::um::winnt::*;
use may::sync::Blocker;

struct Waiter {
    wait_object: HANDLE,
    blocker: Arc<Blocker>,
}

unsafe impl Sync for Waiter {}
unsafe impl Send for Waiter {}

impl Drop for Waiter {
    fn drop(&mut self) {
        unsafe {
            let rc = UnregisterWaitEx(self.wait_object, INVALID_HANDLE_VALUE);
            if rc == 0 {
                eprintln!("failed to unregister: {}", io::Error::last_os_error());
            }
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
        let blocker = Blocker::current();
        let ptr = Arc::into_raw(blocker.clone());
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
            drop(unsafe { Arc::from_raw(ptr) });
            return Err(err);
        }

        Ok(Waiter {
            blocker,
            wait_object,
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
        waiter.blocker.park(None).ok();

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
    let blocker = Arc::from_raw(ptr as *mut Blocker);
    blocker.unpark();
}
