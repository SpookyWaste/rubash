//! rubash#1046: `GROUPS` must reflect the process's real group set,
//! not a hardcoded `(0)`.
//!
//! GNU anchor: `variables.c` binds GROUPS dynamically from
//! getgroups(): element 0 is the real group id and the array lists
//! every supplementary group, matching `id -G`. rubash hardcoded
//! `(0)`, so two different users saw identical values; the same
//! report notes `compgen -A arrayvar` listed FUNCNAME instead of
//! GROUPS at top level.
//!
//! Expectations verified against GNU bash 5.3.20. The checks are
//! self-referential (the shell compares its own GROUPS against the
//! `id` utility run in the same context), so they hold for any
//! test account.
#![cfg(unix)]

use std::process::{Command, Stdio};

fn run_script(script: &str) -> (Option<i32>, String, String) {
    let dir = std::env::temp_dir().join(format!("rubash-issue1046-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create issue1046 scratch dir");
    let script_path = dir.join("probe.sh");
    std::fs::write(&script_path, script).expect("write probe script");
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg(&script_path)
        .stdin(Stdio::null())
        .env_remove("BASH_ENV")
        .env_remove("WINUXSH_ROOT")
        .output()
        .expect("run rubash script");
    let _ = std::fs::remove_dir_all(&dir);
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// GROUPS equals `id -G`, its first element equals `id -g`, it is
/// an indexed array, and completion lists it among array variables.
#[test]
fn groups_reflects_the_real_group_set() {
    let (code, stdout, _) = run_script(
        "g=\"${GROUPS[*]}\"; i=$(id -G); [ \"$g\" = \"$i\" ] && echo GROUPS-MATCH || echo \"GROUPS-DIFF g=[$g] i=[$i]\"\n\
         [ \"${GROUPS[0]}\" = \"$(id -g)\" ] && echo FIRST-OK || echo FIRST-BAD\n\
         declare -p GROUPS | grep -q '^declare -a GROUPS=' && echo DECLARE-A-OK || echo DECLARE-BAD\n\
         compgen -A arrayvar | grep -qx GROUPS && echo COMPGEN-OK || echo COMPGEN-BAD\n",
    );
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "GROUPS-MATCH\nFIRST-OK\nDECLARE-A-OK\nCOMPGEN-OK\n");
}
