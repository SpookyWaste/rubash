//! rubash#1496 — write-error diagnostics lacked the command name
//! and the "write error" wording (errors were attributed to the
//! shell itself). GNU: "echo: write error: No space left on device"
//! for the builtin, the program's own path-prefixed line for an
//! external command, "printf: write error: ..." for printf; all
//! three commands return 1 on /dev/full.
//!
//! Probe: clusters/x4/repro/1496.sh.
//! Expectations verified against GNU bash 5.3.20 (script files).

#![cfg(target_os = "linux")]

use std::path::PathBuf;
use std::process::{Command, Stdio};

fn tempdir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rubash1496-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create tempdir");
    dir
}

#[test]
fn write_errors_name_the_command() {
    let dir = tempdir("writeerr");
    let script = dir.join("probe.sh");
    std::fs::write(
        &script,
        "echo hi > /dev/full; echo \"rc=$?\"\n\
         /bin/echo hi > /dev/full; echo \"rc2=$?\"\n\
         printf 'hi\\n' > /dev/full; echo \"rc3=$?\"\n",
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
    assert_eq!(stdout, "rc=1\nrc2=1\nrc3=1\n");
    assert!(
        stderr.contains("echo: write error: No space left on device"),
        "stderr: {stderr}"
    );
    assert!(
        stderr.contains("/bin/echo: write error: No space left on device"),
        "stderr: {stderr}"
    );
    assert!(
        stderr.contains("printf: write error: No space left on device"),
        "stderr: {stderr}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
