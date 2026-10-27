//! Regression pins for the 2026-10-27 simulation-layer narrowing
//! ruling (rubash#1636, #1725, #1726; DECISIONS.md 2026-10-27,
//! analysis memo `full-audit/niu-builtins/MEMO.md` 变体甲）.
//!
//! The in-process emulations in `src/executor/external_file_builtins.rs`
//! stand in for PATH binaries. The ruling: an emulation may answer only
//! the shapes it evaluates exactly; every shape it accepts but does not
//! implement — the memo's seven silent subsets — declines to the real
//! binary instead of silently preempting it with a subset simulation.
//!
//! The decisive observable is *which* implementation answered. Most
//! pins below put a fake binary first in PATH (a script that prints a
//! `FAKEBIN-<NAME>` marker and exits 42): if the marker appears, the
//! invocation declined and the PATH binary ran; if the emulation still
//! answers, the marker never appears and the emulation's own result is
//! observed instead. Control rows pin the kept subset: shapes the
//! emulation evaluates exactly must still be answered in-process (no
//! marker, the emulation's documented result). Semantic rows run with
//! the real system PATH and assert the GNU/coreutils outcome itself.
//!
//! Fake-binary rows are unix-only (they need a `#!/bin/sh` stand-in).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn shell_bin() -> PathBuf {
    if let Some(path) = option_env!("CARGO_BIN_EXE_rubash") {
        return PathBuf::from(path);
    }
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(profile)
        .join(if cfg!(windows) {
            "rubash.exe"
        } else {
            "rubash"
        })
}

struct Output {
    status: i32,
    stdout: String,
    stderr: String,
}

fn run_in_dir(dir: &Path, script: &str, path_env: &str) -> Output {
    let mut child = Command::new(shell_bin())
        .args(["-c", script])
        .current_dir(dir)
        .env("PATH", path_env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rubash");
    let _ = child.stdin.take();
    let output = child.wait_with_output().expect("wait for rubash");
    Output {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).replace('\r', ""),
    }
}

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rubash-sim-decline-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// A PATH whose first entry holds one fake binary per name in `names`
/// (each prints `FAKEBIN-<NAME>` plus its argv and exits 42), followed
/// by the real system directories.
#[cfg(unix)]
fn fakebin_path(dir: &Path, names: &[&str]) -> String {
    let fakebin = dir.join("fakebin");
    std::fs::create_dir_all(&fakebin).expect("fakebin dir");
    for name in names {
        let script = format!("#!/bin/sh\necho \"FAKEBIN-{name} args:$*\"\nexit 42\n");
        let path = fakebin.join(name);
        std::fs::write(&path, script).expect("fake binary");
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("fake binary mode");
    }
    format!("{}:/usr/bin:/bin", fakebin.display())
}

const SYSTEM_PATH: &str = "/usr/bin:/bin";

// --------------------------------------------------------------------
// rubash#1725 — mkfifo: off Windows the emulation declines entirely;
// the real binary creates a real fifo. (The Windows face keeps the
// honest `Operation not supported` refusal, with diagnostics routed
// through the command's own redirections.)
// --------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn issue1725_mkfifo_creates_a_real_fifo() {
    use std::os::unix::fs::FileTypeExt;
    let dir = scratch("mkfifo-real");
    let out = run_in_dir(&dir, "mkfifo p; echo rc=$?", SYSTEM_PATH);
    let is_fifo = std::fs::symlink_metadata(dir.join("p"))
        .map(|meta| meta.file_type().is_fifo())
        .unwrap_or(false);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(out.stdout.trim(), "rc=0", "stderr={:?}", out.stderr);
    assert!(is_fifo, "mkfifo must create a fifo, not a regular file");
}

#[cfg(unix)]
#[test]
fn issue1725_mkfifo_failure_is_the_real_binarys_diagnostic() {
    let dir = scratch("mkfifo-exists");
    std::fs::write(dir.join("p"), b"taken").expect("fixture file");
    let out = run_in_dir(
        &dir,
        "mkfifo p 2>err.txt; echo rc=$?; cat err.txt",
        SYSTEM_PATH,
    );
    let _ = std::fs::remove_dir_all(&dir);
    // GNU coreutils mkfifo on an existing path: rc 1 and its own
    // `cannot create fifo ... File exists` text on the command's
    // redirected stderr — pre-fix the emulation printed
    // `Operation not supported` to the terminal, ignoring the 2>.
    assert!(
        out.stdout.contains("rc=1"),
        "stdout={:?} stderr={:?}",
        out.stdout,
        out.stderr
    );
    assert!(
        out.stdout.contains("File exists"),
        "stdout={:?} stderr={:?}",
        out.stdout,
        out.stderr
    );
}

// --------------------------------------------------------------------
// rubash#1726 — chmod: mode classes and option shapes the evaluator
// cannot honor decline to the real binary; the bare MODE + files form
// with plain rwx bits stays in-process.
// --------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn issue1726_chmod_octal_special_bits_are_preserved() {
    let dir = scratch("chmod-1777");
    // Pre-fix the emulation masked the value with & 0o777 and landed
    // 0777 without a word; GNU keeps the sticky bit.
    let out = run_in_dir(&dir, "touch f; chmod 1777 f; stat -c %a f", SYSTEM_PATH);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(out.stdout.trim(), "1777", "stderr={:?}", out.stderr);
}

#[cfg(unix)]
#[test]
fn issue1726_chmod_symbolic_special_bits_apply() {
    let dir = scratch("chmod-symbolic");
    // The issue's seed: `a+rwxt` is legal GNU (rwx for all + sticky);
    // pre-fix the emulation rejected it as an invalid mode.
    let out = run_in_dir(
        &dir,
        "touch f; chmod 755 f; chmod a+rwxt f; stat -c %a f",
        SYSTEM_PATH,
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(out.stdout.trim(), "1777", "stderr={:?}", out.stderr);
}

#[cfg(unix)]
#[test]
fn issue1726_chmod_unhonored_shapes_reach_the_path_binary() {
    let dir = scratch("chmod-decline");
    std::fs::write(dir.join("f"), b"x").expect("fixture f");
    std::fs::create_dir_all(dir.join("d")).expect("fixture d");
    let path = fakebin_path(&dir, &["chmod"]);
    for script in [
        // -R never recursed; --reference was swallowed and the next
        // word misparsed as the mode; a missing operand was answered
        // silently; s/t and copy classes and special bits are #1726.
        "chmod -R 700 d",
        "chmod --reference=f f",
        "chmod",
        "chmod 1777 f",
        "chmod u+s f",
        "chmod g=u f",
        "chmod u+rw-x f",
    ] {
        let out = run_in_dir(&dir, script, &path);
        assert!(
            out.stdout.contains("FAKEBIN-chmod") && out.status == 42,
            "{script}: expected the PATH binary to answer, got status={} stdout={:?} stderr={:?}",
            out.status,
            out.stdout,
            out.stderr
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

// --------------------------------------------------------------------
// rubash#1636 — the umbrella narrowing: rm's interactive forms, the
// rmdir stub, mkdir's non--p options, touch's -h and missing
// operands, cp's unimplemented option shapes, and tty's operand and
// device-name shapes decline; each command's kept subset stays
// in-process.
// --------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn issue1636_rm_interactive_forms_reach_the_path_binary() {
    let dir = scratch("rm-decline");
    std::fs::write(dir.join("victim"), b"x").expect("fixture victim");
    let path = fakebin_path(&dir, &["rm"]);
    // -i/-I/--interactive were accepted and the file was deleted
    // without any prompt; --one-file-system was accepted and
    // ignored. All of them belong to the real binary.
    for script in [
        "rm -i victim",
        "rm -I victim",
        "rm --interactive victim",
        "rm --interactive=always victim",
        "rm -r --one-file-system victim",
    ] {
        let out = run_in_dir(&dir, script, &path);
        assert!(
            out.stdout.contains("FAKEBIN-rm") && out.status == 42,
            "{script}: expected the PATH binary to answer, got status={} stdout={:?} stderr={:?}",
            out.status,
            out.stdout,
            out.stderr
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn issue1636_rm_plain_forms_stay_in_process() {
    let dir = scratch("rm-kept");
    std::fs::write(dir.join("victim"), b"x").expect("fixture victim");
    std::fs::create_dir_all(dir.join("tree/sub")).expect("fixture tree");
    std::fs::write(dir.join("tree/sub/f"), b"y").expect("fixture tree file");
    let path = fakebin_path(&dir, &["rm"]);
    let out = run_in_dir(&dir, "rm victim; rm -r tree; echo done", &path);
    let kept = dir.join("victim").exists() || dir.join("tree").exists();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(out.stdout.trim(), "done", "stderr={:?}", out.stderr);
    assert!(!kept, "the emulation must have removed both operands");
}

#[cfg(unix)]
#[test]
fn issue1636_rmdir_reaches_the_path_binary() {
    let dir = scratch("rmdir-decline");
    std::fs::create_dir_all(dir.join("d")).expect("fixture d");
    let path = fakebin_path(&dir, &["rmdir"]);
    let out = run_in_dir(&dir, "rmdir d", &path);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        out.stdout.contains("FAKEBIN-rmdir") && out.status == 42,
        "expected the PATH binary to answer, got status={} stdout={:?} stderr={:?}",
        out.status,
        out.stdout,
        out.stderr
    );
}

#[cfg(unix)]
#[test]
fn issue1636_rmdir_reports_failures_and_removes_ancestors() {
    let dir = scratch("rmdir-semantics");
    // Pre-fix: `rmdir missing` exited 0 in silence and `rmdir -p`
    // removed only the leaf. GNU: failure is status 1 with a
    // diagnostic, and -p removes the whole ancestor chain.
    let out = run_in_dir(
        &dir,
        "mkdir -p a/b/c; rmdir -p a/b/c; test ! -d a && echo CHAIN-GONE; rmdir missing; echo rc=$?",
        SYSTEM_PATH,
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        out.stdout.contains("CHAIN-GONE"),
        "stdout={:?} stderr={:?}",
        out.stdout,
        out.stderr
    );
    assert!(
        out.stdout.contains("rc=1"),
        "stdout={:?} stderr={:?}",
        out.stdout,
        out.stderr
    );
    assert!(
        out.stderr.contains("rmdir:"),
        "the real rmdir diagnostic must surface, stderr={:?}",
        out.stderr
    );
}

#[cfg(unix)]
#[test]
fn issue1636_mkdir_unimplemented_options_reach_the_path_binary() {
    let dir = scratch("mkdir-decline");
    let path = fakebin_path(&dir, &["mkdir"]);
    // -m's mode was consumed and never applied, -v never printed,
    // unknown options were swallowed while the directory was still
    // created. All decline now.
    for script in [
        "mkdir -m 700 d",
        "mkdir --mode=700 d",
        "mkdir -v d",
        "mkdir --bogus d",
        "mkdir -Z d",
        "mkdir -pm 700 d",
    ] {
        let out = run_in_dir(&dir, script, &path);
        assert!(
            out.stdout.contains("FAKEBIN-mkdir") && out.status == 42,
            "{script}: expected the PATH binary to answer, got status={} stdout={:?} stderr={:?}",
            out.status,
            out.stdout,
            out.stderr
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn issue1636_mkdir_mode_applies_and_unknown_options_fail_with_the_real_binary() {
    let dir = scratch("mkdir-semantics");
    let out = run_in_dir(&dir, "mkdir -m 700 d; stat -c %a d", SYSTEM_PATH);
    assert_eq!(out.stdout.trim(), "700", "stderr={:?}", out.stderr);
    let out = run_in_dir(&dir, "mkdir --bogus e; echo rc=$?", SYSTEM_PATH);
    let made_e = dir.join("e").exists();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        out.stdout.contains("rc=1") && !made_e,
        "stdout={:?} stderr={:?} made_e={made_e}",
        out.stdout,
        out.stderr
    );
}

#[cfg(unix)]
#[test]
fn issue1636_mkdir_parents_stays_in_process() {
    let dir = scratch("mkdir-kept");
    let path = fakebin_path(&dir, &["mkdir"]);
    let out = run_in_dir(&dir, "mkdir -p a/b/c; echo done", &path);
    let made = dir.join("a/b/c").is_dir();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(out.stdout.trim(), "done", "stderr={:?}", out.stderr);
    assert!(made, "the emulation must still create the -p chain");
}

#[cfg(unix)]
#[test]
fn issue1636_touch_unimplemented_shapes_reach_the_path_binary() {
    let dir = scratch("touch-decline");
    std::fs::write(dir.join("f"), b"x").expect("fixture f");
    let path = fakebin_path(&dir, &["touch"]);
    // -h was a no-op (the stamp followed the symlink), and a missing
    // operand list exited 0 silently.
    for script in ["touch -h f", "touch --no-dereference f", "touch"] {
        let out = run_in_dir(&dir, script, &path);
        assert!(
            out.stdout.contains("FAKEBIN-touch") && out.status == 42,
            "{script}: expected the PATH binary to answer, got status={} stdout={:?} stderr={:?}",
            out.status,
            out.stdout,
            out.stderr
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn issue1636_touch_without_operands_reports_missing_operand() {
    let dir = scratch("touch-no-operand");
    let out = run_in_dir(
        &dir,
        "touch 2>err.txt; echo rc=$?; cat err.txt",
        SYSTEM_PATH,
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        out.stdout.contains("rc=1") && out.stdout.contains("missing file operand"),
        "stdout={:?} stderr={:?}",
        out.stdout,
        out.stderr
    );
}

#[cfg(unix)]
#[test]
fn issue1636_touch_no_dereference_stamps_the_link_not_the_target() {
    let dir = scratch("touch-h-semantics");
    // A dangling symlink: GNU `touch -h` stamps the link itself and
    // succeeds; following the link (the emulation's old -h no-op)
    // would have created the target instead.
    let out = run_in_dir(
        &dir,
        "ln -s nofile link; touch -h link; echo rc=$?; test ! -e nofile && echo NO-TARGET; test -L link && echo LINK-KEPT",
        SYSTEM_PATH,
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        out.stdout.contains("rc=0")
            && out.stdout.contains("NO-TARGET")
            && out.stdout.contains("LINK-KEPT"),
        "stdout={:?} stderr={:?}",
        out.stdout,
        out.stderr
    );
}

#[cfg(unix)]
#[test]
fn issue1636_touch_plain_forms_stay_in_process() {
    let dir = scratch("touch-kept");
    let path = fakebin_path(&dir, &["touch"]);
    let out = run_in_dir(&dir, "touch f; echo done", &path);
    let made = dir.join("f").exists();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(out.stdout.trim(), "done", "stderr={:?}", out.stderr);
    assert!(made, "the emulation must still create the file");
}

#[cfg(unix)]
#[test]
fn issue1636_cp_unimplemented_options_reach_the_path_binary() {
    let dir = scratch("cp-decline");
    std::fs::write(dir.join("a"), b"data").expect("fixture a");
    let path = fakebin_path(&dir, &["cp"]);
    // -l/-s produced plain copies, --update=all was read as the
    // `older` semantics, --help printed the emulation's own short
    // text to stderr, and the rest were accepted no-ops.
    for script in [
        "cp -l a b",
        "cp -s a b",
        "cp -d a b",
        "cp -P a b",
        "cp -f a b",
        "cp --parents a b",
        "cp --backup a b",
        "cp -S .bak a b",
        "cp --update=all a b",
        "cp --update=none a b",
        "cp --help",
    ] {
        let out = run_in_dir(&dir, script, &path);
        assert!(
            out.stdout.contains("FAKEBIN-cp") && out.status == 42,
            "{script}: expected the PATH binary to answer, got status={} stdout={:?} stderr={:?}",
            out.status,
            out.stdout,
            out.stderr
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn issue1636_cp_link_options_have_real_effects() {
    let dir = scratch("cp-link-semantics");
    std::fs::write(dir.join("a"), b"data").expect("fixture a");
    // -l must produce a hard link (same inode), -s a symlink.
    let out = run_in_dir(
        &dir,
        "cp -l a b; stat -c %i a; stat -c %i b; cp -s a l; test -L l && echo IS-LINK",
        SYSTEM_PATH,
    );
    let _ = std::fs::remove_dir_all(&dir);
    let lines: Vec<&str> = out.stdout.lines().collect();
    assert_eq!(
        lines.len(),
        3,
        "stdout={:?} stderr={:?}",
        out.stdout,
        out.stderr
    );
    assert_eq!(lines[0], lines[1], "cp -l must share the inode");
    assert_eq!(lines[2], "IS-LINK");
}

#[cfg(unix)]
#[test]
fn issue1636_cp_update_all_copies_even_when_source_is_older() {
    let dir = scratch("cp-update-all");
    // GNU --update=all copies regardless of timestamps; the
    // emulation read every --update form as `older` and skipped.
    let out = run_in_dir(
        &dir,
        "echo NEW > a; echo OLD > b; touch -d 2020-01-01 a; touch -d 2021-01-01 b; cp --update=all a b; cat b",
        SYSTEM_PATH,
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(out.stdout.trim(), "NEW", "stderr={:?}", out.stderr);
}

#[cfg(unix)]
#[test]
fn issue1636_cp_plain_forms_stay_in_process() {
    let dir = scratch("cp-kept");
    std::fs::write(dir.join("a"), b"data").expect("fixture a");
    std::fs::create_dir_all(dir.join("tree")).expect("fixture tree");
    std::fs::write(dir.join("tree/f"), b"y").expect("fixture tree file");
    let path = fakebin_path(&dir, &["cp"]);
    let out = run_in_dir(&dir, "cp a b; cp -r tree copy; echo done", &path);
    let copied = std::fs::read(dir.join("b"))
        .map(|v| v == b"data")
        .unwrap_or(false);
    let treed = std::fs::read(dir.join("copy/f"))
        .map(|v| v == b"y")
        .unwrap_or(false);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(out.stdout.trim(), "done", "stderr={:?}", out.stderr);
    assert!(copied && treed, "the emulation must still copy in-process");
}

#[cfg(unix)]
#[test]
fn issue1636_tty_extra_operand_is_a_usage_error() {
    let dir = scratch("tty-operand");
    // GNU: `tty junk` is a usage error (exit 2, `extra operand`
    // diagnostic); the emulation silently ignored the operand.
    let out = run_in_dir(&dir, "tty junk", SYSTEM_PATH);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(
        out.status, 2,
        "stdout={:?} stderr={:?}",
        out.stdout, out.stderr
    );
    assert!(
        out.stderr.contains("extra operand"),
        "stderr={:?}",
        out.stderr
    );
}

#[cfg(unix)]
#[test]
fn issue1636_tty_kept_shapes_stay_in_process() {
    let dir = scratch("tty-kept");
    let path = fakebin_path(&dir, &["tty"]);
    // stdin is not a terminal here: bare `tty` prints `not a tty`
    // (status 1) and `-s` prints nothing (status 1), both answered
    // by the emulation — no marker may appear.
    let out = run_in_dir(&dir, "tty; echo rc=$?", &path);
    assert_eq!(out.stdout, "not a tty\nrc=1\n", "stderr={:?}", out.stderr);
    let out = run_in_dir(&dir, "tty -s; echo rc=$?", &path);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(out.stdout, "rc=1\n", "stderr={:?}", out.stderr);
}

#[cfg(unix)]
#[test]
fn issue1636_tty_on_a_terminal_prints_the_real_device_name() {
    // Under a pty, a bare `tty` must print the terminal's real name
    // (the emulation used to fabricate `/dev/tty`). util-linux
    // `script` provides the pty; without it this pin cannot run.
    if !Path::new("/usr/bin/script").exists() {
        return;
    }
    let dir = scratch("tty-pty");
    let probe = dir.join("probe.sh");
    std::fs::write(
        &probe,
        format!(
            "#!/bin/sh\nexec {} -c 'tty; echo rc=$?'\n",
            shell_bin().display()
        ),
    )
    .expect("probe script");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&probe, std::fs::Permissions::from_mode(0o755)).expect("probe mode");
    let output = Command::new("/usr/bin/script")
        .args(["-qec", &format!("{}", probe.display()), "/dev/null"])
        .env("PATH", SYSTEM_PATH)
        .stdin(Stdio::null())
        .output()
        .expect("run script(1)");
    let stdout = String::from_utf8_lossy(&output.stdout).replace('\r', "");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        stdout.contains("/dev/pts/") && stdout.contains("rc=0"),
        "expected the real binary's pts name, got {stdout:?}"
    );
    assert!(
        !stdout.lines().any(|line| line.trim() == "/dev/tty"),
        "the fabricated name must not appear, got {stdout:?}"
    );
}

#[cfg(unix)]
#[test]
fn issue1726_chmod_plain_forms_stay_in_process() {
    let dir = scratch("chmod-kept");
    std::fs::write(dir.join("f"), b"x").expect("fixture f");
    let path = fakebin_path(&dir, &["chmod"]);
    // Kept subset: bare octal and single-operator rwx symbolics are
    // evaluated exactly by the emulation — no marker, and the real
    // inode mode (read back through the real stat) proves it answered.
    let out = run_in_dir(&dir, "chmod 755 f; stat -c %a f", &path);
    assert_eq!(out.stdout.trim(), "755", "stderr={:?}", out.stderr);
    let out = run_in_dir(&dir, "chmod u+x f; stat -c %a f", &path);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(out.stdout.trim(), "755", "stderr={:?}", out.stderr);
}
