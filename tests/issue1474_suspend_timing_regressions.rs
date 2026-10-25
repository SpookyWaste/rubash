//! rubash#1474 — `suspend -f` took effect only after the REST of the
//! script had run (the stop was delivered outside the execution
//! stream); GNU stops the process AT the builtin: output produced
//! before SIGCONT ends at the line before `suspend -f`.
//!
//! Probe: clusters/x2/repro/1474-sup.sh + 1474-target.sh — a session
//! supervisor (setsid) observes the stopped state and the partial
//! output, then sends SIGCONT. This test reproduces that harness:
//! the child runs in its own session, we poll /proc for state T,
//! check the partial output, then continue it.
//! Expectations verified against GNU bash 5.3.20 (same harness).

#![cfg(target_os = "linux")]

use std::io::Read;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn tempdir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rubash1474-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create tempdir");
    dir
}

fn proc_state(pid: u32) -> Option<char> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // comm may contain spaces/parens; state is the field after ") ".
    let after = stat.rsplit(") ").next()?;
    after.chars().next()
}

fn read_file(path: &PathBuf) -> String {
    let mut s = String::new();
    let _ = std::fs::File::open(path).map(|mut f| f.read_to_string(&mut s));
    s
}

#[test]
fn suspend_f_stops_before_following_commands() {
    let dir = tempdir("suspend");
    let target = dir.join("t.sh");
    std::fs::write(
        &target,
        "suspend\necho \"suspend rc=$?\"\nsuspend -f\necho \"suspend -f rc=$?\"\nlogout\necho \"logout rc=$?\"\n",
    )
    .expect("write target");
    let out_path = dir.join("out.txt");
    let err_path = dir.join("err.txt");
    let out_file = std::fs::File::create(&out_path).expect("create out");
    let err_file = std::fs::File::create(&err_path).expect("create err");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rubash"));
    cmd.arg("t.sh")
        .current_dir(&dir)
        .stdin(Stdio::null())
        .stdout(Stdio::from(out_file))
        .stderr(Stdio::from(err_file));
    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = cmd.spawn().expect("spawn rubash");
    let pid = child.id();

    // Poll up to 15s for the stopped state.
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut stopped = false;
    while Instant::now() < deadline {
        if proc_state(pid) == Some('T') {
            stopped = true;
            break;
        }
        if let Ok(Some(_)) = child.try_wait() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(stopped, "process never entered stopped state T");

    // Give any (wrongly) deferred commands a chance to run, then the
    // output must still end right before `suspend -f`.
    std::thread::sleep(Duration::from_millis(500));
    let partial = read_file(&out_path);
    assert_eq!(
        partial, "suspend rc=1\n",
        "commands after suspend -f ran before SIGCONT"
    );

    // Continue the whole session and let it finish.
    unsafe {
        libc::kill(-(pid as i32), libc::SIGCONT);
        libc::kill(pid as i32, libc::SIGCONT);
    }
    let status = child.wait().expect("wait child");
    assert!(status.success(), "final status: {status}");
    let full = read_file(&out_path);
    assert_eq!(
        full, "suspend rc=1\nsuspend -f rc=0\nlogout rc=1\n",
        "final output after SIGCONT"
    );
    let err = read_file(&err_path);
    assert!(
        err.contains("suspend: cannot suspend: no job control"),
        "stderr: {err}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
