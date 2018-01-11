#[macro_use]
extern crate may;
extern crate may_process;

use may_process::Command;

#[cfg(windows)]
#[test]
fn simple_test() {
    // sleep 3 seconds on windows
    let ret = Command::new("cmd")
        .args(&["/C", "echo hello"])
        .output()
        .expect("failed to execute process");
    // println!("ret = {:?}", ret);
    let exit_status = ret.status;
    assert_eq!(exit_status.success(), true);
    assert_eq!(exit_status.code(), Some(0));
}

#[cfg(windows)]
#[test]
fn simple_wait_test() {
    // sleep 3 seconds on windows
    let ret = Command::new("cmd")
        .args(&["/C", "ping -n 3 127.0.0.1 > nul"])
        .status();
    // println!("ret = {:?}", ret);
    assert_eq!(ret.is_ok(), true);
    let exit_status = ret.unwrap();
    assert_eq!(exit_status.success(), true);
    assert_eq!(exit_status.code(), Some(0));
}

#[cfg(windows)]
#[test]
fn coroutine_test() {
    join!(
        {
            let ret = Command::new("cmd").args(&["/C", "echo hello"]).status();
            assert_eq!(ret.is_ok(), true);
            let exit_status = ret.unwrap();
            assert_eq!(exit_status.success(), true);
            assert_eq!(exit_status.code(), Some(0));
        },
        {
            let ret = Command::new("cmd").args(&["/C", "echo may"]).status();
            assert_eq!(ret.is_ok(), true);
            let exit_status = ret.unwrap();
            assert_eq!(exit_status.success(), true);
            assert_eq!(exit_status.code(), Some(0));
        }
    );
}
