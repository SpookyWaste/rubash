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
