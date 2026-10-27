//! rubash#1639 follow-up shapes — the untrapped-SIGINT line discard
//! for the remaining foreground-wait producers and frames, driven
//! through the same `script(1)` PTY carrier as
//! issue1639_1640_interactive_sigint_regressions.rs.
//!
//! - `read`: a Ctrl-C that interrupts a blocking `read` is an
//!   untrapped-SIGINT foreground wait (GNU: the interrupted read
//!   returns 128+SIGINT and the executing list throws to the top
//!   level, discarding the rest of the line). The read producer arm
//!   was ported from PR #1702 (a-rookie-of-C-language), whose
//!   implementation rode the evalerror DISCARD vehicle; here it
//!   marks the slice's `fg_sigint_unwind` at the three points where
//!   an interrupted status is actually returned.
//! - loop / eval frames: the SigintUnwind error must cross loop and
//!   eval frames exactly like the function/subshell frames.
//!
//! Expectations verified against GNU bash 5.3.20 under the same PTY
//! discipline (script(1) carrier, ^C byte to the foreground group):
//! every shape discards the rest of the line and leaves NEXT_130.

#![cfg(unix)]

use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

struct PtySession {
    child: Child,
    stdin: std::process::ChildStdin,
    rx: mpsc::Receiver<Vec<u8>>,
    transcript: String,
}

fn spawn_session() -> PtySession {
    let cmdline = format!("{} -i", env!("CARGO_BIN_EXE_rubash"));
    let mut child = Command::new("script")
        .args(["-qec", &cmdline, "/dev/null"])
        .env("TERM", "xterm")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn script(1)");
    let stdin = child.stdin.take().expect("child stdin");
    let mut stdout = child.stdout.take().expect("child stdout");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match stdout.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });
    let mut session = PtySession {
        child,
        stdin,
        rx,
        transcript: String::new(),
    };
    session.send_line("echo READY_$((30+12))");
    session.wait_for("READY_42", Duration::from_secs(30));
    session.transcript.clear();
    session
}

impl PtySession {
    fn pump(&mut self, dur: Duration) {
        let deadline = Instant::now() + dur;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return;
            }
            match self.rx.recv_timeout(left) {
                Ok(chunk) => {
                    self.transcript.push_str(&String::from_utf8_lossy(&chunk));
                }
                Err(_) => return,
            }
        }
    }

    fn wait_for(&mut self, marker: &str, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        while !self.transcript.contains(marker) {
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(
                !left.is_zero(),
                "timed out waiting for {marker:?}; transcript: {:?}",
                self.transcript
            );
            match self.rx.recv_timeout(left) {
                Ok(chunk) => {
                    self.transcript.push_str(&String::from_utf8_lossy(&chunk));
                }
                Err(_) => break,
            }
        }
        assert!(
            self.transcript.contains(marker),
            "marker {marker:?} never appeared; transcript: {:?}",
            self.transcript
        );
    }

    fn send_line(&mut self, line: &str) {
        let _ = self.stdin.write_all(line.as_bytes());
        let _ = self.stdin.write_all(b"\n");
        let _ = self.stdin.flush();
    }

    fn send_ctrl_c(&mut self) {
        let _ = self.stdin.write_all(b"\x03");
        let _ = self.stdin.flush();
    }

    fn finish(mut self) {
        self.send_line("exit");
        self.pump(Duration::from_secs(2));
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn script_available() -> bool {
    Command::new("script").arg("--version").output().is_ok()
}

fn ctrl_c_discards(line: &str, forbidden: &[&str]) {
    let mut session = spawn_session();
    session.send_line(line);
    session.pump(Duration::from_millis(1500));
    session.send_ctrl_c();
    session.pump(Duration::from_millis(1200));
    session.send_line("echo NEXT_$?");
    session.wait_for("NEXT_130", Duration::from_secs(15));
    // Assert on the OUTPUT side of the echo, not the raw transcript
    // (GATE-READJUDICATION B): the PTY necessarily echoes the typed
    // line, and every forbidden marker is part of that line's text,
    // so a bare `!transcript.contains(marker)` is false for ANY
    // product behavior. The line's own echo (its first occurrence)
    // is stripped; a marker that actually ran would print again in
    // what remains, so the check keeps its teeth. If the echo is not
    // found verbatim, fail loudly instead of passing on an
    // unstripped transcript.
    assert!(
        session.transcript.contains(line),
        "typed line's echo not found in transcript; transcript: {:?}",
        session.transcript
    );
    let output_side = session.transcript.replacen(line, "", 1);
    for marker in forbidden {
        assert!(
            !output_side.contains(marker),
            "{marker:?} ran despite the Ctrl-C discard; transcript: {:?}",
            session.transcript
        );
    }
    session.finish();
}

#[test]
fn issue1639_read_ctrl_c_discards_rest_of_line() {
    if !script_available() {
        return;
    }
    ctrl_c_discards("read x; echo AFTER_READ", &["AFTER_READ"]);
}

#[test]
fn issue1639_loop_abandoned_after_ctrl_c() {
    if !script_available() {
        return;
    }
    ctrl_c_discards(
        "while true; do sleep 6; echo IN_LOOP; done; echo AFTER_LOOP",
        &["IN_LOOP", "AFTER_LOOP"],
    );
}

#[test]
fn issue1639_eval_ctrl_c_abandons_whole_string() {
    if !script_available() {
        return;
    }
    ctrl_c_discards(
        "eval 'sleep 6; echo IN_EVAL'; echo AFTER_EVAL",
        &["IN_EVAL", "AFTER_EVAL"],
    );
}
