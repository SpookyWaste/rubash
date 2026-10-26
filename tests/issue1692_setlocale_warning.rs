//! Regression pins for rubash#1692 — assigning an invalid locale name
//! produced no diagnostic, where GNU bash warns:
//!   bash: warning: setlocale: LC_ALL: cannot change locale (bogus_LOCALE)
//!
//! GNU ownership: variables.c's special_vars table (variables.c:5800)
//! binds LANG and every LC_* category to sv_locale (variables.c:6272),
//! which calls locale.c:185 set_locale_var on ASSIGNMENT. On a failed
//! setlocale it warns `setlocale: <VAR>: cannot change locale (<value>)`
//! (locale.c:225 / locale.c:302), appending `: <strerror(errno)>` only
//! when errno is set — glibc leaves errno set only on the FIRST failed
//! lookup of a given locale name in the process, so a repeat assignment
//! of the same bad name warns without the suffix. LANG itself never
//! warns (set_lang is silent). A category assignment is not even
//! attempted while LC_ALL is non-empty, so it warns about nothing then.
//! At startup only LC_ALL warns (shell.c -> locale.c set_default_lang);
//! invalid LANG or LC_CTYPE in the startup environment are silent.
//! Temporary-environment assignments for EXTERNAL commands never reach
//! the bind step (assign_in_env), so they do not warn; for builtins
//! they bind and do warn.
//!
//! All expectations are the literal GNU bash 5.3.20 outputs for the same
//! script files / environments on a system where the bogus names do not
//! resolve (only C/C.utf8/POSIX installed). The bogus names are chosen
//! so no real locale can ever match them.
#![cfg(unix)]

use std::path::PathBuf;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_rubash");

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rubash-1692-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// Base command with the whole locale family removed from the
/// inherited environment so each case starts from a known state.
fn base() -> Command {
    let mut cmd = Command::new(BIN);
    for var in [
        "LANG",
        "LC_ALL",
        "LC_COLLATE",
        "LC_CTYPE",
        "LC_MESSAGES",
        "LC_NUMERIC",
        "LC_TIME",
    ] {
        cmd.env_remove(var);
    }
    cmd.stdin(std::process::Stdio::null());
    cmd
}

fn run_script(tag: &str, body: &str) -> (String, String, String, Option<i32>) {
    let dir = scratch(tag);
    let script = dir.join("probe.sh");
    std::fs::write(&script, body).expect("write script");
    let output = base()
        .arg(&script)
        .current_dir(&dir)
        .output()
        .expect("run rubash");
    let result = (
        script.to_string_lossy().into_owned(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code(),
    );
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[test]
fn issue1692_assignment_form_warns_like_gnu() {
    // Line 1 (export) is the first failing attempt of this name in the
    // process: GNU appends the strerror suffix. Line 2 repeats the same
    // name as a plain assignment: the warning fires again, no suffix.
    let (path, stdout, stderr, code) = run_script(
        "assign",
        "export LC_ALL=bogus_LOCALE\nLC_ALL=bogus_LOCALE\necho after\n",
    );
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "after\n", "stdout is unaffected by the warning");
    assert_eq!(
        stderr,
        format!(
            "{path}: line 1: warning: setlocale: LC_ALL: cannot change locale (bogus_LOCALE): No such file or directory\n\
             {path}: line 2: warning: setlocale: LC_ALL: cannot change locale (bogus_LOCALE)\n"
        )
    );
}

#[test]
fn issue1692_category_assignment_warns_with_its_own_name() {
    let (path, _stdout, stderr, code) =
        run_script("category", "LC_CTYPE=bogus_CTYPE_LOCALE\necho after\n");
    assert_eq!(code, Some(0));
    assert_eq!(
        stderr,
        format!(
            "{path}: line 1: warning: setlocale: LC_CTYPE: cannot change locale (bogus_CTYPE_LOCALE): No such file or directory\n"
        )
    );
}

#[test]
fn issue1692_silent_cases_stay_silent() {
    // LANG never warns; a category assignment under a non-empty LC_ALL
    // is not attempted; clearing LC_ALL is silent.
    let (_path, stdout, stderr, code) = run_script(
        "silent",
        "LANG=bogus_LANG_LOCALE\nLC_ALL=C\nLC_CTYPE=bogus_CTYPE_LOCALE\nLC_ALL=\necho after\n",
    );
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "after\n");
    assert_eq!(stderr, "", "no setlocale warning in the silent cases");
}

#[test]
fn issue1692_startup_environment_warns_for_lc_all_only() {
    let output = base()
        .env("LC_ALL", "bogus_LOCALE")
        .args(["-c", "echo hi"])
        .output()
        .expect("run rubash");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hi\n");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        format!("{BIN}: warning: setlocale: LC_ALL: cannot change locale (bogus_LOCALE)\n"),
        "startup warning: argv[0] prefix, no line number, no strerror suffix"
    );

    for (var, value) in [
        ("LANG", "bogus_LANG_LOCALE"),
        ("LC_CTYPE", "bogus_CTYPE_LOCALE"),
    ] {
        let output = base()
            .env(var, value)
            .args(["-c", "echo hi"])
            .output()
            .expect("run rubash");
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            "",
            "startup {var} must not warn (only LC_ALL does)"
        );
    }
}

#[test]
fn issue1692_temporary_env_warns_only_when_it_binds() {
    // External command: the temp assignment never binds in the shell —
    // silent. Builtin: it binds — warns (first attempt: with suffix).
    let (path, stdout, stderr, code) = run_script(
        "tempenv",
        "LC_ALL=bogus_LOCALE /usr/bin/true\necho \"ext:$?\"\nLC_ALL=bogus_LOCALE echo hi\n",
    );
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "ext:0\nhi\n");
    assert_eq!(
        stderr,
        format!(
            "{path}: line 3: warning: setlocale: LC_ALL: cannot change locale (bogus_LOCALE): No such file or directory\n"
        )
    );
}
