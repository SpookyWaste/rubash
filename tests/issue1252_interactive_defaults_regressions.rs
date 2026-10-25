//! Regression pins for rubash#1252: interactive shells did not apply
//! the interactive `set -o` defaults — monitor mode stayed off, `$-`
//! lacked `m`, and `fg`/`bg` failed with `no job control` even though
//! the job-control machinery existed (`set -m` by hand worked around
//! it, confirming a defaults gap, not missing machinery).
//!
//! GNU bash 5.3 (shell.c / flags: an interactive shell enables monitor
//! mode — `set -o monitor`, the `m` in `$-` — along with history and
//! histexpand; jobs.c then lets fg/bg attach). The issue's oracle was
//! GNU on a PTY: `monitor on`, `$-` = `himBH`, `sleep 30 & fg` attaches.
//! GNU only reaches that state with a terminal; without one it leaves
//! monitor off. This engine applies the interactive defaults whenever
//! interactive mode is requested (-i), which is the form CI can pin,
//! and the effective values match the GNU-on-PTY contract exactly:
//!
//!   fixed rubash -i -c:  `monitor on`, `$-` = himBHc, fg attaches rc 0
//!   fixed rubash -c:     `monitor off` (control — the default must not
//!                        leak into non-interactive shells)
//!
//! The histexpand/history halves of the interactive defaults are
//! rubash#1248's surface and are pinned with it; this file pins the
//! monitor half only, per the issue's own dup-check.

use std::process::Command;

fn rubash(args: &[&str], script: &str) -> (String, String, Option<i32>) {
    let out = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .args(args)
        .arg(script)
        .output()
        .expect("spawn rubash");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code(),
    )
}

#[test]
fn issue1252_interactive_enables_monitor_and_m_flag() {
    let (stdout, _e, _c) = rubash(&["-i", "-c"], "set -o | grep -E '^monitor'; echo FLAGS=$-");
    assert!(
        stdout
            .lines()
            .any(|l| l.starts_with("monitor") && l.ends_with("on")),
        "interactive shell must default monitor on, got: {stdout:?}"
    );
    let flags = stdout
        .lines()
        .find(|l| l.starts_with("FLAGS="))
        .unwrap_or("");
    assert!(
        flags.contains('m'),
        "$- must carry m in interactive mode (GNU-on-PTY: himBH), got: {flags:?}"
    );
}

#[test]
fn issue1252_interactive_fg_attaches_without_set_m() {
    let (stdout, stderr, _c) = rubash(&["-i", "-c"], "sleep 0.5 & fg; echo fg_rc=$?");
    assert!(
        stdout.contains("fg_rc=0"),
        "fg must attach to the running job interactively, got stdout: {stdout:?} stderr: {stderr:?}"
    );
    assert!(
        !stderr.contains("no job control") && !stdout.contains("no job control"),
        "the 'no job control' failure was the bug, got stderr: {stderr:?}"
    );
}

#[test]
fn issue1252_noninteractive_monitor_stays_off_control() {
    let (stdout, _e, _c) = rubash(&["-c"], "set -o | grep -E '^monitor'");
    assert!(
        stdout
            .lines()
            .any(|l| l.starts_with("monitor") && l.ends_with("off")),
        "non-interactive shells keep monitor off, got: {stdout:?}"
    );
}
