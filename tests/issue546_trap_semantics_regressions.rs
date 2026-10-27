//! rubash#546: trap semantics — the listing, in-action $BASH_COMMAND, and
//! the ERR+DEBUG rc combination.
//!
//! Three script-observable cells (WSL GNU 5.3.0(1) probes, 2026-10-12):
//!
//! cell 1 — a fresh shell's `trap` table is EMPTY: signals the shell
//!   ignores on its own behalf are not traps and never reach the listing.
//!   A signal ignored BEFORE startup (inherited SIG_IGN) IS listed —
//!   probe: a parent `trap '' RTMIN` makes the GNU child list
//!   `trap -- '' SIGRTMIN` — and rubash keeps that inherited-transport
//!   behavior via __RUBASH_TRAP_ORIG_IGN, which these tests strip to pin
//!   the fresh-shell contract.
//!
//! cell 2 — inside a trap action $BASH_COMMAND stays frozen at the value
//!   the trap fired with (variables.c:1558 get_bash_command reads
//!   the_printed_command_except_trap): an ERR action's DEBUG firings
//!   report the FAILED command, not the action's own commands.
//!
//! cell 3 — `trap false ERR; trap false DEBUG; echo x` leaves $? alone:
//!   `survived=0` (the trap actions' rc of 1 does not leak into the
//!   following command's status).

use std::process::Command;

fn run(script: &str) -> (String, String, Option<i32>) {
    sanitize_entry_ignores();
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .env_remove("__RUBASH_TRAP_ORIG_IGN")
        .env_remove("__RUBASH_TRAP_SIGRTMIN")
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

/// Reset launcher-inherited SIG_IGN dispositions (except SIGPIPE, which
/// the Rust runtime owns) to SIG_DFL in THIS test process before
/// spawning the shell under test. GNU snapshots entry dispositions at
/// startup (trap.c initialize_traps -> SIG_HARD_IGNORE, rubash#1124):
/// a dirty test-runner entry mask (e.g. Termux SigIgn 38 + SIGQUIT)
/// would otherwise seed `trap -- '' SIG` lines into every "fresh
/// shell" listing and refuse foreground installs. The fresh-shell
/// tests premise a clean entry; restoring it keeps them
/// host-independent (a no-op on clean CI runners). Script-level
/// ignores (`trap '' INT` in a parent script + spawn) still propagate
/// through the kernel and are unaffected.
#[cfg(unix)]
fn sanitize_entry_ignores() {
    for sig in 1..=64 {
        if sig == libc::SIGPIPE as i32 {
            continue;
        }
        let mut current: libc::sigaction = unsafe { std::mem::zeroed() };
        if unsafe { libc::sigaction(sig, std::ptr::null(), &mut current) } != 0 {
            continue;
        }
        if current.sa_sigaction == libc::SIG_IGN {
            unsafe {
                libc::signal(sig, libc::SIG_DFL);
            }
        }
    }
}

#[cfg(not(unix))]
fn sanitize_entry_ignores() {}

/// `trap -p` display lines a fresh child image lists with no explicit
/// traps: entry-ignores the host imposes at every exec, which no
/// process can reset. On this Termux host bionic re-applies SigIgn 38
/// (a realtime hole) in every new process image — even children of a
/// sanitized parent list it, exactly like GNU bash 5.3.20 does
/// (`trap -- '' 38`). Clean-entry hosts (all CI runners) yield "".
/// The explicit traps in these tests sort before every suffix signal,
/// so concatenation matches GNU's numeric canonical order.
fn live_entry_ignore_suffix() -> String {
    sanitize_entry_ignores();
    let mut command = Command::new(env!("CARGO_BIN_EXE_rubash"));
    command.env_remove("__RUBASH_TRAP_ORIG_IGN");
    command.env_remove("__RUBASH_TRAP_SIGRTMIN");
    // A rubash-based launcher exports its whole trap table; the
    // per-signal keys would seed the child independent of entry state.
    let leaked: Vec<String> = std::env::vars()
        .map(|(key, _)| key)
        .filter(|key| key.starts_with("__RUBASH_TRAP"))
        .collect();
    for key in leaked {
        command.env_remove(key);
    }
    let output = command
        .arg("-c")
        .arg("trap -p")
        .output()
        .expect("spawn rubash");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn fresh_shell_trap_listing_is_empty() {
    // On hosts with an immutable launcher entry-ignore (Termux SigIgn
    // 38) GNU lists it too — the "empty" contract holds modulo that
    // host suffix (empty on clean CI runners).
    let (stdout, stderr, rc) = run("trap");
    assert_eq!(stdout, live_entry_ignore_suffix());
    assert_eq!(stderr, "");
    assert_eq!(rc, Some(0));
}

#[test]
fn fresh_shell_trap_p_listing_is_empty() {
    let (stdout, stderr, rc) = run("trap -p");
    assert_eq!(stdout, live_entry_ignore_suffix());
    assert_eq!(stderr, "");
    assert_eq!(rc, Some(0));
}

#[test]
fn explicit_traps_still_list() {
    let (stdout, stderr, rc) = run("trap 'echo bad' TERM; trap '' HUP; (trap)");
    // Subshell keeps the parent's table; the shell-internal SIGRTMIN
    // ignore leaks nothing. An immutable host entry-ignore (Termux 38)
    // sorts after both explicit traps in GNU's numeric canonical order.
    assert_eq!(
        stdout,
        format!(
            "trap -- '' SIGHUP\ntrap -- 'echo bad' SIGTERM\n{}",
            live_entry_ignore_suffix()
        )
    );
    assert_eq!(stderr, "");
    assert_eq!(rc, Some(0));
}

#[test]
fn inherited_ignore_marker_still_lists() {
    // A signal ignored BEFORE startup is a real inherited ignore (GNU
    // probe: parent `trap '' RTMIN` -> child lists `trap -- '' SIGRTMIN`);
    // rubash transports it in __RUBASH_TRAP_ORIG_IGN. An immutable host
    // entry-ignore (SIGUSR2 is 12 < 38) sorts after it.
    sanitize_entry_ignores();
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .env("__RUBASH_TRAP_ORIG_IGN", "SIGUSR2")
        .arg("-c")
        .arg("trap")
        .output()
        .expect("run rubash");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("trap -- '' SIGUSR2\n{}", live_entry_ignore_suffix())
    );
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn bash_command_stays_frozen_inside_err_action() {
    let script = "trap \"echo DEBUG: \\$BASH_COMMAND\" DEBUG\ntrap \"echo ACT; false\" ERR\nfalse\necho done=$?";
    let (stdout, _stderr, rc) = run(script);
    // GNU: the in-action DEBUG firings all report the failed command
    // `false`; the action's own `echo ACT` never becomes BASH_COMMAND.
    assert_eq!(
        stdout,
        "DEBUG: trap \"echo ACT; false\" ERR\nDEBUG: false\nDEBUG: false\nACT\nDEBUG: false\nDEBUG: echo done=$?\ndone=1\n",
        "rc={rc:?}"
    );
    assert_eq!(rc, Some(0));
}

#[test]
fn err_plus_debug_trap_rc_does_not_leak_into_status() {
    let (stdout, _stderr, rc) = run("trap false ERR; trap false DEBUG; echo x; echo survived=$?");
    assert_eq!(stdout, "x\nsurvived=0\n");
    assert_eq!(rc, Some(0));
}

#[test]
fn lone_err_trap_rc_still_does_not_leak() {
    let (stdout, _stderr, rc) = run("trap false ERR; echo x; echo survived=$?");
    assert_eq!(stdout, "x\nsurvived=0\n");
    assert_eq!(rc, Some(0));
}
