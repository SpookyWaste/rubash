//! rubash#1656 — case silently accepts an invalid `\;&` item terminator.
//!
//! Follow-up to #1641 (extra `)` in the pattern runs the
//! branch). The mirror shape: an invalid terminator makes
//! rubash parse the clause body as ordinary commands —
//! `case a in a) echo A;\;& b) echo B;; esac` ran A and
//! then `b` as a stray command (rc=127) — while GNU
//! rejects the whole case at parse time with `syntax
//! error near unexpected token `)'`, rc=2, running nothing
//! on that line.
//!
//! Root cause: case_body_end's boundary stack tracks
//! if/for/case/{ frames but not subshell parens, so a bare
//! `)` at the clause's top level was scanned past and the
//! body parser silently dropped everything after it. The
//! fix tracks paren depth alongside the stack (a `(` at
//! stack-empty opens a subshell/function/arithmetic group,
//! all balanced; a `)` with nothing to close stops the
//! scan) and reports the stray `)` with the same
//! parse-error node shape as the pattern-position errors
//! (executor: syntax error near `)`, rc=2). Parens inside
//! a nested frame — nested case patterns included, where
//! the stack is non-empty — are left to the stack
//! machinery, so balanced constructs in the body
//! (subshells, nested cases, functions, arithmetic,
//! extglob, command/process substitution, brace groups,
//! if/for/while bodies) are unaffected.
//!
//! Expectations verified against GNU bash 5.3.20. Stdin is
//! nulled so a regression blocks on nothing.

use std::process::{Command, Stdio};

fn rubash_c(script: &str) -> (String, String, Option<i32>) {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(script)
        .stdin(Stdio::null())
        .output()
        .expect("run rubash");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code(),
    )
}

#[test]
fn stray_close_after_body_is_syntax_error() {
    // Issue seed (non-matching word): GNU errors, runs nothing.
    let (stdout, stderr, code) = rubash_c(
        "case x in a) echo A;\\;& b) echo B;; esac; echo \"rc=$?\"",
    );
    assert_eq!(stdout, "");
    assert!(stderr.contains("syntax error near unexpected token `)'"));
    assert_eq!(code, Some(2));
    // Matching word: the branch must NOT run either.
    let (stdout, stderr, code) = rubash_c(
        "case a in a) echo A;\\;& b) echo B;; esac; echo \"rc=$?\"",
    );
    assert_eq!(stdout, "");
    assert!(stderr.contains("syntax error near unexpected token `)'"));
    assert_eq!(code, Some(2));
}

#[test]
fn fuzz_original_double_escaped_terminator() {
    let (stdout, stderr, code) = rubash_c(
        "case x in a) echo A;\\;\\& b) echo B;; esac; echo \"rc=$?\"",
    );
    assert_eq!(stdout, "");
    assert!(stderr.contains("syntax error near unexpected token `)'"));
    assert_eq!(code, Some(2));
}

#[test]
fn stray_close_variants_are_syntax_errors() {
    for script in [
        "case a in a) echo hi) ;; esac",
        "case a in a) ) ;; esac",
        "case a in a) echo A;; b) echo B; ) ;; esac",
    ] {
        let (stdout, stderr, code) = rubash_c(script);
        assert_eq!(stdout, "", "script: {script}");
        assert!(
            stderr.contains("syntax error near unexpected token `)'"),
            "script: {script}, stderr: {stderr}"
        );
        assert_eq!(code, Some(2), "script: {script}");
    }
}

#[test]
fn balanced_constructs_in_body_still_parse() {
    for (script, expected) in [
        ("case a in a) (echo hi) ;; esac", "hi\n"),
        ("case a in a) case b in b|c) echo Y;; esac;; esac", "Y\n"),
        ("case a in a) f() { echo F; }; f;; esac", "F\n"),
        ("case a in a) (( x = 1 + 2 )); echo $x;; esac", "3\n"),
        ("case a in a) echo $(echo Z);; esac", "Z\n"),
        ("case a in a) cat <(echo P);; esac", "P\n"),
        ("case a in a) { echo G; };; esac", "G\n"),
        ("case a in a) if true; then echo I; fi;; esac", "I\n"),
        ("case a in a) for i in 1 2; do echo $i; done;; esac", "1\n2\n"),
    ] {
        let (stdout, _, code) = rubash_c(script);
        assert_eq!(stdout, expected, "script: {script}");
        assert_eq!(code, Some(0), "script: {script}");
    }
}

#[test]
fn valid_terminators_unchanged() {
    for (script, expected) in [
        ("case a in a) echo A;; b) echo B;; esac", "A\n"),
        ("case a in a) echo A;& b) echo B;; esac", "A\nB\n"),
        ("case a in a) echo A;;& a) echo A2;; esac", "A\nA2\n"),
        ("case b in a|b) echo AB;; *) echo STAR;; esac", "AB\n"),
    ] {
        let (stdout, _, code) = rubash_c(script);
        assert_eq!(stdout, expected, "script: {script}");
        assert_eq!(code, Some(0), "script: {script}");
    }
}
