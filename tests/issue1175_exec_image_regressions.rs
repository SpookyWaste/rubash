//! rubash#1175 — exec does not replace the shell process image
//! (PID changes).
//!
//! GNU builtins/exec.def / execute_cmd.c: `exec cmd` replaces the
//! shell process — the command runs with the SAME pid the shell had,
//! at top level and inside a function alike, and nothing after the
//! exec runs.
//!
//! The test re-execs the rubash binary itself and compares the pid
//! printed before and after; GNU parity for this shape is definitional
//! (pid preservation), verified against GNU bash 5.3.20.

#![cfg(unix)]

use std::process::Command;

#[test]
fn exec_replaces_process_image_same_pid() {
    let exe = env!("CARGO_BIN_EXE_rubash");
    let script = format!("echo \"self=$$\"\nexec \"{exe}\" -c 'echo \"after pid=$$\"'");
    let output = Command::new(exe)
        .arg("-c")
        .arg(&script)
        .output()
        .expect("run rubash");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let self_pid = stdout
        .lines()
        .find_map(|l| l.strip_prefix("self="))
        .unwrap_or_else(|| panic!("stdout: {stdout}"));
    let after_pid = stdout
        .lines()
        .find_map(|l| l.strip_prefix("after pid="))
        .unwrap_or_else(|| panic!("stdout: {stdout}"));
    assert_eq!(
        self_pid, after_pid,
        "exec must preserve the pid; stdout: {stdout}"
    );
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn exec_inside_function_replaces_image() {
    let exe = env!("CARGO_BIN_EXE_rubash");
    let script = format!(
        "f() {{ exec \"{exe}\" -c 'echo \"in-func-exec pid=$$\"'; }}\n\
         echo \"self=$$\"\n\
         f\n\
         echo unreachable"
    );
    let output = Command::new(exe)
        .arg("-c")
        .arg(&script)
        .output()
        .expect("run rubash");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(!stdout.contains("unreachable"), "stdout: {stdout}");
    let self_pid = stdout
        .lines()
        .find_map(|l| l.strip_prefix("self="))
        .unwrap_or_else(|| panic!("stdout: {stdout}"));
    let exec_pid = stdout
        .lines()
        .find_map(|l| l.strip_prefix("in-func-exec pid="))
        .unwrap_or_else(|| panic!("stdout: {stdout}"));
    assert_eq!(self_pid, exec_pid, "stdout: {stdout}");
    assert_eq!(output.status.code(), Some(0));
}
