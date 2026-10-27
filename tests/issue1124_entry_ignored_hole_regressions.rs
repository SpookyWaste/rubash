//! rubash#1124: `trap -p` lists signals ignored on shell entry,
//! including hole numbers with no signal name.
//!
//! GNU trap.c:236 decode_signal accepts any number < NSIG, and
//! builtins/trap.def:308-311 showtrap prints holes as the bare number
//! (support/signames.c:458-464 names them "SIGJUNK(n)"): a signal
//! ignored on entry (Termux SigIgn 0x2000000000 -> signal 38, a bionic
//! hole) lists as `trap -- '' 38` (verified against bash 5.3.20), and
//! set/reset on it are silent no-ops (trap.c:850 SIG_HARD_IGNORE).
//! The trap table keys the hole by its number — "" would collide every
//! hole on one key.

use std::process::Command;

fn run_with_orig_ign(orig_ign: &str, script: &str) -> (String, String, Option<i32>) {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .env("__RUBASH_TRAP_ORIG_IGN", orig_ign)
        .arg("-c")
        .arg(script)
        .output()
        .expect("spawn rubash");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code(),
    )
}

/// A transported numeric entry-ignore lists by number (portable: no
/// kernel entry mask needed, exercises the same table path as the
/// launcher-inherited Termux 38).
#[test]
fn transported_numeric_ignore_lists_by_number() {
    let (stdout, stderr, code) = run_with_orig_ign("38", "trap -p 38");
    assert_eq!(stderr, "", "stderr: {stderr:?}");
    assert_eq!(stdout, "trap -- '' 38\n");
    assert_eq!(code, Some(0));
}

/// The numeric entry-ignore appears in the full `trap -p` listing too.
#[test]
fn transported_numeric_ignore_lists_in_full_table() {
    let (stdout, stderr, code) = run_with_orig_ign("38", "trap -p");
    assert_eq!(stderr, "", "stderr: {stderr:?}");
    assert_eq!(code, Some(0));
    assert!(
        stdout.contains("trap -- '' 38\n"),
        "full table must carry the numeric entry-ignore:\n{stdout}"
    );
}

/// Set/reset on a hard-ignored hole are silent no-ops (trap.c:850):
/// the listing is unchanged and the status stays 0.
#[test]
fn numeric_ignore_refuses_set_and_reset_silently() {
    let (stdout, stderr, code) = run_with_orig_ign(
        "38",
        "trap 'echo X' 38; echo \"set rc=$?\"; trap -p 38; \
         trap -- '' 38; echo \"ign rc=$?\"; trap -p 38; \
         trap - 38; echo \"reset rc=$?\"; trap -p 38",
    );
    assert_eq!(stderr, "", "stderr: {stderr:?}");
    assert_eq!(code, Some(0));
    assert_eq!(
        stdout,
        "set rc=0\ntrap -- '' 38\nign rc=0\ntrap -- '' 38\nreset rc=0\ntrap -- '' 38\n"
    );
}

/// A non-ignored hole still lists nothing (GNU `trap -p 32` prints
/// nothing, rc 0): the number key alone is not a trap.
#[test]
fn non_ignored_hole_lists_nothing() {
    let (stdout, stderr, code) = run_with_orig_ign("", "trap -p 32; echo \"rc=$?\"");
    assert_eq!(stderr, "", "stderr: {stderr:?}");
    assert_eq!(stdout, "rc=0\n");
    assert_eq!(code, Some(0));
}

/// Live environment: when the kernel really ignores the hole at entry
/// (this Termux launcher's SigIgn 38), `trap -p 38` lists it with no
/// transport env involved. Skipped vacuously on hosts with a clean
/// entry mask (clean CI runners).
#[test]
#[cfg(unix)]
fn live_entry_hole_lists_when_kernel_ignores_it() {
    let ignored_at_entry = {
        let mut current: libc::sigaction = unsafe { std::mem::zeroed() };
        if unsafe { libc::sigaction(38, std::ptr::null(), &mut current) } != 0 {
            false
        } else {
            current.sa_sigaction == libc::SIG_IGN
        }
    };
    if !ignored_at_entry {
        return;
    }
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .env_remove("__RUBASH_TRAP_ORIG_IGN")
        .arg("-c")
        .arg("trap -p 38")
        .output()
        .expect("spawn rubash");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(stderr, "", "stderr: {stderr:?}");
    assert_eq!(stdout, "trap -- '' 38\n");
    assert_eq!(output.status.code(), Some(0));
}
