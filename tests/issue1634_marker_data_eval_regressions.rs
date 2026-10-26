//! Issue rubash#1634 (eval face): user data that happens to contain the
//! internal marker strings `__RUBASH_CA1__` / `__RUBASH_HD1__` was
//! deleted from eval input — eval_source_for_reparse blanket-replaced
//! both ASCII markers over the whole joined argument text, which is pure
//! runtime data (the only caller is execute_eval; the compound-operand
//! producers for eval are kept in source form by the gates in
//! command_prepare). The blanket replaces are removed; the C0/PUA
//! carrier restores in the same function are untouched.
//!
//! The storage faces of #1634 (a value whose text starts with the CA1
//! marker being re-parsed as a compound assignment by the assignment
//! funnel and the declare/local paths) are NOT fixed here: the marker is
//! an in-band ASCII flag on plain (name, String) values, so runtime data
//! and transport values are indistinguishable at every consumption
//! boundary. They are pinned below as #[ignore]d GNU-expectation tests
//! for the marker-protocol work (entry-encoding the named markers, the
//! audit's RW-10 recommendation).
//!
//! Expectations are the literal GNU bash 5.3.20 outputs.
#![cfg(unix)]

use std::process::Command;

fn run(script: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(script)
        .env("PATH", "/usr/bin:/bin")
        .output()
        .expect("run rubash");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn issue1634_eval_preserves_marker_text_in_arguments() {
    let out = run(r#"
eval 'echo eval1:[A:__RUBASH_CA1__x]'
eval 'echo eval2:[B:__RUBASH_HD1__y]'
eval 'echo eval3:[__RUBASH_CA1__]'
eval 'echo eval4:[__RUBASH_HD1__]'
"#);
    assert_eq!(
        out,
        "eval1:[A:__RUBASH_CA1__x]\n\
         eval2:[B:__RUBASH_HD1__y]\n\
         eval3:[__RUBASH_CA1__]\n\
         eval4:[__RUBASH_HD1__]\n"
    );
}

#[test]
fn issue1634_eval_still_evaluates_compound_assignments() {
    // Control: eval of ordinary source-form compound assignments must
    // keep working after the blanket strip is gone.
    let out = run(r#"
eval 'a=(1 2 3)'; echo "n=${#a[@]} e1=${a[1]}"
x='a=(4 5)'; eval "$x"; echo "m=${#a[@]}"
eval -- 'b=(x y)'; echo "k=${#b[@]}"
"#);
    assert_eq!(out, "n=3 e1=2\nm=2\nk=2\n");
}

#[test]
fn issue1634_trap_text_preserves_marker_strings() {
    // Control: trap actions never went through the eval strip; pin that
    // they stay verbatim.
    let out = run(r#"
trap 'echo trap:[T:__RUBASH_CA1__z]' EXIT
echo body
"#);
    assert_eq!(out, "body\ntrap:[T:__RUBASH_CA1__z]\n");
}

#[test]
#[ignore = "diagnosed in fix7/carrier REPORT: funnel re-sniffs the in-band CA1 marker; needs the marker-protocol change (RW-10)"]
fn issue1634_quoted_assignment_keeps_scalar_type() {
    let out = run(r#"
w='__RUBASH_CA1__(a b)'; echo "n=${#w[@]} s=[$w]"
w="__RUBASH_CA1__(a b)"; echo "n=${#w[@]} s=[$w]"
"#);
    assert_eq!(
        out,
        "n=1 s=[__RUBASH_CA1__(a b)]\nn=1 s=[__RUBASH_CA1__(a b)]\n"
    );
}

#[test]
#[ignore = "diagnosed in fix7/carrier REPORT: funnel re-sniffs the in-band CA1 marker; needs the marker-protocol change (RW-10)"]
fn issue1634_runtime_channels_keep_marker_data() {
    let out = run(r#"
w=$(printf '__RUBASH_CA1__(r s)'); echo "comsub n=${#w[@]} s=[$w]"
read -r w <<< '__RUBASH_CA1__(t u)'; echo "read n=${#w[@]} s=[$w]"
v='__RUBASH_CA1__'; w=$v'(a b)'; echo "dollarv n=${#w[@]} s=[$w]"
"#);
    assert_eq!(
        out,
        "comsub n=1 s=[__RUBASH_CA1__(r s)]\n\
         read n=1 s=[__RUBASH_CA1__(t u)]\n\
         dollarv n=1 s=[__RUBASH_CA1__(a b)]\n"
    );
}
