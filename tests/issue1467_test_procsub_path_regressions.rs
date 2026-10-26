//! rubash#1467 — the builtin `test`/`[ ]` file tests (-r/-e/-p) all
//! failed on process-substitution paths (/dev/fd/N), although reading
//! the same fd through a redirection worked. GNU: the tests succeed.
//!
//! Probe: clusters/x2/repro/1467.sh.
//! Expectations verified against GNU bash 5.3.20 (script files).

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
fn test_builtin_accepts_procsub_paths() {
    let (stdout, stderr, code) = rubash(
        "test -r <(printf 'x\\n') && echo OK-r || echo FAIL-r\n\
         test -e <(printf 'x\\n') && echo OK-e || echo FAIL-e\n\
         test -p <(printf 'x\\n') && echo OK-p || echo FAIL-p\n\
         [ -r <(printf 'x\\n') ] && echo OK-br || echo FAIL-br\n\
         head -c 1 <(printf 'xy'); echo \" <- control-head\"",
    );
    assert_eq!(stdout, "OK-r\nOK-e\nOK-p\nOK-br\nx <- control-head\n");
    assert_eq!(stderr, "");
    assert_eq!(code, Some(0));
}
