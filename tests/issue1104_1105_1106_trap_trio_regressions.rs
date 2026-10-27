//! Trap trio rubash#1104/#1105/#1106.
//!
//! #1104 (P1): an EXIT trap installed inside a function must fire when the
//! call IS the whole command-substitution body or pipeline segment. GNU
//! subst.c:7340-7341 ends the comsub child with `rc = run_exit_trap ();
//! exit (rc)` (captured into the substitution), and execute_cmd.c:1761-1763
//! runs the subshell EXIT trap at every pipeline element's exit. The
//! `$(f)` word-level fast path and the function/builtin pipeline stages
//! dropped it (no reset + no exit-trap run).
//!
//! #1105: nested RETURN traps are global last-set-wins. GNU
//! execute_cmd.c:5291-5303 removes the inherited RETURN trap at entry only
//! for untraced functions (restoring it at exit iff the body left it
//! untrapped, trap_if_untrapped), and the fire itself is unconditional — so
//! an inner install fires at the OUTER return too. The old function-scoped
//! gate dropped the outer fire (and the missing entry save/restore dropped
//! the v4 restore).
//!
//! #1106 (P3, pathological): `return N` inside a RETURN action re-arms
//! the trap in GNU (trap.c _run_trap_internal rethrows through
//! return_catch and execute_cmd.c:5377 refires because the
//! from_return_trap setter is `#if 0`'d out, trap.c:1252 — bash 5.3.20
//! recurses forever). Rubash loops the fire the same way.
//!
//! All expectations byte-verified against bash 5.3.20 via script files.

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
        output.status.code(),
    )
}

/// #1104: `x=$(f)` captures the EXIT trap output installed inside f.
#[test]
fn exit_trap_inside_function_fires_in_comsub_body() {
    let (stdout, stderr, code) =
        rubash("f() { trap 'echo EXIT-TRAP' EXIT; echo body; }\nx=$(f); echo \"captured=[$x]\"");
    assert_eq!(stderr, "");
    assert_eq!(stdout, "captured=[body\nEXIT-TRAP]\n");
    assert_eq!(code, Some(0));
}

/// #1104: `f | cat` carries the function-installed EXIT trap down the pipe.
#[test]
fn exit_trap_inside_function_fires_in_pipeline_segment() {
    let (stdout, stderr, code) = rubash("f() { trap 'echo EXIT-TRAP' EXIT; echo body; }\nf | cat");
    assert_eq!(stderr, "");
    assert_eq!(stdout, "body\nEXIT-TRAP\n");
    assert_eq!(code, Some(0));
}

/// #1104 family: `trap 'echo X' EXIT | cat` fires at the element's exit.
#[test]
fn exit_trap_builtin_as_pipeline_segment_fires() {
    let (stdout, stderr, code) = rubash("trap 'echo X' EXIT | cat\necho done");
    assert_eq!(stderr, "");
    assert_eq!(stdout, "X\ndone\n");
    assert_eq!(code, Some(0));
}

/// #1104 guard: a parent EXIT trap must not fire inside the stage.
#[test]
fn parent_exit_trap_does_not_fire_in_pipeline_stage() {
    let (stdout, stderr, code) =
        rubash("trap 'echo PARENT-EXIT' EXIT\nf() { echo body; }\nf | cat");
    assert_eq!(stderr, "");
    assert_eq!(stdout, "body\nPARENT-EXIT\n");
    assert_eq!(code, Some(0));
}

/// #1105: nested install is last-set-wins — the outer return runs B too.
#[test]
fn nested_return_trap_outer_return_runs_inner_action() {
    let (stdout, stderr, code) = rubash(
        "outer() { trap 'echo A' RETURN; inner; }\ninner() { trap 'echo B' RETURN; echo in-inner; }\nouter",
    );
    assert_eq!(stderr, "");
    assert_eq!(stdout, "in-inner\nB\nB\n");
    assert_eq!(code, Some(0));
}

/// #1105: a body `trap - RETURN` still restores the saved action at exit.
#[test]
fn return_trap_removed_in_inner_restores_outer_action() {
    let (stdout, stderr, code) = rubash(
        "outer() { trap 'echo A' RETURN; inner; }\ninner() { trap - RETURN; echo in-inner; }\nouter",
    );
    assert_eq!(stderr, "");
    assert_eq!(stdout, "in-inner\nA\n");
    assert_eq!(code, Some(0));
}

/// #1105 guard: untraced functions neither inherit nor leak the trap.
#[test]
fn untraced_nested_functions_do_not_fire_inherited_return_trap() {
    let (stdout, stderr, code) = rubash(
        "trap 'echo R' RETURN\nouter() { inner; }\ninner() { echo in-inner; }\nouter\necho done",
    );
    assert_eq!(stderr, "");
    assert_eq!(stdout, "in-inner\ndone\n");
    assert_eq!(code, Some(0));
}

/// #1106: `return N` in the RETURN action re-arms the trap (GNU
/// recurses forever here — verified with `timeout`, bash 5.3.20 never
/// prints and dies 124). The guard file lets the second firing exit
/// instead of hanging: GNU fires twice and exits 42 through the action;
/// the old absorb-once build fired once and printed `rc=9`.
#[test]
fn return_inside_return_trap_rearms_like_gnu() {
    let dir = std::env::temp_dir();
    let tag = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    );
    let guard = dir.join(format!("rubash-1106-guard-{tag}"));
    let guard_str = guard.to_string_lossy().into_owned();
    let (stdout, stderr, code) = rubash(&format!(
        "r() {{ trap 'if [ -e {guard_str} ]; then exit 42; fi; touch {guard_str}; return 3' RETURN; echo in-r; return 0; }}\nr; echo \"rc=$?\""
    ));
    let _ = std::fs::remove_file(&guard);
    assert_eq!(stderr, "");
    assert_eq!(stdout, "in-r\n");
    assert_eq!(code, Some(42));
}
