//! Issue rubash#1661: on the plain (untimed) dispatch path, a null
//! command produced by a command substitution — `$(false)` as a whole
//! simple command — never fired the ERR trap under `set -e`, and the
//! redirect-carrying form lost the substitution's status entirely.
//!
//! GNU execute_cmd.c: a failed simple command is adjudicated by
//! run_error_trap BEFORE the ERREXIT jump, and execute_null_command's
//! status is the last command substitution status from the command's
//! expansion. The assignment/redirect forms already fired via the
//! rubash#1171 tail of execute_empty_words_command; the bare
//! vanished-words arm in execute_command converted to
//! Err(ExitCode) without firing, and the empty-words tail started its
//! status at 0, clobbering the substitution status for
//! `$(false) >/dev/null`.
//!
//! Consolidation follow-up (same issue): both conversion sites
//! initially fired without the rubash#1611 timed-dispatch deferral,
//! so under `time` the bare form fired twice and the assignment form
//! converted unmarked (exit 127 via the genuine-unwind arm). Both
//! sites now follow the #1611 mechanism — depth-guarded fire plus
//! the errexit_status_converted mark — leaving the ast_exec time
//! branch as the single adjudication site for timed commands.
//!
//! Expectations verified against GNU bash 5.3.20 (self-compiled
//! oracle), script-file mode authoritative.

use std::process::Command;

fn rubash_file(script: &str) -> (String, String, Option<i32>) {
    let dir = std::env::temp_dir().join(format!(
        "rubash-issue1661-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    std::fs::write(dir.join("case.sh"), script).expect("write case.sh");
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("case.sh")
        .current_dir(&dir)
        .output()
        .expect("run rubash file");
    let _ = std::fs::remove_dir_all(&dir);
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code(),
    )
}

#[test]
fn issue1661_bare_null_command_fires_err_trap_under_errexit() {
    // The issue's Regression seed, verbatim in shape.
    let (stdout, _, code) = rubash_file("set -e\ntrap 'echo ERRFIRED' ERR\n$(false)\necho done\n");
    assert_eq!(stdout, "ERRFIRED\n");
    assert_eq!(code, Some(1));
}

#[test]
fn issue1661_bare_null_command_fires_err_trap_once_without_errexit() {
    let (stdout, _, code) = rubash_file("trap 'echo ERRFIRED' ERR\n$(false)\necho done\n");
    assert_eq!(stdout, "ERRFIRED\ndone\n");
    assert_eq!(code, Some(0));
}

#[test]
fn issue1661_redirected_null_command_fires_err_trap_under_errexit() {
    let (stdout, _, code) =
        rubash_file("set -e\ntrap 'echo ERRFIRED' ERR\n$(false) >/dev/null\necho done\n");
    assert_eq!(stdout, "ERRFIRED\n");
    assert_eq!(code, Some(1));
}

#[test]
fn issue1661_null_command_keeps_substitution_status() {
    let (stdout, _, code) = rubash_file(
        "$(exit 3) >/dev/null\necho \"st=$?\"\n\
         $(false) >/dev/null\necho \"st2=$?\"\n\
         x=$(false)\necho \"st3=$?\"\n\
         \"$(false)\"\necho \"st4=$?\"\n\
         `exit 3` >/dev/null\necho \"st5=$?\"\n",
    );
    assert_eq!(stdout, "st=3\nst2=1\nst3=1\nst4=127\nst5=3\n");
    assert_eq!(code, Some(0));
}

#[test]
fn issue1661_seed_does_not_leak_across_commands() {
    // The substitution-status channel is only attributable when the
    // null command's own words carried a substitution: a plain
    // assignment+redirect null command after a substitution-bearing
    // command keeps status 0 (the pre-fix behavior for that shape).
    let (stdout, _, code) =
        rubash_file("echo $(false) >/dev/null\nx=1 >/dev/null\necho \"st=$?\"\n");
    assert_eq!(stdout, "st=0\n");
    assert_eq!(code, Some(0));
}

#[test]
fn issue1661_assignment_only_form_still_fires_err_trap() {
    // Control: the rubash#1171 arm — assignment-only null commands
    // already fired before this fix and must keep firing exactly once.
    let (stdout, _, code) =
        rubash_file("set -e\ntrap 'echo ERRFIRED' ERR\nx=$(false)\necho done\n");
    assert_eq!(stdout, "ERRFIRED\n");
    assert_eq!(code, Some(1));
}

#[test]
fn issue1661_timed_bare_null_command_fires_err_trap_exactly_once() {
    // The consolidation stop: the bare arm fired unguarded, and the
    // ast_exec time branch fired again for the same command, so
    // `time $(false)` printed ERRFIRED twice (GNU: once). The time
    // report must still print and the exit status stays 1.
    let (stdout, stderr, code) =
        rubash_file("set -e\ntrap 'echo ERRFIRED' ERR\ntime $(false)\necho done\n");
    assert_eq!(stdout, "ERRFIRED\n");
    assert!(stderr.contains("real"), "time report missing: {stderr}");
    assert_eq!(code, Some(1));
}

#[test]
fn issue1661_timed_assignment_null_command_fires_once_and_exits_1() {
    // The assignment-only tail converted without the rubash#1611
    // conversion mark, so the timed frame mistook the conversion
    // for a genuine unwind and the script exited 127. GNU: the
    // trap fires exactly once, the report prints, exit 1.
    let (stdout, stderr, code) =
        rubash_file("set -e\ntrap 'echo ERRFIRED' ERR\ntime x=$(false)\necho done\n");
    assert_eq!(stdout, "ERRFIRED\n");
    assert!(stderr.contains("real"), "time report missing: {stderr}");
    assert_eq!(code, Some(1));
}

#[test]
fn issue1661_timed_plain_failure_fires_err_trap_exactly_once() {
    // Control: an ordinary failing command under `time` — the
    // dispatch-level conversion sites already defer to the time
    // branch; the null-command forms above must match this count.
    let (stdout, stderr, code) =
        rubash_file("set -e\ntrap 'echo ERRFIRED' ERR\ntime false\necho done\n");
    assert_eq!(stdout, "ERRFIRED\n");
    assert!(stderr.contains("real"), "time report missing: {stderr}");
    assert_eq!(code, Some(1));
}
