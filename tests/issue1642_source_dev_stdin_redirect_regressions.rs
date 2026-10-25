//! rubash#1642 — `source /dev/stdin` with a non-pipe stdin
//! redirection (here-string, heredoc, `< file`) silently read
//! nothing: the /dev/stdin arm fell through to the process stdin,
//! which blocked on a live stdin or read EOF when stdin was
//! redirected. The piped form worked because the pipe IS the
//! process stdin.
//!
//! GNU ownership: redir.c do_redirections binds fd 0 before the
//! builtin runs, so opening /dev/stdin reads the redirect. The fix
//! checks first whether the source command carries its own fd-0
//! stdin redirect, and if so sources the redirect's text (via the
//! same stdin_string_for_command_mut path other builtins use);
//! bare commands in pipelines keep the existing FUNCTION_STDIN /
//! process-stdin path untouched.
//!
//! Expectations verified against GNU bash. Stdin is explicitly
//! nulled so a regression blocks on nothing (it would read EOF
//! instead of hanging the suite).

use std::io::Write;
use std::process::{Command, Stdio};

fn rubash_c_null_stdin(script: &str) -> (String, String, Option<i32>) {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(script)
        .stdin(Stdio::null())
        .output()
        .expect("run rubash");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code(),
    )
}

fn rubash_c_piped_stdin(script: &str, piped: &str) -> (String, String, Option<i32>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rubash");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(piped.as_bytes())
        .expect("write piped stdin");
    let output = child.wait_with_output().expect("wait rubash");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code(),
    )
}

#[test]
fn source_dev_stdin_herestring() {
    let (stdout, _, code) =
        rubash_c_null_stdin("source /dev/stdin <<< 'echo SRCHS'; echo \"rc=$?\"");
    assert_eq!(stdout, "SRCHS\nrc=0\n");
    assert_eq!(code, Some(0));
}

#[test]
fn source_dev_stdin_file_redirect() {
    let dir = std::env::temp_dir().join(format!(
        "rubash-1642-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir(&dir).expect("create scratch dir");
    std::fs::write(dir.join("s.sh"), "echo FROMFILE2\n").expect("seed file");
    let script = format!(
        "source /dev/stdin < {}; echo \"rc=$?\"",
        dir.join("s.sh").display()
    );
    let (stdout, _, code) = rubash_c_null_stdin(&script);
    assert_eq!(stdout, "FROMFILE2\nrc=0\n");
    assert_eq!(code, Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn source_dev_stdin_heredoc() {
    let (stdout, _, code) = rubash_c_null_stdin(
        "source /dev/stdin <<EOF\necho FROMHEREDOC\nEOF\necho \"rc=$?\"",
    );
    assert_eq!(stdout, "FROMHEREDOC\nrc=0\n");
    assert_eq!(code, Some(0));
}

#[test]
fn source_dev_stdin_pipe_still_works() {
    let (stdout, _, code) =
        rubash_c_piped_stdin("source /dev/stdin; echo \"rc=$?\"", "echo SRCSTDIN\n");
    assert_eq!(stdout, "SRCSTDIN\nrc=0\n");
    assert_eq!(code, Some(0));
}

#[test]
fn source_dev_stdin_explicit_redirect_wins_over_pipe() {
    let (stdout, _, code) = rubash_c_piped_stdin(
        "source /dev/stdin <<< 'echo FROMREDIR'; echo \"rc=$?\"",
        "echo FROMPIPE\n",
    );
    assert_eq!(stdout, "FROMREDIR\nrc=0\n");
    assert_eq!(code, Some(0));
}

#[test]
fn source_dev_fd0_alias_herestring() {
    let (stdout, _, code) =
        rubash_c_null_stdin("source /dev/fd/0 <<< 'echo FD0'; echo \"rc=$?\"");
    assert_eq!(stdout, "FD0\nrc=0\n");
    assert_eq!(code, Some(0));
}

#[test]
fn source_dev_stdin_missing_redirect_fails_fast() {
    let (stdout, stderr, code) =
        rubash_c_null_stdin("source /dev/stdin < /missing-xyz-1642; echo \"rc=$?\"");
    assert_eq!(stdout, "rc=1\n");
    assert!(stderr.contains("/missing-xyz-1642"), "stderr: {stderr}");
    assert_eq!(code, Some(0));
}
