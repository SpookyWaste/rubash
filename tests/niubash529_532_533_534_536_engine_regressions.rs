//! Regression pins for niubash issues whose root cause and fix live in
//! THIS engine. Per the cross-repo convention, niubash's ENGINE-DUP rows
//! are pinned on the rubash side (precedent: `issue519_dev_tty_redirect_
//! regressions.rs` pins niubash#519 here); niubash does not duplicate
//! engine tests. Each block names its niubash issue and the GNU behavior
//! the fix restored.
//!
//! niubash#529: inside `$( )`, `[ -t 1 ]` reported TRUE — command
//! substitution must see its stdout as the pipe it is (isatty follows
//! the real fd, subst.c command_substitute gives the child a pipe).
//!   GNU on a pty: A=N (in $()) / B=Y (fd 2 untouched) / D=Y (direct).
//!   The non-pty form below pins the same invariant: with stdout piped,
//!   the $() test is false.
//!
//! niubash#532 (P1): a compound command with its own redirect,
//! `( echo A; printf 'B\n' | cat; echo C ) > file`, LOST the pipeline's
//! output — the shell's write handle and the pipeline tail's write
//! handle were two independent offsets into the same file, so C
//! overwrote B at offset 2. GNU: one shared open file description per
//! redirect target; the file ends up `A\nB\nC\n`.
//!
//! niubash#533: the file-builtin family's timestamp layer was empty —
//! `touch -t/-d/-r` silently did nothing (rc 0) and `cp -p`/`cp -a` did
//! not preserve mtime. GNU coreutils semantics: the flags take effect
//! or the command fails loudly, never silent success.
//!
//! niubash#534 (P2): `rm dir` without -r REMOVED directories (even
//! non-empty), rc 0, no message. GNU coreutils: `rm: cannot remove
//! 'dir': Is a directory`, rc 1, directory and contents untouched.
//!
//! niubash#536: `read -n 2` on a 3-byte no-newline file returned 1
//! after successfully reading 2 chars (GNU: reading the requested
//! count is success, rc 0; the trailing EOF is not a failure), and
//! `read -u 3 v 3<file` reported `Bad file descriptor` — the fd
//! established by the SAME command's redirection was invisible to the
//! builtin (GNU reads `abc` into v; rc 1 there is only the
//! EOF-without-newline rule).

use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, UNIX_EPOCH};

fn rubash(script: &str) -> (String, String, Option<i32>) {
    rubash_in(&std::env::temp_dir(), script)
}

fn rubash_in(dir: &PathBuf, script: &str) -> (String, String, Option<i32>) {
    let out = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg("-c")
        .arg(script)
        .current_dir(dir)
        .output()
        .expect("spawn rubash");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code(),
    )
}

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rubash-bfd-niu-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn mtime_secs(path: &PathBuf) -> i64 {
    let meta = std::fs::metadata(path).expect("metadata");
    let t = meta.modified().expect("modified");
    t.duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_else(|e| -(e.duration().as_secs() as i64))
}

// ---------- niubash#529 ----------

#[test]
fn niubash529_isatty_inside_command_substitution_is_false() {
    // This test process pipes the shell's stdout, so fd 1 is not a tty
    // anywhere in the script; inside $() it must ALSO be false (the bug
    // leaked the outer answer into the substitution).
    let (stdout, _e, _c) =
        rubash("echo A=$([ -t 1 ] && echo Y || echo N); echo D=$([ -t 1 ] && echo Y || echo N)");
    assert_eq!(
        stdout, "A=N\nD=N\n",
        "[ -t 1 ] inside $() must follow the substitution's pipe, got: {stdout:?}"
    );
}

// ---------- niubash#532 ----------

#[test]
fn niubash532_compound_redirect_pipeline_output_not_overwritten() {
    let dir = scratch("532");
    let (_o, _e, code) = rubash_in(
        &dir,
        "( echo A; printf 'B\\n' | cat; echo C ) > g1; \
         ( echo A; echo B | cat; echo C ) > g4; \
         ( echo A; printf 'B\\n' | cat ) > g3",
    );
    let g1 = std::fs::read_to_string(dir.join("g1")).unwrap_or_default();
    let g4 = std::fs::read_to_string(dir.join("g4")).unwrap_or_default();
    let g3 = std::fs::read_to_string(dir.join("g3")).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(code, Some(0));
    assert_eq!(
        g1, "A\nB\nC\n",
        "pipeline segment in the middle must survive (shared write offset)"
    );
    assert_eq!(g4, "A\nB\nC\n", "all-builtin variant of the same shape");
    assert_eq!(g3, "A\nB\n", "pipeline-last control leg (always worked)");
}

// ---------- niubash#533 ----------

#[test]
fn niubash533_touch_timestamp_flags_take_effect() {
    let dir = scratch("533");
    std::fs::write(dir.join("tf1"), b"").expect("tf1");
    let (_o, _e, code) = rubash_in(&dir, "touch -t 202001020304.05 tf1");
    assert_eq!(code, Some(0));
    let t = mtime_secs(&dir.join("tf1"));
    assert!(
        (1_577_836_800..1_609_459_200).contains(&t),
        "touch -t must set the mtime to 2020-01-02 (any tz), got epoch {t}"
    );
    let (_o2, _e2, code2) = rubash_in(&dir, "touch -d '2019-01-01 00:00:00' tf1");
    assert_eq!(code2, Some(0));
    let t2 = mtime_secs(&dir.join("tf1"));
    assert!(
        (1_546_000_000..1_577_836_800).contains(&t2),
        "touch -d must set the mtime into 2019 (any tz), got epoch {t2}"
    );
    // -r copies the reference file's time exactly.
    let (_o3, _e3, code3) = rubash_in(&dir, "touch tf5; touch -r tf1 tf5");
    assert_eq!(code3, Some(0));
    let t_ref = std::fs::metadata(dir.join("tf1"))
        .unwrap()
        .modified()
        .unwrap();
    let t_copy = std::fs::metadata(dir.join("tf5"))
        .unwrap()
        .modified()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(
        t_ref, t_copy,
        "touch -r must copy the reference mtime exactly"
    );
}

#[test]
fn niubash533_cp_preserve_keeps_mtime() {
    let dir = scratch("533cp");
    std::fs::write(dir.join("src.txt"), b"payload").expect("src");
    let (_o, _e, _c) = rubash_in(&dir, "touch -t 201806070809.10 src.txt");
    let src_t = std::fs::metadata(dir.join("src.txt"))
        .unwrap()
        .modified()
        .unwrap();
    let (_o2, _e2, code) = rubash_in(&dir, "cp -p src.txt d_p.txt; cp -a src.txt d_a.txt");
    assert_eq!(code, Some(0));
    let p_t = std::fs::metadata(dir.join("d_p.txt"))
        .unwrap()
        .modified()
        .unwrap();
    let a_t = std::fs::metadata(dir.join("d_a.txt"))
        .unwrap()
        .modified()
        .unwrap();
    // Control: plain cp stamps the copy with ~now (both shells agree).
    let (_o3, _e3, _c3) = rubash_in(&dir, "cp src.txt d_plain.txt");
    let plain_t = std::fs::metadata(dir.join("d_plain.txt"))
        .unwrap()
        .modified()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(src_t, p_t, "cp -p must preserve mtime");
    assert_eq!(src_t, a_t, "cp -a must preserve mtime");
    assert!(
        plain_t > src_t + Duration::from_secs(3600),
        "plain cp must NOT preserve the 2018 mtime (control leg)"
    );
}

// ---------- niubash#534 ----------

#[test]
fn niubash534_rm_without_r_refuses_directories() {
    let dir = scratch("534");
    std::fs::create_dir_all(dir.join("rmdir1/sub")).expect("mkdir tree");
    std::fs::write(dir.join("rmdir1/sub/f"), b"x").expect("nested file");
    std::fs::create_dir_all(dir.join("r_empty")).expect("mkdir empty");
    let (stdout, stderr, _c) = rubash_in(&dir, "rm rmdir1; echo rc1=$?; rm r_empty; echo rc2=$?");
    let tree_alive = dir.join("rmdir1/sub/f").exists();
    let empty_alive = dir.join("r_empty").is_dir();
    // -r control leg: recursion still works when asked for.
    let (stdout2, _e2, _c2) = rubash_in(&dir, "rm -r rmdir1; echo rc_r=$?");
    let tree_gone = !dir.join("rmdir1").exists();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        stdout.contains("rc1=1") && stdout.contains("rc2=1"),
        "rm on a directory (empty or not) returns 1, got stdout: {stdout:?}"
    );
    assert!(
        stderr.contains("Is a directory"),
        "rm on a directory must say why it refuses, got: {stderr:?}"
    );
    assert!(tree_alive, "non-empty directory must survive rm without -r");
    assert!(
        empty_alive,
        "empty directory must also survive rm without -r (no -d given)"
    );
    assert!(
        tree_gone && stdout2.contains("rc_r=0"),
        "rm -r control leg must still remove the tree with rc 0, got: {stdout2:?}"
    );
}

// ---------- niubash#536 ----------

#[test]
fn niubash536_read_n_full_count_then_eof_is_success() {
    let dir = scratch("536");
    std::fs::write(dir.join("r1"), b"abc").expect("r1"); // no trailing newline
    let (stdout, _e, _c) = rubash_in(&dir, "read -n 2 v < r1; echo \"rc=$? v=[$v]\"");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(
        stdout.trim(),
        "rc=0 v=[ab]",
        "read -n that got its full count returns 0 even at EOF"
    );
}

#[test]
fn niubash536_read_u_uses_same_command_redirect_fd() {
    let dir = scratch("536u");
    std::fs::write(dir.join("r1"), b"abc").expect("r1");
    let (stdout, stderr, _c) = rubash_in(&dir, "read -u 3 v 3<r1; echo \"rc=$? v=[$v]\"");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(
        stdout.trim(),
        "rc=1 v=[abc]",
        "read -u 3 must read via the fd its own command just opened (rc 1 = EOF without newline, as GNU)"
    );
    assert!(
        stderr.is_empty(),
        "no Bad file descriptor diagnostic may appear, got: {stderr:?}"
    );
}
