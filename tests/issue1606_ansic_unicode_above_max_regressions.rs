//! Issue unixwin/rubash#1606 (residual of #1465): `$'\U00110000'` — the
//! first value above the Unicode maximum — was UTF-8-encoded into the
//! historic 4-byte form F4 90 80 80 instead of staying literal.
//!
//! GNU owners:
//!  - lib/sh/strtrans.c:161-181 ansicstr: `\u`/`\U` decode through
//!    u32cconv; values <= 0x7f translate directly.
//!  - lib/sh/unicode.c:239 u32cconv: conversion goes through wctomb,
//!    which fails for values above U+10FFFF in the C locale (GNU's
//!    default when no locale is selected); the u32tocesc fallback
//!    (unicode.c:141: `\U%08X`, uppercase hex) then keeps the escape
//!    literal. The sign-bit band (>= 0x80000000) produces nothing
//!    (#1465); the band between the Unicode maximum and the sign bit
//!    stays literal.
//!
//! Verified against GNU Bash 5.3.20 (script-file probes, ambient
//! no-locale environment — the state these tests run in, cf. the
//! platform-default note in the #353 regressions).

use std::process::Command;

fn rubash(script: &str) -> (String, String, Option<i32>) {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(script)
        .env_remove("LC_ALL")
        .env_remove("LC_CTYPE")
        .env_remove("LANG")
        .output()
        .expect("run rubash");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code(),
    )
}

#[test]
fn issue1606_first_value_above_unicode_max_stays_literal() {
    // The filed seed: bytes 5c 55 30 30 31 31 30 30 30 30, lenb=10.
    let (stdout, stderr, code) =
        rubash("b=$'\\U00110000'; printf '%s' \"$b\"; echo \" lenb=${#b}\"");
    assert_eq!(stdout, "\\U00110000 lenb=10\n");
    assert_eq!(stderr, "");
    assert_eq!(code, Some(0));
}

#[test]
fn issue1606_whole_above_max_band_stays_literal() {
    // Band ends and one interior value. (U+20000 and below are valid
    // codepoints and must encode — see the boundary control below.)
    let (stdout, _stderr, code) = rubash(
        "b=$'\\U7FFFFFFF'; printf '%s' \"$b\"; echo \" lenb=${#b}\"\n\
         c=$'\\U00120000'; printf '%s' \"$c\"; echo \" lenc=${#c}\"",
    );
    assert_eq!(stdout, "\\U7FFFFFFF lenb=10\n\\U00120000 lenc=10\n");
    assert_eq!(code, Some(0));
}

#[test]
fn issue1606_unicode_max_itself_still_encodes() {
    // Boundary control: U+10FFFF is a valid codepoint and keeps the
    // existing UTF-8 encoding (F4 8F BF BF) under the platform default.
    let (stdout, _stderr, code) = rubash(
        "b=$'\\U0010FFFF'; printf '%s' \"$b\" | od -An -tx1 | tr -d ' \\n'; echo \" lenb=${#b}\"",
    );
    assert_eq!(stdout, "f48fbfbf lenb=1\n");
    assert_eq!(code, Some(0));
}

#[test]
fn issue1606_sign_bit_band_still_dropped() {
    // #1465's settled half must not move: >= 0x80000000 produces nothing.
    let (stdout, _stderr, code) = rubash(
        "x=$'\\U80000000'; echo \"len=${#x} [$x]\"\n\
         y=$'a\\UFFFFFFFFb'; echo \"leny=${#y} [$y]\"",
    );
    assert_eq!(stdout, "len=0 []\nleny=2 [ab]\n");
    assert_eq!(code, Some(0));
}
