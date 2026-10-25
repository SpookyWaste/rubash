//! rubash#1643 — a bare `builtin` with only output redirections
//! ignored them: `builtin > f` created no file, `builtin > f` on an
//! existing file did not truncate it.
//!
//! GNU ownership: execute_builtin applies a simple command's
//! redirections before the builtin runs, even when the builtin
//! takes no arguments. The fix applies the same no-output-builtin
//! redirect pass the `:` arm already uses to the bare-`builtin`
//! early return in builtin_direct_command.rs (that return had
//! skipped redirection entirely; `command` and `:` take their own
//! redirect passes and already agreed with GNU).
//!
//! Expectations verified against GNU bash. Each test runs in its
//! own scratch directory so the created/truncated files are the
//! test's own evidence.

use std::process::Command;

fn scratch_dir() -> std::path::PathBuf {
    let tag = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    );
    let dir = std::env::temp_dir().join(format!("rubash-1643-{tag}"));
    std::fs::create_dir(&dir).expect("create scratch dir");
    dir
}

fn rubash_c(dir: &std::path::Path, script: &str) -> (String, String, Option<i32>) {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(script)
        .current_dir(dir)
        .output()
        .expect("run rubash");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code(),
    )
}

#[test]
fn bare_builtin_creates_truncate_target() {
    let dir = scratch_dir();
    let (stdout, _stderr, code) = rubash_c(&dir, "builtin > f; echo AFTER; ls f");
    assert_eq!(stdout, "AFTER\nf\n");
    assert_eq!(code, Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bare_builtin_append_and_stderr_targets() {
    let dir = scratch_dir();
    let (_stdout, _stderr, code) = rubash_c(&dir, "builtin >> f; builtin 2> e");
    assert_eq!(code, Some(0));
    assert!(dir.join("f").exists(), "append target not created");
    assert!(dir.join("e").exists(), "stderr target not created");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bare_builtin_truncates_existing_file() {
    let dir = scratch_dir();
    std::fs::write(dir.join("f"), "OLD\n").expect("seed file");
    let (stdout, _stderr, code) = rubash_c(&dir, "builtin > f; wc -c < f");
    assert_eq!(stdout, "0\n");
    assert_eq!(code, Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bare_builtin_failed_redirect_reports_status_1() {
    let dir = scratch_dir();
    let (stdout, stderr, code) = rubash_c(&dir, "builtin > no-dir/f; echo rc=$?");
    assert!(stderr.contains("no-dir/f"), "stderr: {stderr}");
    // The failing redirect makes `builtin` itself status 1, printed
    // by the following echo; the script as a whole still exits 0.
    assert_eq!(stdout, "rc=1\n");
    assert_eq!(code, Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bare_builtin_dashdash_redirect_and_plain_noop() {
    let dir = scratch_dir();
    let (stdout, _stderr, code) = rubash_c(&dir, "builtin -- > f; ls f; builtin; echo rc=$?");
    assert_eq!(stdout, "f\nrc=0\n");
    assert_eq!(code, Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}
