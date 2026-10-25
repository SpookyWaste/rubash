//! Issue rubash#1650: `shift` with surplus operands reported
//! "too many arguments" (status 2) but did NOT abandon the rest of the
//! current command list.
//!
//! GNU builtins/shift.def reads its operand through get_numeric_arg
//! (builtins/common.c:511), which ends the walk with no_args() — a
//! surplus-operand failure that longjmps DISCARD (the rubash#705 class,
//! sibling of rubash#1240/#1519): the rest of the failing line is
//! skipped, a function call unwinds whole with status 2, and in `-c`
//! mode the whole string is abandoned and the shell exits 1. The
//! sibling failures are NOT in this class: a non-numeric operand
//! (EX_USAGE, plain return) and an out-of-range count
//! (EXECUTION_FAILURE) let the line continue.
//!
//! Expectations verified against GNU bash 5.3.20 (self-compiled
//! oracle), script-file mode authoritative.

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

fn rubash_file(script: &str) -> (String, String, Option<i32>) {
    let dir = std::env::temp_dir().join(format!(
        "rubash-issue1650-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    std::fs::write(dir.join("case.sh"), script).expect("write case.sh");
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("case.sh")
        .current_dir(&dir)
        .output()
        .expect("run rubash file");
    let _ = std::fs::remove_dir_all(&dir);
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code(),
    )
}

#[test]
fn issue1650_shift_too_many_args_discards_rest_of_line() {
    let (stdout, stderr, code) = rubash_file(
        "set -- a b c\nshift 1 2; echo \"SHOULD_NOT_PRINT\"\necho \"after rc=$? n=$#\"\n",
    );
    assert_eq!(stdout, "after rc=2 n=3\n");
    assert!(stderr.contains("shift: too many arguments"), "{stderr}");
    assert_eq!(code, Some(0));
}

#[test]
fn issue1650_shift_too_many_args_unwinds_function() {
    let (stdout, _, code) =
        rubash_file("f() { shift 1 2; echo \"INNER_NOT_PRINT\"; }\nf\necho \"frc=$?\"\n");
    assert_eq!(stdout, "frc=2\n");
    assert_eq!(code, Some(0));
}

#[test]
fn issue1650_shift_too_many_args_abandons_dash_c_string() {
    let (stdout, _, code) = rubash("shift 1 2; echo NOPE");
    assert_eq!(stdout, "");
    assert_eq!(code, Some(1));
}

#[test]
fn issue1650_shift_non_numeric_operand_does_not_discard() {
    let (stdout, _, code) = rubash_file("set -- a b c\nshift x; echo \"rc=$?\"\n");
    assert_eq!(stdout, "rc=2\n");
    assert_eq!(code, Some(0));
}

#[test]
fn issue1650_shift_out_of_range_does_not_discard() {
    let (stdout, _, code) = rubash_file("set -- a b\nshift 99; echo \"rc=$?\"\n");
    assert_eq!(stdout, "rc=1\n");
    assert_eq!(code, Some(0));
}
