extern crate may_process;

use may_process::Command;

#[cfg(windows)]
#[test]
fn simple_test() {
    let ret = Command::new("cmd").args(&["/C", "echo hello"]).status();
    assert_eq!(ret.is_ok(), true);
    let exit_status = ret.unwrap();
    assert_eq!(exit_status.success(), true);
    assert_eq!(exit_status.code(), Some(0));
}
