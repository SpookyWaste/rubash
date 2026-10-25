//! Issue rubash#1640: a shell-directed SIGINT aborts the builtin
//! sleep with 130; GNU lets it complete (rc=0).
//!
//! The builtin `sleep` is a "virtual child" (trap_exec
//! `sleep_interruptibly`): there is no real foreground child whose
//! death could report the signal, so the wait models one. The model
//! must key on WHO the delivery could have killed (see
//! `KERNEL_CAUSED` in kill.rs): only a terminal-generated delivery
//! (si_code SI_KERNEL, i.e. ^C) kills GNU's real foreground child,
//! so only a kernel-caused SIGINT may end the virtual sleep early.
//! A kill(2) delivery addressed to this shell alone leaves GNU's
//! real child running, so the sleep completes with rc=0 and the
//! signal stays recorded for boundary handling.
//!
//! The bug was in `ForegroundBuiltinSigintGuard::take_sigint_abort`
//! plus the observe fold right after it in `sleep_interruptibly`:
//! both aborted on ANY untrapped SIGINT, so a `kill -INT <shell>`
//! during a tty-interactive builtin sleep reported 130. The fix
//! gates both on `last_delivery_kernel_caused`.
//!
//! Harness: the `script(1)` PTY pattern from
//! tests/issue1016_interactive_pty_regressions.rs, extended with a
//! shared transcript the writer synchronizes on: the writer waits
//! for the shell's own READY marker before starting the sleep, so
//! the kill/^C always lands mid-sleep even on a slow host (a fixed
//! 1s pace is not enough there — the first PTY prompt alone took
//! 5s on the dev VM). The shell pid is found by walking the
//! process tree script -> sh -c -> rubash by ppid, so parallel
//! PTY sessions can never cross-target each other.
//!
//! Assertions are on the TRANSCRIPT only, never on the session
//! exit code: `script` itself sits in the PTY foreground group
//! here (rubash does not move itself to a fresh group the way GNU
//! bash does), so after a ^C `script` exits 130 on its own even
//! when the shell exits 0 — the session code is contaminated and
//! must not be asserted.
//!
//! Timing: the ^C test uses a 25s sleep with a 20s session
//! timeout, so a failure to abort (sleep running to completion)
//! trips the timeout instead of passing.
#![cfg(unix)]

use std::io::{Read, Write};
use std::process::{ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

/// Shared transcript: the reader appends chunks as they arrive;
/// the writer polls it for markers.
struct Transcript {
    text: Mutex<String>,
}

impl Transcript {
    fn contains(&self, needle: &str) -> bool {
        self.text.lock().unwrap().contains(needle)
    }

    fn snapshot(&self) -> String {
        self.text.lock().unwrap().clone()
    }
}

fn wait_for(transcript: &Transcript, needle: &str, timeout: Duration) {
    let start = Instant::now();
    while !transcript.contains(needle) {
        if start.elapsed() > timeout {
            panic!(
                "marker {needle:?} never appeared; transcript so far: {:?}",
                transcript.snapshot()
            );
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Find the rubash pid under one `script` session: walk the process
/// tree script -> sh -c -> rubash by ppid. Retried briefly: on a
/// slow host the fork chain may lag the READY marker.
fn find_rubash_pid(script_pid: u32) -> Option<String> {
    let bin = env!("CARGO_BIN_EXE_rubash");
    for _ in 0..50 {
        let out = Command::new("ps").args(["-eo", "pid,ppid,args"]).output().ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        let mut rows: Vec<(u32, u32, String)> = Vec::new();
        for line in text.lines() {
            // NB: split_whitespace, NOT splitn(is_whitespace): ps
            // pads its columns, and splitn does not collapse the
            // run of spaces, so the ppid slot would parse as "" and
            // every row would be skipped.
            let mut parts = line.split_whitespace();
            let (Some(pid), Some(ppid)) = (parts.next(), parts.next()) else {
                continue;
            };
            let args = parts.collect::<Vec<_>>().join(" ");
            if let (Ok(pid), Ok(ppid)) = (pid.parse(), ppid.parse()) {
                rows.push((pid, ppid, args));
            }
        }
        if let Some(sh) = rows.iter().find(|(_, ppid, _)| *ppid == script_pid) {
            if let Some(rubash) = rows.iter().find(|(_, ppid, _)| *ppid == sh.0) {
                if rubash.2 == format!("{bin} -i") {
                    return Some(rubash.0.to_string());
                }
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    None
}

fn write_line(stdin: &mut ChildStdin, line: &str) {
    if stdin.write_all(line.as_bytes()).is_err() {
        return;
    }
    let _ = stdin.flush();
}

/// Drive one PTY session. The writer thread first synchronizes on
/// the shell's READY marker, then runs `drive` (test-specific
/// writes/kill) with stdin, then writes the shared tail
/// (AFTER/DONE/exit). Returns the full transcript.
fn run_session(
    drive: impl FnOnce(&mut ChildStdin, u32) + Send + 'static,
    timeout: Duration,
) -> String {
    if Command::new("script").arg("--version").output().is_err() {
        panic!("no script(1) on this host; cannot run PTY test");
    }
    let cmdline = format!("{} -i", env!("CARGO_BIN_EXE_rubash"));
    let mut child = Command::new("script")
        .args(["-qec", &cmdline, "/dev/null"])
        .env("TERM", "dumb")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn script(1)");
    let script_pid = child.id();

    let transcript = Arc::new(Transcript {
        text: Mutex::new(String::new()),
    });
    let mut stdin = child.stdin.take().expect("child stdin");
    let drive_transcript = Arc::clone(&transcript);
    let writer = std::thread::spawn(move || {
        // The shell echoes its own READY marker once it is actually
        // at a prompt, however slow startup is.
        write_line(&mut stdin, "echo READY1640\n");
        wait_for(&drive_transcript, "READY1640", Duration::from_secs(30));
        drive(&mut stdin, script_pid);
        for line in ["echo AFTER rc=$?\n", "echo DONE1640\n", "exit\n"] {
            write_line(&mut stdin, line);
            std::thread::sleep(Duration::from_secs(1));
        }
    });

    let mut stdout = child.stdout.take().expect("child stdout");
    let reader_transcript = Arc::clone(&transcript);
    let (tx, rx) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut buf = [0u8; 1024];
        loop {
            match stdout.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let chunk = String::from_utf8_lossy(&buf[..n]).into_owned();
                    reader_transcript.text.lock().unwrap().push_str(&chunk);
                }
                Err(_) => break,
            }
        }
        let _ = tx.send(());
    });

    let finished = rx.recv_timeout(timeout).is_ok();
    if !finished {
        let _ = child.kill();
        panic!(
            "PTY session did not finish within {timeout:?}; transcript so far: {:?}",
            transcript.snapshot()
        );
    }
    let _ = child.wait();
    let _ = writer.join();
    let _ = reader.join();
    transcript.snapshot()
}

#[test]
fn issue1640_shell_directed_int_lets_builtin_sleep_complete() {
    // Sleep first, then (2s later, mid-sleep) kill -INT the shell
    // pid only. The sleep must run to completion: AFTER rc=0.
    let transcript = run_session(
        |stdin, script_pid| {
            write_line(stdin, "sleep 5\n");
            std::thread::sleep(Duration::from_secs(2));
            let pid = find_rubash_pid(script_pid).expect("rubash pid in ps");
            let ok = Command::new("sh")
                .args(["-c", &format!("kill -INT {pid}")])
                .status()
                .expect("run kill")
                .success();
            assert!(ok, "kill -INT {pid} failed");
        },
        Duration::from_secs(60),
    );
    assert!(
        transcript.contains("AFTER rc=0"),
        "shell-directed INT must not abort the builtin sleep, transcript: {transcript:?}"
    );
    assert!(
        transcript.contains("DONE1640"),
        "shell must survive and keep reading, transcript: {transcript:?}"
    );
}

#[test]
fn issue1640_terminal_ctrl_c_still_aborts_builtin_sleep() {
    // ^C through the PTY line discipline is a kernel-caused SIGINT
    // to the foreground group: the virtual child must still die
    // with 130 (the #1250 behavior), and the shell must survive.
    // The AFTER line is written by run_session AFTER drive
    // returns, i.e. after the ^C — on purpose: ^C flushes any
    // input buffered ahead of it in the line discipline.
    let transcript = run_session(
        |stdin, _| {
            write_line(stdin, "sleep 25\n");
            std::thread::sleep(Duration::from_secs(2));
            write_line(stdin, "\x03");
            std::thread::sleep(Duration::from_secs(2));
        },
        Duration::from_secs(20),
    );
    assert!(
        transcript.contains("AFTER rc=130"),
        "terminal ^C must still abort the builtin sleep, transcript: {transcript:?}"
    );
    assert!(
        transcript.contains("DONE1640"),
        "shell must survive ^C, transcript: {transcript:?}"
    );
}
