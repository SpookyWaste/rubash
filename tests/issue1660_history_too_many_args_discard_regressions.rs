//! Issue rubash#1660: `history` with surplus operands reported
//! "too many arguments" (status 2) but did NOT abandon the rest of the
//! current command list. Same root class as rubash#1650 (GNU
//! builtins/common.c no_args -> DISCARD, the rubash#705 machinery):
//! display_history validates its operand via get_numeric_arg and
//! no_args rejects any further operands. The sibling failures are NOT
//! in this class: a non-numeric operand (sh_neednumarg, EX_USAGE) and
//! an invalid option (EX_USAGE) are plain returns.
//!
//! Both live paths are pinned: the session path
//! (executor/history_exec.rs — a plain script gets an on-demand
//! session) is what these tests drive.
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
        "rubash-issue1660-{}-{}",
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
fn issue1660_history_too_many_args_discards_rest_of_line() {
    let (stdout, stderr, code) =
        rubash_file("history 1 2; echo \"SHOULD_NOT_PRINT\"\necho \"after rc=$?\"\n");
    assert_eq!(stdout, "after rc=2\n");
    assert!(stderr.contains("history: too many arguments"), "{stderr}");
    assert_eq!(code, Some(0));
}

#[test]
fn issue1660_history_too_many_args_unwinds_function() {
    let (stdout, _, code) =
        rubash_file("f() { history 1 2; echo \"INNER_NOT_PRINT\"; }\nf\necho \"frc=$?\"\n");
    assert_eq!(stdout, "frc=2\n");
    assert_eq!(code, Some(0));
}

#[test]
fn issue1660_history_too_many_args_abandons_dash_c_string() {
    let (stdout, _, code) = rubash("history 1 2; echo NOPE");
    assert_eq!(stdout, "");
    assert_eq!(code, Some(1));
}

#[test]
fn issue1660_history_provider_path_signals_too_many_arguments() {
    // The host-provider path (builtins/history.rs, driven by niu's
    // history provider) cannot be reached from this binary without a
    // host, so pin its contract directly: only the surplus-operand
    // failure in the listing path reports the DISCARD flag.
    let args: Vec<String> = ["1", "2"].iter().map(ToString::to_string).collect();
    let mut out: Vec<u8> = Vec::new();
    let mut err: Vec<u8> = Vec::new();
    let result =
        rubash::builtins::history::execute_with_history(&args, "rubash: ", &[], &mut out, &mut err)
            .expect("history provider call");
    assert_eq!(result, (2, true));
    assert!(String::from_utf8_lossy(&err).contains("too many arguments"));

    for (operand, expected) in [("abc", (2, false)), ("-z", (2, false)), ("1", (0, false))] {
        let args: Vec<String> = [operand].iter().map(ToString::to_string).collect();
        let mut out: Vec<u8> = Vec::new();
        let mut err: Vec<u8> = Vec::new();
        let result = rubash::builtins::history::execute_with_history(
            &args,
            "rubash: ",
            &[],
            &mut out,
            &mut err,
        )
        .expect("history provider call");
        assert_eq!(result, expected, "operand {operand}");
    }

    let args: Vec<String> = ["1", "2"].iter().map(ToString::to_string).collect();
    let mut err: Vec<u8> = Vec::new();
    let result = rubash::builtins::history::execute_with_io(&args, "rubash: ", &mut err)
        .expect("history io call");
    assert_eq!(result, (2, true));
}

#[test]
fn issue1660_history_non_numeric_operand_does_not_discard() {
    let (stdout, _, code) = rubash_file("history abc; echo \"rc=$?\"\n");
    assert_eq!(stdout, "rc=2\n");
    assert_eq!(code, Some(0));
}

#[test]
fn issue1660_history_invalid_option_does_not_discard() {
    let (stdout, _, code) = rubash_file("history -z; echo \"rc=$?\"\n");
    assert_eq!(stdout, "rc=2\n");
    assert_eq!(code, Some(0));
}
