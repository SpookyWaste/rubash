//! Regression pins for the ms-stopbleed acceptance repairs (PR #1778,
//! acceptance round 2). The multi-session harness caught three real
//! divergence classes on top of the G-01/G-02/G-03 stop-bleed designs;
//! each class gets one focused, deterministic pin here (the harness
//! itself remains the end-to-end instrument):
//!
//! 1. G-02 residual — relative redirect data paths and in-process
//!    emulated-command operands resolved against the PROCESS cwd
//!    instead of the session cwd: every session's `>> rel` appends
//!    accumulated into the test process's own `rel.txt` while the
//!    per-session files stayed empty, and the emulated `cat rel` read
//!    the shared file back. Spawn `current_dir` stamps were already
//!    correct; the in-process opens were not.
//! 2. G-03 child visibility — a session's mirrored assignment
//!    (`__RUBASH_*` family) never reached spawned children because
//!    `apply_child_environment` env_clear()s and rebuilt only from the
//!    session's exported table, never projecting the process-env
//!    mirror the assignment wrote through to.
//! 3. G-01/G-03 residual — host-API `set_env` writes of the
//!    `__RUBASH_IS_C` / `__RUBASH_INTERACTIVE` protocol markers escaped
//!    the G-03 write journal (set_env pushed no journal guard), so they
//!    leaked past the owner's Drop into the process env and were
//!    imported by every later Executor in the process, flipping its
//!    invocation classification (the six cross-test lib failures).
//!
//! Everything lives in a single #[test] on purpose: each scenario
//! mutates process-global state by design, and cargo runs a binary's
//! tests on parallel threads of one process.
//!
//! Run with: cargo test --test ms_stopbleed_regressions --locked

use rubash::executor::Executor;
use rubash::script_driver::run_source;
use std::path::{Path, PathBuf};

fn shell_path(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// Drive one script step and require success, harness-style.
fn run(executor: &mut Executor, source: &str) {
    assert_eq!(
        run_source(executor, source, false),
        0,
        "step failed: {source}"
    );
}

#[test]
fn stopbleed_repairs_relative_paths_child_env_and_marker_leak() {
    let root =
        std::env::temp_dir().join(format!("rubash-ms-stopbleed-regr-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let sess = root.join("sess");
    std::fs::create_dir_all(&sess).expect("create session dir");

    // The leak target the buggy code wrote to: the process cwd. Clear
    // any stale file from an earlier (buggy) run so existence below is
    // evidence about THIS run only.
    let process_cwd_file = PathBuf::from("msregr_rel.txt");
    let _ = std::fs::remove_file(&process_cwd_file);

    // ---- Class 1: relative redirect writes + emulated cat reads ----
    let mut a = Executor::new();
    run(&mut a, &format!("cd {}", shell_path(&sess)));
    run(&mut a, "echo one >> msregr_rel.txt");
    run(&mut a, "echo two >> msregr_rel.txt");
    let written = std::fs::read_to_string(sess.join("msregr_rel.txt"))
        .expect("session dir must hold the appends");
    assert_eq!(
        written, "one\ntwo\n",
        "relative >> appends must land in the session cwd"
    );
    assert!(
        !process_cwd_file.exists(),
        "relative >> appends leaked into the process cwd"
    );
    run(&mut a, "cat msregr_rel.txt > msregr_cat.txt");
    let read_back = std::fs::read_to_string(sess.join("msregr_cat.txt"))
        .expect("cat output file must exist in the session dir");
    assert_eq!(
        read_back, "one\ntwo\n",
        "relative cat operand must read from the session cwd"
    );
    drop(a);

    // ---- Class 2: mirrored assignment reaches spawned children ----
    let mut b = Executor::new();
    run(&mut b, &format!("cd {}", shell_path(&sess)));
    run(&mut b, "__RUBASH_MSREGR_PROBE=probe-b");
    run(&mut b, "MSREGR_PLAIN_UNEXPORTED=nope");
    run(&mut b, "/usr/bin/env > msregr_env.txt");
    let dump = std::fs::read_to_string(sess.join("msregr_env.txt"))
        .expect("env dump must exist in the session dir");
    assert!(
        dump.lines()
            .any(|line| line == "__RUBASH_MSREGR_PROBE=probe-b"),
        "spawned child must see the session's mirrored __RUBASH_* assignment; dump:\n{dump}"
    );
    assert!(
        !dump.contains("MSREGR_PLAIN_UNEXPORTED"),
        "an unexported non-mirror variable must stay out of the child env; dump:\n{dump}"
    );
    drop(b);
    assert!(
        std::env::var("__RUBASH_MSREGR_PROBE").is_err(),
        "the mirror write must be rolled back when its owner drops"
    );

    // ---- Class 3: set_env protocol markers die with their owner ----
    {
        let mut c = Executor::new();
        c.set_env("__RUBASH_IS_C", "1");
        c.set_env("__RUBASH_INTERACTIVE", "1");
    }
    assert!(
        std::env::var("__RUBASH_IS_C").is_err(),
        "__RUBASH_IS_C set via set_env leaked past Drop into the process env"
    );
    assert!(
        std::env::var("__RUBASH_INTERACTIVE").is_err(),
        "__RUBASH_INTERACTIVE set via set_env leaked past Drop into the process env"
    );
    let d = Executor::new();
    assert!(
        d.get_env("__RUBASH_IS_C").is_none(),
        "a fresh Executor must not import a dropped sibling's __RUBASH_IS_C"
    );
    assert!(
        d.get_env("__RUBASH_INTERACTIVE").is_none(),
        "a fresh Executor must not import a dropped sibling's __RUBASH_INTERACTIVE"
    );
    drop(d);

    let _ = std::fs::remove_file(&process_cwd_file);
    let _ = std::fs::remove_dir_all(&root);
}
