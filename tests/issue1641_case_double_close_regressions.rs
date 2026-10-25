//! rubash#1641 — a case pattern closed twice, `(x))`, was silently
//! accepted and the branch ran; GNU reports `syntax error near
//! unexpected token `)'` with status 2.
//!
//! GNU ownership: parse.y's pattern_list ends at the first `)` and
//! the grammar admits no second one. The fix checks, right after
//! the pattern's closing `)` is consumed in case_command.rs,
//! whether the next token is another `)`, and returns a parse-error
//! node for it (same shape as the `(`-in-pattern error, but without
//! that error's extglob-gate reparse — a `)` error is settled at
//! parse time). Parsing stops there and the executor reports the
//! error and aborts with status 2, so earlier script lines run and
//! later ones do not — GNU's script outcome.
//!
//! Expectations verified against GNU bash (5.2 locally, 5.3.20 in
//! the issue — the diagnostic text and status agree). Legal forms
//! ((x), (x|y), bare x), *) and extglob @(x)) must keep working.

use std::process::Command;

fn rubash_file(script: &str) -> (String, String, Option<i32>) {
    let tag = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    );
    let path = std::env::temp_dir().join(format!("rubash-1641-{tag}.sh"));
    std::fs::write(&path, script).expect("write script");
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg(&path)
        .output()
        .expect("run rubash");
    let _ = std::fs::remove_file(&path);
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code(),
    )
}

#[test]
fn double_closed_pattern_is_syntax_error() {
    let (stdout, stderr, code) =
        rubash_file("case x in (x)) echo P ;; *) echo NO ;; esac\n");
    assert_eq!(stdout, "");
    assert!(
        stderr.contains("syntax error near unexpected token `)'"),
        "stderr: {stderr}"
    );
    assert_eq!(code, Some(2));
}

#[test]
fn double_closed_alternation_is_syntax_error() {
    let (stdout, stderr, code) =
        rubash_file("case x in (x|y)) echo P ;; *) echo NO ;; esac\n");
    assert_eq!(stdout, "");
    assert!(
        stderr.contains("syntax error near unexpected token `)'"),
        "stderr: {stderr}"
    );
    assert_eq!(code, Some(2));
}

#[test]
fn syntax_error_aborts_rest_of_script_only() {
    let (stdout, _stderr, code) = rubash_file(
        "echo before\ncase x in (x)) echo P ;; esac\necho after\n",
    );
    assert_eq!(stdout, "before\n");
    assert_eq!(code, Some(2));
}

#[test]
fn legal_pattern_forms_still_run() {
    let (stdout, _stderr, code) = rubash_file(
        "case x in (x) echo P ;; *) echo NO ;; esac\ncase x in (x|y) echo P ;; *) echo NO ;; esac\ncase x in x) echo Q ;; *) echo NO ;; esac\ncase z in *) echo STAR ;; esac\n",
    );
    assert_eq!(stdout, "P\nP\nQ\nSTAR\n");
    assert_eq!(code, Some(0));
}

#[test]
fn extglob_pattern_still_runs_and_double_close_still_errors() {
    let (stdout, stderr, code) = rubash_file(
        "shopt -s extglob\ncase x in @(x)) echo EXT ;; *) echo NO ;; esac\ncase x in (x)) echo BAD ;; *) echo NO ;; esac\n",
    );
    assert_eq!(stdout, "EXT\n");
    assert!(
        stderr.contains("syntax error near unexpected token `)'"),
        "stderr: {stderr}"
    );
    assert_eq!(code, Some(2));
}
