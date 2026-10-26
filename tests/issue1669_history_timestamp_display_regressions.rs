//! rubash#1669 — the display/read residual of #1628: GNU renders a
//! history entry's stamp through builtins/history.def histtime on
//! top of lib/readline/history.c history_get_time — a strtol parse
//! (`#12x` is 12 seconds), a `<text>: invalid timestamp` form when
//! the parse yields 0 or localtime cannot represent the year, and
//! `??` for entries that carry no stamp. An entry carries no stamp
//! when the file was read while the timestamp comment char was
//! unarmed (no HISTTIMEFORMAT assigned yet and the file does not
//! start with a stamp line — histfile.c history_read's temporary
//! arming covers exactly that first-line shape). rubash rendered
//! every corrupt or missing stamp as the epoch.
//!
//! Expectations verified against GNU bash 5.3.20 (script files,
//! TZ=UTC).

use std::path::{Path, PathBuf};
use std::process::Command;

fn tempdir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rubash1669-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create tempdir");
    dir
}

fn run_script(dir: &Path, script: &str) -> (String, i32) {
    let script_path = dir.join("probe.sh");
    std::fs::write(&script_path, script).expect("write probe script");
    let output = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg(&script_path)
        .current_dir(dir)
        .output()
        .expect("run rubash");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        output.status.code().unwrap_or(-1),
    )
}

const FILES_AND_PHASES: &str = "\
export TZ=UTC
HISTTIMEFORMAT='[%s] '
history -r corrupt.txt
history
history -c
unset HISTTIMEFORMAT
history -r plain.txt
HISTTIMEFORMAT='[%s] '
history
history -c
unset HISTTIMEFORMAT
history -r mid.txt
HISTTIMEFORMAT='[%s] '
history
history -c
HISTTIMEFORMAT='[%Y] '
history -r band.txt
history
";

#[test]
fn corrupt_stamps_render_gnu_forms_and_unarmed_reads_render_question_marks() {
    let dir = tempdir("shapes");
    std::fs::write(
        dir.join("corrupt.txt"),
        "#12x\ncmd-partial\n#99999999999999999999999999\ncmd-overflow\n#0\ncmd-zero\n#1700000000\ncmd-normal\n",
    )
    .expect("write corrupt.txt");
    std::fs::write(dir.join("plain.txt"), "cmd-one\ncmd-two\n").expect("write plain.txt");
    std::fs::write(dir.join("mid.txt"), "cmd-first\n#1700000000\ncmd-second\n")
        .expect("write mid.txt");
    std::fs::write(
        dir.join("band.txt"),
        "#99999999999999999\ncmd-band17\n#67700000000000000\ncmd-band16\n#253402300799\ncmd-y9999\n",
    )
    .expect("write band.txt");
    let (stdout, status) = run_script(&dir, FILES_AND_PHASES);
    assert_eq!(status, 0, "script status; stdout: {stdout}");
    assert_eq!(
        stdout,
        "    1  [12] cmd-partial\n\
         \x20   2  99999999999999999999999999: invalid timestampcmd-overflow\n\
         \x20   3  0: invalid timestampcmd-zero\n\
         \x20   4  [1700000000] cmd-normal\n\
         \x20   1  ??cmd-one\n\
         \x20   2  ??cmd-two\n\
         \x20   1  ??cmd-first\n\
         \x20   2  ??#1700000000\n\
         \x20   3  ??cmd-second\n\
         \x20   1  99999999999999999: invalid timestampcmd-band17\n\
         \x20   2  [2145329566] cmd-band16\n\
         \x20   3  [9999] cmd-y9999\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_new_unarmed_entries_render_question_marks() {
    // Same unarmed shape through the -n (read_new_file) path.
    let dir = tempdir("readnew");
    std::fs::write(dir.join("plain.txt"), "cmd-one\ncmd-two\n").expect("write plain.txt");
    let script = "\
export TZ=UTC
history -n plain.txt
HISTTIMEFORMAT='[%s] '
history
";
    let (stdout, status) = run_script(&dir, script);
    assert_eq!(status, 0, "script status; stdout: {stdout}");
    assert_eq!(stdout, "    1  ??cmd-one\n    2  ??cmd-two\n");
    let _ = std::fs::remove_dir_all(&dir);
}
