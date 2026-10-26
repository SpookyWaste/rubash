//! Issue rubash#1607 (residual of #1494): a stop the shell's own
//! `kill` sent becomes visible to `jobs` at a NOTIFICATION POINT
//! (GNU learns it asynchronously via jobs.c sigchld_handler ->
//! waitchld, and waitchld also runs when the shell blocks in
//! wait_for) — not on a fixed reader-command count. With only a fast
//! builtin intervening, GNU still shows the pre-signal state and the
//! old current/previous marks; once a real wait point has passed
//! (here: a foreground `sleep`), the stopped state appears and the
//! stopped job is promoted to current. Rubash's deferred marks were
//! applied purely by command count (seq + 1 < now), publishing one
//! command too early in the fast-builtin case.

use std::process::Command;

fn rubash(script: &str) -> (String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(script)
        .output()
        .expect("run rubash");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn job_line(stdout: &str, id: u32) -> String {
    let prefix = format!("[{id}]");
    stdout
        .lines()
        .find(|line| line.starts_with(&prefix))
        .unwrap_or_else(|| panic!("no [{id}] line in jobs output: {stdout:?}"))
        .to_string()
}

/// The issue seed: one fast builtin between kill -STOP and jobs —
/// GNU shows both jobs Running with the ORIGINAL marks ([1]- [2]+).
#[test]
fn stopped_state_not_published_after_one_fast_builtin() {
    let script = "\
set -m
sleep 30 &
sleep 30 &
kill -STOP %1
echo intervening-builtin
jobs
kill -9 %1 %2 2>/dev/null; wait 2>/dev/null
";
    let (stdout, stderr) = rubash(script);
    let one = job_line(&stdout, 1);
    let two = job_line(&stdout, 2);
    assert!(
        one.starts_with("[1]-") && one.contains("Running"),
        "job 1 published too early: {one:?} (stderr={stderr:?})"
    );
    assert!(
        two.starts_with("[2]+") && two.contains("Running"),
        "job 2 mark disturbed: {two:?} (stderr={stderr:?})"
    );
}

/// The mirror: after a foreground wait point the stop IS published
/// and the stopped job becomes current (GNU wait_for -> waitchld).
#[test]
fn stopped_state_published_after_foreground_wait() {
    let script = "\
set -m
sleep 30 &
sleep 30 &
kill -STOP %1
sleep 0.3
jobs
kill -9 %1 %2 2>/dev/null; wait 2>/dev/null
";
    let (stdout, stderr) = rubash(script);
    let one = job_line(&stdout, 1);
    let two = job_line(&stdout, 2);
    assert!(
        one.starts_with("[1]+") && one.contains("Stopped"),
        "job 1 stop never published: {one:?} (stderr={stderr:?})"
    );
    assert!(
        two.starts_with("[2]-") && two.contains("Running"),
        "job 2 mark not demoted: {two:?} (stderr={stderr:?})"
    );
}

/// Same-line form: `kill -STOP %1; jobs` never observes the stop.
#[test]
fn stopped_state_not_published_on_same_line() {
    let script = "\
set -m
sleep 30 &
kill -STOP %1; jobs
kill -9 %1 2>/dev/null; wait 2>/dev/null
";
    let (stdout, stderr) = rubash(script);
    let one = job_line(&stdout, 1);
    assert!(
        one.contains("Running") && !one.contains("Stopped"),
        "same-line jobs saw the stop: {one:?} (stderr={stderr:?})"
    );
}
