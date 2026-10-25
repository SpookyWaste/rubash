//! rubash#1369 — `command time ...` / `builtin time ...` ran the time
//! KEYWORD; GNU looks `time` up as a command/builtin name and fails
//! (127 "command not found" / 1 "not a shell builtin"), while
//! `command -v time` and `type time` still report the keyword.
//!
//! Probe: clusters/z4/probes/final/1369.sh (+ gnu-1369.out/.err).
//! Expectations verified against GNU bash 5.3.20 (script files).

use std::path::PathBuf;
use std::process::{Command, Stdio};

fn tempdir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rubash1369-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create tempdir");
    dir
}

#[test]
fn command_and_builtin_do_not_run_time_keyword() {
    let dir = tempdir("time");
    let script = dir.join("probe.sh");
    std::fs::write(
        &script,
        "command time true; echo rc=$?\n\
         builtin time true; echo rc=$?\n\
         command time -p true; echo rc=$?\n\
         command -v time; echo v_rc=$?\n\
         type time; echo type_rc=$?\n\
         command true; echo cmd_true_rc=$?\n\
         builtin echo hi; echo builtin_echo_rc=$?\n",
    )
    .expect("write script");
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("probe.sh")
        .current_dir(&dir)
        .stdin(Stdio::null())
        .output()
        .expect("run rubash");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(
        stdout,
        "rc=127\nrc=1\nrc=127\ntime\nv_rc=0\ntime is a shell keyword\ntype_rc=0\ncmd_true_rc=0\nhi\nbuiltin_echo_rc=0\n"
    );
    assert_eq!(
        stderr.matches("time: command not found").count(),
        2,
        "stderr: {stderr}"
    );
    assert!(
        stderr.contains("builtin: time: not a shell builtin"),
        "stderr: {stderr}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
