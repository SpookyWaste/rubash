//! Issue rubash#1639: Ctrl-C during a foreground wait runs the rest
//! of the line (`sleep 25; echo AFTER` prints `AFTER rc=130`); GNU
//! discards it (fresh prompt, `$?` = 130 for the NEXT command).
//!
//! GNU's rule (jobs.c:3305-3361): a job-control foreground job killed
//! by SIGINT with SIGINT untrapped sets interrupt_state, so the loop
//! or list breaks and the reader discards the rest of the line. Rubash
//! has no job control; its interactive fg wait is the job-control-like
//! mode (jobs.c wait_for model, rubash#1250), so the same discard is
//! implemented on the existing DISCARD vehicle (ast_exec loop-top
//! abandon check) with a new `sigint_discard_pending` class marker
//! that leaves 130 instead of 1. The trigger is narrow by design:
//!
//! * interactive only — GNU scripts continue after a child SIGINT
//!   death (probed against bash 5.2), so a script-mode child that
//!   kills itself with SIGINT still runs the rest of the line;
//! * untrapped only — a trapped SIGINT runs its action at the
//!   boundary and the list continues;
//! * subshell-bounded — the discard dies with a flat `( )` region
//!   (GNU's forked subshell boundary) and the parent list continues.
//!
//! Harness: the `script(1)` PTY pattern from
//! tests/issue1640_shell_int_builtin_sleep.rs (READY-marker sync so
//! the ^C always lands mid-sleep; transcript assertions only, never
//! the session exit code — `script` itself sits in the PTY
//! foreground group and exits 130 on its own after a ^C).
#![cfg(unix)]

use std::io::{Read, Write};
use std::process::{ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

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

fn write_line(stdin: &mut ChildStdin, line: &str) {
    if stdin.write_all(line.as_bytes()).is_err() {
        return;
    }
    let _ = stdin.flush();
}

/// Drive one PTY session: synchronize on the shell's READY marker,
/// run `drive` (the test's command line + ^C), then write the tail
/// (`NEXT rc=$?`, DONE, exit) as fresh reader commands. Returns the
/// full transcript.
fn run_session(
    drive: impl FnOnce(&mut ChildStdin) + Send + 'static,
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

    let transcript = Arc::new(Transcript {
        text: Mutex::new(String::new()),
    });
    let mut stdin = child.stdin.take().expect("child stdin");
    let drive_transcript = Arc::clone(&transcript);
    let writer = std::thread::spawn(move || {
        write_line(&mut stdin, "echo READY1639\n");
        wait_for(&drive_transcript, "READY1639", Duration::from_secs(30));
        drive(&mut stdin);
        // Written AFTER the ^C on purpose: ^C flushes input buffered
        // ahead of it in the line discipline, so the NEXT probe must
        // be a fresh reader command.
        for line in ["echo NEXT rc=$?\n", "echo DONE1639\n", "exit\n"] {
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

/// One ^C during a sleep: write the line, wait until the sleep is
/// certainly running, send ^C through the PTY, wait for the abort.
fn ctrl_c_during_sleep(stdin: &mut ChildStdin, line: &str) {
    write_line(stdin, line);
    std::thread::sleep(Duration::from_secs(2));
    write_line(stdin, "\x03");
    std::thread::sleep(Duration::from_secs(2));
}

#[test]
fn issue1639_builtin_sleep_ctrl_c_discards_rest_of_line() {
    let transcript = run_session(
        |stdin| ctrl_c_during_sleep(stdin, "sleep 25; echo AFTER rc=$?\n"),
        Duration::from_secs(60),
    );
    // NB: the echoed input line itself contains "AFTER", so assert
    // on the would-be OUTPUT line ("AFTER rc=130"), never the bare word.
    assert!(
        !transcript.contains("AFTER rc=130"),
        "Ctrl-C must discard the rest of the line, transcript: {transcript:?}"
    );
    assert!(
        transcript.contains("NEXT rc=130"),
        "the discard must leave 130 for the next command, transcript: {transcript:?}"
    );
    assert!(
        transcript.contains("DONE1639"),
        "shell must survive and keep reading, transcript: {transcript:?}"
    );
}

#[test]
fn issue1639_external_sleep_ctrl_c_discards_rest_of_line() {
    // Absolute path: a guaranteed external wait, not the builtin
    // fast path.
    let transcript = run_session(
        |stdin| ctrl_c_during_sleep(stdin, "/usr/bin/sleep 25; echo AFTER2 rc=$?\n"),
        Duration::from_secs(60),
    );
    assert!(
        !transcript.contains("AFTER2 rc=130"),
        "Ctrl-C must discard the rest of the line after an external wait, transcript: {transcript:?}"
    );
    assert!(
        transcript.contains("NEXT rc=130"),
        "the discard must leave 130 for the next command, transcript: {transcript:?}"
    );
}

#[test]
fn issue1639_or_fallback_does_not_run_after_ctrl_c() {
    let transcript = run_session(
        |stdin| ctrl_c_during_sleep(stdin, "sleep 25 || echo FALLBACK\n"),
        Duration::from_secs(60),
    );
    // Same input-echo trap: the echoed line contains "echo
    // FALLBACK", so assert on a standalone output line instead.
    assert!(
        !transcript.contains("\r\nFALLBACK\r\n"),
        "Ctrl-C must discard the whole AND-OR list, transcript: {transcript:?}"
    );
    assert!(
        transcript.contains("NEXT rc=130"),
        "the discard must leave 130 for the next command, transcript: {transcript:?}"
    );
}

#[test]
fn issue1639_loop_abandoned_after_ctrl_c() {
    let transcript = run_session(
        |stdin| {
            ctrl_c_during_sleep(
                stdin,
                "for i in 1 2; do sleep 25; echo A$i; done; echo B\n",
            )
        },
        Duration::from_secs(60),
    );
    assert!(
        !transcript.contains("A1"),
        "Ctrl-C must abandon the loop body, transcript: {transcript:?}"
    );
    // A standalone "B" output line — the echoed input line itself
    // contains "echo B" but never a bare B line.
    assert!(
        !transcript.contains("\r\nB\r\n"),
        "Ctrl-C must discard the commands after the loop, transcript: {transcript:?}"
    );
    assert!(
        transcript.contains("NEXT rc=130"),
        "the discard must leave 130 for the next command, transcript: {transcript:?}"
    );
}

#[test]
fn issue1639_trapped_int_runs_trap_and_continues() {
    let transcript = run_session(
        |stdin| {
            ctrl_c_during_sleep(
                stdin,
                "trap 'echo CAUGHT' INT; sleep 25; echo AFTER rc=$?\n",
            )
        },
        Duration::from_secs(60),
    );
    assert!(
        transcript.contains("CAUGHT"),
        "a trapped SIGINT must run its action, transcript: {transcript:?}"
    );
    assert!(
        transcript.contains("AFTER rc=130"),
        "a trapped SIGINT must NOT discard the rest of the line, transcript: {transcript:?}"
    );
}

// NOTE: no PTY test for the `( )` boundary: ^C inside a flat subshell
// region kills the shell today (the fg SIGINT guard arms only at
// subshell_depth == 0), so the region has no fork-death emulation to
// discard within. That is a pre-existing flat-region signal gap for a
// separate issue; the boundary branch in the abandon check mirrors
// the evalerror subshell branch (fast-forward + restore + clear) and
// is structurally covered by that family. The discard itself is
// verified at top level, in AND-OR lists, and in loops above.
#[test]
fn issue1639_read_ctrl_c_discards_rest_of_line() {
    // GNU discards here too (probed against bash 5.2: `read x; echo
    // AFTER` + ^C prints no AFTER, next command sees rc=130). The
    // read blocks immediately, so no sleep timing is involved.
    let transcript = run_session(
        |stdin| {
            write_line(stdin, "read x; echo AFTER\n");
            std::thread::sleep(Duration::from_secs(2));
            write_line(stdin, "\x03");
            std::thread::sleep(Duration::from_secs(2));
        },
        Duration::from_secs(60),
    );
    assert!(
        !transcript.contains("\r\nAFTER\r\n"),
        "Ctrl-C must discard the rest of the line after an interrupted read, transcript: {transcript:?}"
    );
    assert!(
        transcript.contains("NEXT rc=130"),
        "the discard must leave 130 for the next command, transcript: {transcript:?}"
    );
}

#[test]
fn issue1639_eval_single_line_ctrl_c_discards_all() {
    // The discard propagates through the eval boundary to the
    // caller: neither the eval'd tail nor the caller's tail runs.
    // Standalone-line assertions again (the echoed input contains
    // every one of these words).
    let transcript = run_session(
        |stdin| ctrl_c_during_sleep(stdin, "eval 'sleep 25; echo IN'; echo OUT\n"),
        Duration::from_secs(60),
    );
    assert!(
        !transcript.contains("\r\nIN\r\n"),
        "Ctrl-C must discard the eval'd tail, transcript: {transcript:?}"
    );
    assert!(
        !transcript.contains("\r\nOUT\r\n"),
        "Ctrl-C must discard the caller's tail after eval, transcript: {transcript:?}"
    );
    assert!(
        transcript.contains("NEXT rc=130"),
        "the discard must leave 130 for the next command, transcript: {transcript:?}"
    );
}

#[test]
fn issue1639_eval_multiline_ctrl_c_abandons_whole_string() {
    // Unlike evalerror DISCARD (per-line skip, rubash#637), a SIGINT
    // discard abandons the whole eval'd string: GNU's interrupt_state
    // breaks the entire parse_and_execute loop. The drive writes the
    // eval command as two physical lines (the quote spans them, so
    // the shell shows a continuation prompt for the second).
    let transcript = run_session(
        |stdin| {
            write_line(stdin, "eval 'sleep 25; echo IN1\n");
            write_line(stdin, "echo IN2'; echo OUT\n");
            std::thread::sleep(Duration::from_secs(2));
            write_line(stdin, "\x03");
            std::thread::sleep(Duration::from_secs(2));
        },
        Duration::from_secs(60),
    );
    assert!(
        !transcript.contains("\r\nIN1\r\n"),
        "Ctrl-C must discard the eval'd first line tail, transcript: {transcript:?}"
    );
    assert!(
        !transcript.contains("\r\nIN2\r\n"),
        "Ctrl-C must abandon the eval'd later lines too, transcript: {transcript:?}"
    );
    assert!(
        !transcript.contains("\r\nOUT\r\n"),
        "Ctrl-C must discard the caller's tail after eval, transcript: {transcript:?}"
    );
    assert!(
        transcript.contains("NEXT rc=130"),
        "the discard must leave 130 for the next command, transcript: {transcript:?}"
    );
}

#[test]
fn issue1639_script_mode_child_sigint_death_continues() {
    // Non-interactive: a child that kills only itself with SIGINT
    // leaves 130 and the script continues (GNU scripts do the same).
    // No PTY needed — the shell never receives the signal here.
    let dir = std::env::temp_dir().join(format!("rubash1639_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("tempdir");
    let script = dir.join("s.sh");
    std::fs::write(
        &script,
        "/bin/sh -c 'kill -INT $$'; echo AFTER rc=$?\necho DONE\n",
    )
    .expect("write script");
    let out = Command::new(env!("CARGO_BIN_EXE_rubash"))
        .arg(&script)
        .output()
        .expect("run script");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        stdout.contains("AFTER rc=130"),
        "script mode must continue after a child SIGINT death, stdout: {stdout:?}"
    );
    assert!(
        stdout.contains("DONE"),
        "script must run to completion, stdout: {stdout:?}"
    );
}
