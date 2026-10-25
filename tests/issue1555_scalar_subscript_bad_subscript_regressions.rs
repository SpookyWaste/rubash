//! Issue rubash#1555: a scalar PIPESTATUS cell read with a negative
//! subscript returned the scalar value through a dense fallback
//! instead of reporting `bad array subscript`.
//!
//! GNU model (arrayfunc.c:1483 array_value_internal, reached from
//! subst.c parameter_brace_expand, subst.c:7488
//! array_length_reference for the `${#...}` form, and expr.c via
//! get_array_value for arithmetic): the negative-subscript rebase
//! `ind = array_max_index + 1 + ind` is gated on `array_p(var)`.
//! A scalar cell (VT_VARIABLE) never rebases, so ANY negative
//! subscript is INDEX_ERROR (arrayfunc.c:1462, err_badarraysub)
//! and the read is empty; element 0 is the whole string and a
//! positive subscript reads as unset, silently. The rule is
//! general — PIPESTATUS is only observable in scalar shape inside
//! a tempenv window or as a function-local shadow (#1525's cells),
//! because a standalone `PIPESTATUS=99` command's own completion
//! write reinstalls the live array before the next command reads.
//!
//! Expected outputs verified against GNU bash 5.3.20 (script files).

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

fn diag_count(stderr: &str, name: &str) -> usize {
    stderr
        .matches(&format!("{name}: bad array subscript"))
        .count()
}

#[test]
fn issue1555_pipestatus_window_scalar_negative_subscript() {
    // The scalar shadow inside a tempenv window: [-1] is
    // INDEX_ERROR + empty; [0]/[@] are the value; [1] is silently
    // unset.
    let (stdout, stderr, code) = rubash(
        "true | false\nPIPESTATUS=99 eval 'echo \"w-1=[${PIPESTATUS[-1]}] w0=[${PIPESTATUS[0]}] w@=[${PIPESTATUS[@]}] widx=[${!PIPESTATUS[@]}] w1=[${PIPESTATUS[1]}]\"'",
    );
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "w-1=[] w0=[99] w@=[99] widx=[0] w1=[]\n");
    assert_eq!(diag_count(&stderr, "PIPESTATUS"), 1, "stderr: {stderr}");
}

#[test]
fn issue1555_pipestatus_local_scalar_negative_subscript() {
    let (stdout, stderr, code) = rubash(
        "f() { local PIPESTATUS=99; echo \"f-1=[${PIPESTATUS[-1]}] f0=[${PIPESTATUS[0]}]\"; }\nf",
    );
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "f-1=[] f0=[99]\n");
    assert_eq!(diag_count(&stderr, "PIPESTATUS"), 1, "stderr: {stderr}");
}

#[test]
fn issue1555_pipestatus_live_array_negative_still_rebases() {
    // The live array keeps the array rule: -1 resolves against the
    // maximum index, no diagnostic. (The `${!PIPESTATUS[@]}` index
    // list is pinned by the issue1364 suite; as a FIRST touch of
    // PIPESTATUS it hits a separate pre-existing master divergence
    // — `PIPESTATUS[@]: invalid indirect expansion` — recorded in
    // the cluster report, not part of this issue.)
    let (stdout, stderr, code) =
        rubash("true | false | true\necho \"D-1=[${PIPESTATUS[-1]}] D2=[${PIPESTATUS[-2]}]\"");
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "D-1=[0] D2=[1]\n");
    assert_eq!(diag_count(&stderr, "PIPESTATUS"), 0, "stderr: {stderr}");
}

#[test]
fn issue1555_plain_scalar_negative_subscript() {
    let (stdout, stderr, code) = rubash(
        "v=99\necho \"v0=[${v[0]}] v-1=[${v[-1]}] v@=[${v[@]}] vidx=[${!v[@]}] v1=[${v[1]}]\"",
    );
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "v0=[99] v-1=[] v@=[99] vidx=[0] v1=[]\n");
    assert_eq!(diag_count(&stderr, "v"), 1, "stderr: {stderr}");
}

#[test]
fn issue1555_unset_and_valueless_negative_subscript() {
    // array_value_internal adjudicates the subscript before the
    // var==0 / value_cell==0 early-outs: an unset or valueless
    // scalar name errors the same way.
    let (stdout, stderr, code) = rubash("unset u\necho \"u-1=[${u[-1]}] u0=[${u[0]}]\"");
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "u-1=[] u0=[]\n");
    assert_eq!(diag_count(&stderr, "u"), 1, "stderr: {stderr}");

    let (stdout, stderr, code) = rubash("declare w\necho \"w-1=[${w[-1]}]\"");
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "w-1=[]\n");
    assert_eq!(diag_count(&stderr, "w"), 1, "stderr: {stderr}");
}

#[test]
fn issue1555_array_out_of_range_negative_subscript() {
    let (stdout, stderr, code) = rubash("a=(1 2 3)\necho \"a-1=[${a[-1]}] a-4=[${a[-4]}]\"");
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "a-1=[3] a-4=[]\n");
    assert_eq!(diag_count(&stderr, "a"), 1, "stderr: {stderr}");
}

#[test]
fn issue1555_operator_forms_report_and_see_unset() {
    // The fetch inside parameter_brace_expand reports for every
    // operator arm, and the NULL value makes the parameter unset
    // for both `-` and `:-` (and unset for the `+` arms).
    let (stdout, stderr, code) =
        rubash("v=99\necho \"op1=[${v[-1]:-DEF}] op2=[${v[1]:-DEF}] op3=[${v[-1]:+ALT}]\"");
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "op1=[DEF] op2=[DEF] op3=[]\n");
    assert_eq!(diag_count(&stderr, "v"), 2, "stderr: {stderr}");

    let (stdout, stderr, code) = rubash(
        "true | false\nPIPESTATUS=99 eval 'echo \"m1=[${PIPESTATUS[-1]-DEF}] m2=[${PIPESTATUS[-1]:-DEF}] m3=[${PIPESTATUS[-1]+ALT}] m4=[${PIPESTATUS[-1]:+ALT}] m5=[${PIPESTATUS[0]-DEF}]\"'",
    );
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "m1=[DEF] m2=[DEF] m3=[] m4=[] m5=[99]\n");
    assert_eq!(diag_count(&stderr, "PIPESTATUS"), 4, "stderr: {stderr}");
}

#[test]
fn issue1555_nonliteral_and_pattern_and_substring_forms_report() {
    let (stdout, stderr, code) = rubash("v=99; i=-1\necho \"nl=[${v[i]:-DEF}] nl2=[${v[$i]}]\"");
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "nl=[DEF] nl2=[]\n");
    assert_eq!(diag_count(&stderr, "v"), 2, "stderr: {stderr}");

    let (stdout, stderr, code) = rubash("v=99\necho \"pat=[${v[-1]#9}] sub=[${v[-1]:0:1}]\"");
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "pat=[] sub=[]\n");
    assert_eq!(diag_count(&stderr, "v"), 2, "stderr: {stderr}");
}

#[test]
fn issue1555_length_forms() {
    // subst.c:7488 array_length_reference: scalar [0] is the value
    // length, [1] is 0; an unresolvable negative reports
    // `[-1]: bad array subscript` and the expansion fails
    // (expand_wdesc_error): the command is abandoned, status 1.
    let (stdout, stderr, code) = rubash("v=99\necho \"len0=[${#v[0]}] len1=[${#v[1]}]\"");
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "len0=[2] len1=[0]\n");
    assert_eq!(diag_count(&stderr, "v"), 0, "stderr: {stderr}");

    let (stdout, stderr, code) = rubash("v=99\necho \"len=[${#v[-1]}]\"\necho \"after rc=$?\"");
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "after rc=1\n");
    assert!(
        stderr.contains("[-1]: bad array subscript"),
        "stderr: {stderr}"
    );

    // Unset and declared-unset bases return 0 silently (the
    // var == 0 / invisible early-outs precede adjudication).
    let (stdout, stderr, code) = rubash("unset u\necho \"u-1=[${#u[-1]}]\"");
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "u-1=[0]\n");
    assert!(!stderr.contains("bad array subscript"), "stderr: {stderr}");

    let (stdout, stderr, code) = rubash("declare w\necho \"w-1=[${#w[-1]}]\"");
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "w-1=[0]\n");
    assert!(!stderr.contains("bad array subscript"), "stderr: {stderr}");
}

#[test]
fn issue1555_length_pipestatus_live_and_window() {
    // Live PIPESTATUS element lengths come from the cell, not the
    // (empty) env mirror.
    let (stdout, stderr, code) = rubash(
        "true | false | true\necho \"p0=[${#PIPESTATUS[0]}] p-1=[${#PIPESTATUS[-1]}] p5=[${#PIPESTATUS[5]}]\"",
    );
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "p0=[1] p-1=[1] p5=[0]\n");
    assert!(!stderr.contains("bad array subscript"), "stderr: {stderr}");

    // Scalar-shape window: the length form takes the same
    // bad-subscript failure as a plain scalar.
    let (stdout, stderr, code) = rubash(
        "true | false\nPIPESTATUS=99 eval 'echo \"plen=[${#PIPESTATUS[-1]}]\"'\necho \"rc=$?\"",
    );
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "rc=1\n");
    assert!(
        stderr.contains("[-1]: bad array subscript"),
        "stderr: {stderr}"
    );
}

#[test]
fn issue1555_arithmetic_forms() {
    // expr.c reaches the same array_value_internal through
    // get_array_value: the diagnostic fires and the read is 0.
    let (stdout, stderr, code) = rubash("v=99\necho \"arith=$((v[-1]))\"");
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "arith=0\n");
    assert_eq!(diag_count(&stderr, "v"), 1, "stderr: {stderr}");

    let (stdout, stderr, code) = rubash("a=(1 2 3)\necho \"x=$((a[-4])) y=$((a[-1]))\"");
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "x=0 y=3\n");
    assert_eq!(diag_count(&stderr, "a"), 1, "stderr: {stderr}");

    let (stdout, stderr, code) = rubash(
        "true | false\nPIPESTATUS=99 eval 'echo \"aw=$((PIPESTATUS[-1])) aw0=$((PIPESTATUS[0]))\"'",
    );
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "aw=0 aw0=99\n");
    assert_eq!(diag_count(&stderr, "PIPESTATUS"), 1, "stderr: {stderr}");
}

#[test]
fn issue1555_repeated_reference_reports_per_expansion() {
    // GNU reports once per ${} fetch — two identical references in
    // one word report twice.
    let (stdout, stderr, code) = rubash("v=99\necho \"d=[${v[-1]}${v[-1]}]\"");
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "d=[]\n");
    assert_eq!(diag_count(&stderr, "v"), 2, "stderr: {stderr}");
}
