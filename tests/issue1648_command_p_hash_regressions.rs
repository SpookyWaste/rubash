//! Issue rubash#1648: `command -p` consulted the command hash table.
//!
//! GNU findcmd.c search_for_command: since bash 5.2, a lookup under the
//! default/standard path (CDESC_STDPATH, set by `command -p`) walks that
//! path only and never consults phash — a name planted with
//! `hash -p /bin/false myls` must NOT resolve through `command -p myls`,
//! while a plain `command myls` still runs the hashed path and the
//! describe forms (`command -v`, `hash -t`) still report it.
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
fn issue1648_command_p_ignores_hash_table() {
    let (stdout, stderr, code) = rubash("hash -p /bin/false myls; command -p myls; echo \"rc=$?\"");
    assert_eq!(stdout, "rc=127\n");
    assert!(stderr.contains("myls: command not found"), "{stderr}");
    assert_eq!(code, Some(0));
}

#[test]
fn issue1648_plain_command_still_uses_hash_table() {
    // /bin/false exits 1: the hashed path DID run.
    let (stdout, _, _) = rubash("hash -p /bin/false myls; command myls; echo \"rc=$?\"");
    assert_eq!(stdout, "rc=1\n");
}

#[test]
fn issue1648_describe_forms_still_report_hash_entry() {
    let (stdout, _, _) =
        rubash("hash -p /bin/false myls; command -v myls; hash -t myls; command -pv myls");
    assert_eq!(stdout, "/bin/false\n/bin/false\n/bin/false\n");
}

#[test]
fn issue1648_command_p_positive_control_walks_standard_path() {
    let (stdout, _, code) = rubash("command -p echo pctl; echo \"rc=$?\"");
    assert_eq!(stdout, "pctl\nrc=0\n");
    assert_eq!(code, Some(0));
}
