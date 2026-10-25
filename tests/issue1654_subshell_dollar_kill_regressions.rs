//! rubash#1654 — a `kill` to `$$` from inside a flat `( ... )` subshell
//! was judged by the subshell's own trap table instead of the parent's.
//! GNU's subshell is a forked process (execute_cmd.c:1576
//! execute_in_subshell), so a signal to `$$` reaches only the parent
//! process: a subshell `trap '' TERM` cannot swallow it, and a subshell
//! `trap '...' TERM` cannot catch it either — the parent, with no trap,
//! dies 143 in both cases. Rubash's flat subshell shares the process
//! and unix delivery is a real `raise` judged by the kernel disposition
//! at raise time, so the subshell's ignore discarded the signal and
//! the parent survived with `parent=0`.
//!
//! Fix: execute_kill defers a `$$`-kill when the subshell layer itself
//! arms that signal, and restore_flat_subshell replays it once the
//! depth returns to the `$$`-owning level, after reconciling the
//! kernel dispositions from the restored (parent) trap table.
//!
//! Expectations verified against GNU bash 5.3.20 (script files).
//! Signal deaths are asserted via ExitStatusExt::signal(): a process
//! killed by TERM has no exit code, and the outer shell reports 143.

use std::os::unix::process::ExitStatusExt;
use std::process::Command;

fn rubash(script: &str) -> (String, String, Option<i32>) {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(script)
        .output()
        .expect("run rubash");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.signal(),
    )
}

#[test]
fn subshell_ignore_cannot_swallow_dollar_kill() {
    let (stdout, _stderr, signal) =
        rubash("(trap '' TERM; kill -s TERM $$; echo sub); echo parent");
    assert_eq!(stdout, "sub\n");
    assert_eq!(signal, Some(15));
}

#[test]
fn subshell_trap_action_cannot_catch_dollar_kill() {
    let (stdout, _stderr, signal) = rubash(
        "(trap 'echo got' TERM; kill -s TERM $$; echo subalive); echo parentalive",
    );
    // GNU: the subshell's trap never fires (the signal went to the
    // parent pid, not the subshell process) and the parent dies.
    assert_eq!(stdout, "subalive\n");
    assert_eq!(signal, Some(15));
}

#[test]
fn parent_ignore_survives_deferred_dollar_kill() {
    let (stdout, _stderr, signal) = rubash(
        "trap '' TERM\n(trap 'echo SUB-TRAP' TERM; kill -s TERM $$; echo sub); echo parent=$?",
    );
    assert_eq!(stdout, "sub\nparent=0\n");
    assert_eq!(signal, None);
}

#[test]
fn nested_subshells_finish_before_parent_is_judged() {
    let (stdout, _stderr, signal) =
        rubash("((trap '' TERM; kill -s TERM $$; echo inner); echo outer); echo parent");
    assert_eq!(stdout, "inner\nouter\n");
    assert_eq!(signal, Some(15));
}

#[test]
fn untrapped_dollar_kill_still_dies_at_raise() {
    // No trap entry in the subshell layer: the legacy path raises
    // immediately under the parent's default disposition.
    let (stdout, _stderr, signal) =
        rubash("(kill -s TERM $$; echo inside); echo outside");
    assert_eq!(stdout, "");
    assert_eq!(signal, Some(15));
}

#[test]
fn trap_for_other_signal_does_not_defer_dollar_kill() {
    // Only an entry for the killed signal itself defers delivery.
    let (stdout, _stderr, signal) =
        rubash("(trap '' USR1; kill -s TERM $$; echo sub); echo parent");
    assert_eq!(stdout, "");
    assert_eq!(signal, Some(15));
}
