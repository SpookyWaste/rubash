//! rubash#1596 (gen-corpus R5): an external command's own stderr must
//! pass through verbatim — never wrapped in the host shell's
//! diagnostic envelope (`<script>: line N: `). The emulated coreutils
//! programs (cat/cp/touch/...) formatted their diagnostics through
//! `diagnostic_prefix()`; GNU coreutils writes them bare.
//!
//! Oracle: GNU bash 5.3.20 + coreutils, script files.

#![cfg(unix)]

use std::process::{Command, Stdio};

/// Local stand-in for the `tempfile` crate (not a dev-dependency of
/// this repo): a unique scratch directory removed on drop.
struct ScratchDir(std::path::PathBuf);

impl ScratchDir {
    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn scratch_tempdir() -> ScratchDir {
    let dir = std::env::temp_dir().join(format!(
        "rubash-scratch-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    ScratchDir(dir)
}

fn rubash_bin() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_rubash"))
}

struct Run {
    stdout: String,
    stderr: String,
    status: i32,
    dir: ScratchDir,
}

fn run_script(script: &str) -> Run {
    let dir = scratch_tempdir();
    let script_path = dir.path().join("case.sh");
    std::fs::write(&script_path, script).expect("write script");
    let output = Command::new(rubash_bin())
        .arg(&script_path)
        .env_clear()
        .env("PATH", "/usr/local/bin:/usr/bin:/bin")
        .env("HOME", dir.path())
        .current_dir(dir.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn rubash");
    Run {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        status: output.status.code().unwrap_or(-1),
        dir,
    }
}

#[test]
fn issue1596_cat_closed_stdin_stderr_is_bare() {
    // The issue's regression seed.
    let run = run_script("cat <&-\necho \"rc=$?\"\n");
    assert_eq!(
        run.stderr,
        "cat: -: Bad file descriptor\ncat: closing standard input: Bad file descriptor\n",
        "stdout: {:?}",
        run.stdout
    );
    assert_eq!(run.stdout, "rc=1\n");
}

#[test]
fn issue1596_cat_closed_stdin_stderr_redirected_is_bare() {
    // The redirected path: the bare lines land in the target file and
    // nothing reaches the shell's stderr.
    let run = run_script("cat <&- 2>err.txt\necho \"rc=$?\"\n");
    assert_eq!(run.stderr, "", "stdout: {:?}", run.stdout);
    assert_eq!(run.stdout, "rc=1\n");
    let err = std::fs::read_to_string(run.dir.path().join("err.txt")).expect("err.txt");
    assert_eq!(
        err,
        "cat: -: Bad file descriptor\ncat: closing standard input: Bad file descriptor\n"
    );
}

#[test]
fn issue1596_touch_diagnostic_is_bare() {
    let run = run_script("touch --zzz-opt-1596\n");
    let first = run.stderr.lines().next().unwrap_or("");
    assert_eq!(first, "touch: unrecognized option '--zzz-opt-1596'");
    assert!(!run.stderr.contains("line 1:"), "stderr: {:?}", run.stderr);
    let _ = run.status;
}

#[test]
fn issue1596_cp_diagnostic_is_bare() {
    let run = run_script("cp /nonexistent_src_1596 dst\n");
    assert_eq!(
        run.stderr,
        "cp: cannot stat '/nonexistent_src_1596': No such file or directory\n"
    );
    assert_eq!(run.status, 1);
}

#[test]
fn issue1596_shell_diagnostics_keep_their_envelope() {
    // Control: the SHELL's own diagnostics (command not found) keep
    // the script envelope — only the child's bytes are verbatim.
    let run = run_script("nosuchcmd_1596\n");
    assert!(
        run.stderr
            .contains("line 1: nosuchcmd_1596: command not found"),
        "stderr: {:?}",
        run.stderr
    );
    assert_eq!(run.status, 127);
}
