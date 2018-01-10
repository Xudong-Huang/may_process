//! A libaray for working with processes.
//!
//! This crate provides a `Command` type that is compatible with the
//! standard library's `std::process::Command` execpt that it can run in
//! coroutine context without blocking the thread execution. When running
//! in thread context it's the same as using `std::process::Command`.
//!
//! The asynchronous process support is provided through signal handling
//! on Unix and system APIs on Windows.
//!
//! # Examples
//!
//! Here's an example program which will spawn `echo hello world` and then wait
//! for it in a coroutine context.
//!
//! ```no_run
//! extern crate may_process;
//!
//! use may_process::Command;
//!
//! fn main() {
//!     // Use the `Command` type to build a process and
//!     let child = Command::new("echo").arg("hello").arg("world")
//!                         .spawn();
//!
//!     // Make sure our child succeeded in spawning
//!     let child = child.expect("failed to spawn");
//! }
//! ```
//!

#![warn(missing_debug_implementations)]
#![deny(missing_docs)]
#![doc(html_root_url = "https://docs.rs/may_process/0.1")]

#[macro_use]
#[doc(hiden)]
extern crate may;

use std::fmt;
use std::ffi::OsStr;
use std::path::Path;
use std::io::{self, Read};
use std::process::{self, ExitStatus, Output, Stdio};

// #[path = "unix.rs"]
// #[cfg(unix)]
// mod imp;

#[path = "windows.rs"]
#[cfg(windows)]
mod imp;

/// A process builder, providing fine-grained control
/// over how a new process should be spawned.
///
/// A default configuration can be
/// generated using `Command::new(program)`, where `program` gives a path to the
/// program to be executed. Additional builder methods allow the configuration
/// to be changed (for example, by adding arguments) prior to spawning:
///
/// ```no_run
/// use may_process::Command;
///
/// let output = if cfg!(target_os = "windows") {
///     Command::new("cmd")
///             .args(&["/C", "echo hello"])
///             .output()
///             .expect("failed to execute process")
/// } else {
///     Command::new("sh")
///             .arg("-c")
///             .arg("echo hello")
///             .output()
///             .expect("failed to execute process")
/// };
///
/// let hello = output.stdout;
/// ```
pub struct Command {
    inner: process::Command,
}

impl Command {
    /// Constructs a new `Command` for launching the program at
    /// path `program`, with the following default configuration:
    ///
    /// * No arguments to the program
    /// * Inherit the current process's environment
    /// * Inherit the current process's working directory
    /// * Inherit stdin/stdout/stderr for `spawn` or `status`, but create pipes for `output`
    ///
    /// Builder methods are provided to change these defaults and
    /// otherwise configure the process.
    ///
    /// If `program` is not an absolute path, the `PATH` will be searched in
    /// an OS-defined way.
    ///
    /// The search path to be used may be controlled by setting the
    /// `PATH` environment variable on the Command,
    /// but this has some implementation limitations on Windows
    /// (see <https://github.com/rust-lang/rust/issues/37519>).
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use may_process::Command;
    ///
    /// Command::new("sh")
    ///         .spawn()
    ///         .expect("sh command failed to start");
    /// ```
    pub fn new<S: AsRef<OsStr>>(program: S) -> Command {
        Command {
            inner: process::Command::new(program),
        }
    }

    /// Add an argument to pass to the program.
    ///
    /// Only one argument can be passed per use. So instead of:
    ///
    /// ```no_run
    /// # may_process::Command::new("sh")
    /// .arg("-C /path/to/repo")
    /// # ;
    /// ```
    ///
    /// usage would be:
    ///
    /// ```no_run
    /// # may_process::Command::new("sh")
    /// .arg("-C")
    /// .arg("/path/to/repo")
    /// # ;
    /// ```
    ///
    /// To pass multiple arguments see [`args`].
    ///
    /// [`args`]: #method.args
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use may_process::Command;
    ///
    /// Command::new("ls")
    ///         .arg("-l")
    ///         .arg("-a")
    ///         .spawn()
    ///         .expect("ls command failed to start");
    /// ```
    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Command {
        self.inner.arg(arg);
        self
    }

    /// Add multiple arguments to pass to the program.
    ///
    /// To pass a single argument see [`arg`].
    ///
    /// [`arg`]: #method.arg
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use may_process::Command;
    ///
    /// Command::new("ls")
    ///         .args(&["-l", "-a"])
    ///         .spawn()
    ///         .expect("ls command failed to start");
    /// ```
    pub fn args<I, S>(&mut self, args: I) -> &mut Command
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.inner.args(args);
        self
    }

    /// Inserts or updates an environment variable mapping.
    ///
    /// Note that environment variable names are case-insensitive (but case-preserving) on Windows,
    /// and case-sensitive on all other platforms.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use may_process::Command;
    ///
    /// Command::new("ls")
    ///         .env("PATH", "/bin")
    ///         .spawn()
    ///         .expect("ls command failed to start");
    /// ```
    pub fn env<K, V>(&mut self, key: K, val: V) -> &mut Command
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.inner.env(key, val);
        self
    }

    /// Add or update multiple environment variable mappings.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use may_process::Command;
    /// use std::env;
    /// use std::process::Stdio;
    /// use std::collections::HashMap;
    ///
    /// let filtered_env : HashMap<String, String> =
    ///     env::vars().filter(|&(ref k, _)|
    ///         k == "TERM" || k == "TZ" || k == "LANG" || k == "PATH"
    ///     ).collect();
    ///
    /// Command::new("printenv")
    ///         .stdin(Stdio::null())
    ///         .stdout(Stdio::inherit())
    ///         .env_clear()
    ///         .envs(&filtered_env)
    ///         .spawn()
    ///         .expect("printenv failed to start");
    /// ```
    pub fn envs<I, K, V>(&mut self, vars: I) -> &mut Command
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.inner.envs(vars);
        self
    }

    /// Removes an environment variable mapping.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use may_process::Command;
    ///
    /// Command::new("ls")
    ///         .env_remove("PATH")
    ///         .spawn()
    ///         .expect("ls command failed to start");
    /// ```
    pub fn env_remove<K: AsRef<OsStr>>(&mut self, key: K) -> &mut Command {
        self.inner.env_remove(key);
        self
    }

    /// Clears the entire environment map for the child process.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use may_process::Command;
    ///
    /// Command::new("ls")
    ///         .env_clear()
    ///         .spawn()
    ///         .expect("ls command failed to start");
    /// ```
    pub fn env_clear(&mut self) -> &mut Command {
        self.inner.env_clear();
        self
    }

    /// Sets the working directory for the child process.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use may_process::Command;
    ///
    /// Command::new("ls")
    ///         .current_dir("/bin")
    ///         .spawn()
    ///         .expect("ls command failed to start");
    /// ```
    pub fn current_dir<P: AsRef<Path>>(&mut self, dir: P) -> &mut Command {
        self.inner.current_dir(dir);
        self
    }

    /// Configuration for the child process's standard input (stdin) handle.
    ///
    /// Defaults to [`inherit`] when used with `spawn` or `status`, and
    /// defaults to [`piped`] when used with `output`.
    ///
    /// [`inherit`]: struct.Stdio.html#method.inherit
    /// [`piped`]: struct.Stdio.html#method.piped
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use may_process::Command;
    /// use std::process::Stdio;
    ///
    /// Command::new("ls")
    ///         .stdin(Stdio::null())
    ///         .spawn()
    ///         .expect("ls command failed to start");
    /// ```
    pub fn stdin<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Command {
        self.inner.stdin(cfg);
        self
    }

    /// Configuration for the child process's standard output (stdout) handle.
    ///
    /// Defaults to [`inherit`] when used with `spawn` or `status`, and
    /// defaults to [`piped`] when used with `output`.
    ///
    /// [`inherit`]: struct.Stdio.html#method.inherit
    /// [`piped`]: struct.Stdio.html#method.piped
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use may_process::Command;
    /// use std::process::Stdio;
    ///
    /// Command::new("ls")
    ///         .stdout(Stdio::null())
    ///         .spawn()
    ///         .expect("ls command failed to start");
    /// ```
    pub fn stdout<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Command {
        self.inner.stdout(cfg);
        self
    }

    /// Configuration for the child process's standard error (stderr) handle.
    ///
    /// Defaults to [`inherit`] when used with `spawn` or `status`, and
    /// defaults to [`piped`] when used with `output`.
    ///
    /// [`inherit`]: struct.Stdio.html#method.inherit
    /// [`piped`]: struct.Stdio.html#method.piped
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use std::process::Stdio;
    /// use may_process::Command;
    ///
    /// Command::new("ls")
    ///         .stderr(Stdio::null())
    ///         .spawn()
    ///         .expect("ls command failed to start");
    /// ```
    pub fn stderr<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Command {
        self.inner.stderr(cfg);
        self
    }

    /// Executes the command as a child process, returning a handle to it.
    ///
    /// By default, stdin, stdout and stderr are inherited from the parent.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use may_process::Command;
    ///
    /// Command::new("ls")
    ///         .spawn()
    ///         .expect("ls command failed to start");
    /// ```
    pub fn spawn(&mut self) -> io::Result<Child> {
        self.inner.spawn().map(|p| Child {
            inner: imp::Child::new(p),
        })
    }

    /// Executes the command as a child process, waiting for it to finish and
    /// collecting all of its output.
    ///
    /// By default, stdout and stderr are captured (and used to provide the
    /// resulting output). Stdin is not inherited from the parent and any
    /// attempt by the child process to read from the stdin stream will result
    /// in the stream immediately closing.
    ///
    /// # Examples
    ///
    /// ```should_panic
    /// use may_process::Command;
    /// let output = Command::new("/bin/cat")
    ///                      .arg("file.txt")
    ///                      .output()
    ///                      .expect("failed to execute process");
    ///
    /// println!("status: {}", output.status);
    /// println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
    /// println!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    ///
    /// assert!(output.status.success());
    /// ```
    pub fn output(&mut self) -> io::Result<Output> {
        // TODO:
        unimplemented!()
        // self.inner.spawn(imp::Stdio::MakePipe, false).map(Child::from_inner)
        // .and_then(|p| p.wait_with_output())
    }

    /// Executes a command as a child process, waiting for it to finish and
    /// collecting its exit status.
    ///
    /// By default, stdin, stdout and stderr are inherited from the parent.
    ///
    /// # Examples
    ///
    /// ```should_panic
    /// use may_process::Command;
    ///
    /// let status = Command::new("/bin/cat")
    ///                      .arg("file.txt")
    ///                      .status()
    ///                      .expect("failed to execute process");
    ///
    /// println!("process exited with: {}", status);
    ///
    /// assert!(status.success());
    /// ```
    pub fn status(&mut self) -> io::Result<ExitStatus> {
        self.spawn().and_then(|mut p| p.wait())
    }
}

impl fmt::Debug for Command {
    /// Format the program and arguments of a Command for display. Any
    /// non-utf8 data is lossily converted using the utf8 replacement
    /// character.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.fmt(f)
    }
}

/// Representation of a running or exited child process.
///
/// This structure is used to represent and manage child processes. A child
/// process is created via the [`Command`] struct, which configures the
/// spawning process and can itself be constructed using a builder-style
/// interface.
///
/// There is no implementation of [`Drop`] for child processes,
/// so if you do not ensure the `Child` has exited then it will continue to
/// run, even after the `Child` handle to the child process has gone out of
/// scope.
///
/// Calling [`wait`](#method.wait) (or other functions that wrap around it) will make
/// the parent process wait until the child has actually exited before
/// continuing.
///
/// # Examples
///
/// ```should_panic
/// use may_process::Command;
///
/// let mut child = Command::new("/bin/cat")
///                         .arg("file.txt")
///                         .spawn()
///                         .expect("failed to execute child");
///
/// let ecode = child.wait()
///                  .expect("failed to wait on child");
///
/// assert!(ecode.success());
/// ```
///
/// [`Command`]: struct.Command.html
/// [`wait`]: #method.wait
pub struct Child {
    /// actual inner child
    inner: imp::Child,
}

impl fmt::Debug for Child {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl Child {
    /// Forces the child to exit. This is equivalent to sending a
    /// SIGKILL on unix platforms.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use may_process::Command;
    ///
    /// let mut command = Command::new("yes");
    /// if let Ok(mut child) = command.spawn() {
    ///     child.kill().expect("command wasn't running");
    /// } else {
    ///     println!("yes command didn't start");
    /// }
    /// ```
    pub fn kill(&mut self) -> io::Result<()> {
        self.inner.kill()
    }

    /// Returns the OS-assigned process identifier associated with this child.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use may_process::Command;
    ///
    /// let mut command = Command::new("ls");
    /// if let Ok(child) = command.spawn() {
    ///     println!("Child's id is {}", child.id());
    /// } else {
    ///     println!("ls command didn't start");
    /// }
    /// ```
    pub fn id(&self) -> u32 {
        self.inner.id()
    }

    /// Waits for the child to exit completely, returning the status that it
    /// exited with. This function will continue to have the same return value
    /// after it has been called at least once.
    ///
    /// The stdin handle to the child process, if any, will be closed
    /// before waiting. This helps avoid deadlock: it ensures that the
    /// child does not block waiting for input from the parent, while
    /// the parent waits for the child to exit.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use may_process::Command;
    ///
    /// let mut command = Command::new("ls");
    /// if let Ok(mut child) = command.spawn() {
    ///     child.wait().expect("command wasn't running");
    ///     println!("Child has finished its execution!");
    /// } else {
    ///     println!("ls command didn't start");
    /// }
    /// ```
    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        self.inner.wait()
    }

    /// Attempts to collect the exit status of the child if it has already
    /// exited.
    ///
    /// This function will not block the calling thread and will only advisorily
    /// check to see if the child process has exited or not. If the child has
    /// exited then on Unix the process id is reaped. This function is
    /// guaranteed to repeatedly return a successful exit status so long as the
    /// child has already exited.
    ///
    /// If the child has exited, then `Ok(Some(status))` is returned. If the
    /// exit status is not available at this time then `Ok(None)` is returned.
    /// If an error occurs, then that error is returned.
    ///
    /// Note that unlike `wait`, this function will not attempt to drop stdin.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use may_process::Command;
    ///
    /// let mut child = Command::new("ls").spawn().unwrap();
    ///
    /// match child.try_wait() {
    ///     Ok(Some(status)) => println!("exited with: {}", status),
    ///     Ok(None) => {
    ///         println!("status not ready yet, let's really wait");
    ///         let res = child.wait();
    ///         println!("result: {:?}", res);
    ///     }
    ///     Err(e) => println!("error attempting to wait: {}", e),
    /// }
    /// ```
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.inner.try_wait()
    }

    /// Simultaneously waits for the child to exit and collect all remaining
    /// output on the stdout/stderr handles, returning an `Output`
    /// instance.
    ///
    /// The stdin handle to the child process, if any, will be closed
    /// before waiting. This helps avoid deadlock: it ensures that the
    /// child does not block waiting for input from the parent, while
    /// the parent waits for the child to exit.
    ///
    /// By default, stdin, stdout and stderr are inherited from the parent.
    /// In order to capture the output into this `Result<Output>` it is
    /// necessary to create new pipes between parent and child. Use
    /// `stdout(Stdio::piped())` or `stderr(Stdio::piped())`, respectively.
    ///
    /// # Examples
    ///
    /// ```should_panic
    /// use may_process::Command;
    /// use std::process::Stdio;
    ///
    /// let child = Command::new("/bin/cat")
    ///     .arg("file.txt")
    ///     .stdout(Stdio::piped())
    ///     .spawn()
    ///     .expect("failed to execute child");
    ///
    /// let output = child
    ///     .wait_with_output()
    ///     .expect("failed to wait on child");
    ///
    /// assert!(output.status.success());
    /// ```
    ///
    pub fn wait_with_output(mut self) -> io::Result<Output> {
        // TODO: impl nonblocking read for stdio
        let status = self.wait()?;

        let p = &mut self.inner.child;
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        match (p.stdout.take(), p.stderr.take()) {
            (None, None) => {}
            (Some(mut out), None) => {
                let res = out.read_to_end(&mut stdout);
                res.unwrap();
            }
            (None, Some(mut err)) => {
                let res = err.read_to_end(&mut stderr);
                res.unwrap();
            }
            (Some(mut out), Some(mut err)) => join!(
                {
                    // let res = read2(out.inner, &mut stdout, err.inner, &mut stderr);
                    let res = out.read_to_end(&mut stdout);
                    res.unwrap();
                },
                {
                    let res = err.read_to_end(&mut stderr);
                    res.unwrap();
                }
            ),
        }

        Ok(Output {
            status,
            stdout,
            stderr,
        })
    }
}
