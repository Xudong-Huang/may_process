# may_process

A library for working with processes.

This crate provides a `Command` type that is compatible with the
standard library's `std::process::Command` except that it can run in
coroutine context without blocking the thread execution. When running
in thread context it's the same as using `std::process::Command`.

[![Build Status](https://travis-ci.org/Xudong-Huang/may_process.svg?branch=master)](https://travis-ci.org/Xudong-Huang/may_process)
[![Build status](https://ci.appveyor.com/api/projects/status/5w5en8s0vt910k54/branch/master?svg=true)](https://ci.appveyor.com/project/Xudong-Huang/may-process/branch/master)

[Documentation](https://docs.rs/may_process)

## Usage

First, add this to your `Cargo.toml`:

```toml
[dependencies]
may_process = "0.1"
```

Next you can use the API directly:

```rust,no_run
#[macro_use]
extern crate may;
extern crate may_process;

use may_process::Command;

fn main() {
    join!(
        {
            let ret = Command::new("sh").args(&["-c", "echo hello"]).status();
            assert_eq!(ret.is_ok(), true);
            let exit_status = ret.unwrap();
            assert_eq!(exit_status.success(), true);
            assert_eq!(exit_status.code(), Some(0));
        },
        {
            let ret = Command::new("sh").args(&["-c", "echo may"]).status();
            assert_eq!(ret.is_ok(), true);
            let exit_status = ret.unwrap();
            assert_eq!(exit_status.success(), true);
            assert_eq!(exit_status.code(), Some(0));
        }
    );
}
```

# License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.

