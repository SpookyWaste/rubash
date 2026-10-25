//! Issue rubash#1649: the bash 5.3 dynamic variable BASH_MONOSECONDS
//! was missing (expanded to nothing and reported unset).
//!
//! GNU variables.c:1365 get_monoseconds, registered at variables.c:1855
//! via INIT_DYNAMIC_VAR with a NULL assign function: the value is
//! clock_gettime(CLOCK_MONOTONIC) whole seconds, recomputed on every
//! read (so an assignment never sticks), and `unset` unbinds it for the
//! rest of the session like any dynamic variable.
//!
//! Expectations verified against GNU bash 5.3.20 (self-compiled oracle).

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

#[test]
fn issue1649_monoseconds_is_set_and_numeric() {
    let (stdout, _, _) = rubash(
        "echo \"set=${BASH_MONOSECONDS+x}\"; \
         case $BASH_MONOSECONDS in (*[!0-9]*|'') echo BAD ;; (*) echo NUM ;; esac; \
         [[ -v BASH_MONOSECONDS ]] && echo viset",
    );
    assert_eq!(stdout, "set=x\nNUM\nviset\n");
}

#[test]
fn issue1649_monoseconds_ticks_with_monotonic_clock() {
    let (stdout, _, _) = rubash(
        "a=$BASH_MONOSECONDS; sleep 1; b=$BASH_MONOSECONDS; \
         if [ \"$b\" -gt \"$a\" ]; then echo TICK_OK; else echo \"BAD a=$a b=$b\"; fi",
    );
    assert_eq!(stdout, "TICK_OK\n");
}

#[test]
fn issue1649_monoseconds_assignment_is_ignored() {
    // NULL assign function: the next read recomputes from the clock.
    let (stdout, _, _) = rubash(
        "BASH_MONOSECONDS=999999999; \
         if [ \"$BASH_MONOSECONDS\" = 999999999 ]; then echo BAD; else echo OK; fi",
    );
    assert_eq!(stdout, "OK\n");
}

#[test]
fn issue1649_monoseconds_declare_and_arithmetic_views() {
    let (stdout, _, _) = rubash(
        "declare -p BASH_MONOSECONDS; \
         echo \"arith_ok=$(( BASH_MONOSECONDS >= 0 ))\"",
    );
    let mut lines = stdout.lines();
    let declare = lines.next().unwrap_or("");
    assert!(
        declare.starts_with("declare -- BASH_MONOSECONDS=\""),
        "{declare}"
    );
    let value = declare
        .trim_start_matches("declare -- BASH_MONOSECONDS=\"")
        .trim_end_matches('"');
    assert!(
        !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()),
        "{declare}"
    );
    assert_eq!(lines.next(), Some("arith_ok=1"));
}

#[test]
fn issue1649_monoseconds_unset_unbinds_for_the_session() {
    let (stdout, _, _) =
        rubash("unset BASH_MONOSECONDS; echo \"after_unset=[${BASH_MONOSECONDS+x}]\"");
    assert_eq!(stdout, "after_unset=[]\n");
}
