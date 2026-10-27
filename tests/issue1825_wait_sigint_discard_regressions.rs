//! rubash#1825 — `wait` interrupted by an UNTRAPPED Ctrl-C must end
//! the wait, discard the rest of the line, and leave the interactive
//! session alive with $?=130 — not end the session.
//!
//! GNU ground truth (verified against GNU bash 5.3.20 under the same
//! script(1) PTY discipline): wait.def's pending-trap precheck makes
//! wait return 128+SIGINT at once, and jobs.c:3315-3316 ADDINTERRUPT
//! throws to the top level as DISCARD in an interactive shell, so
//! the session survives. rubash's wait_pending_signal_status drained
//! the delivery but returned a bare ExitCode(128+sig) with no discard
//! mark, which the source driver reads as an exit jump — the whole
//! session ended with status 130.
//!
//! Shape A keeps the background job alive across the ^C (it ignores
//! SIGINT), so the ONLY thing that can end the wait promptly is the
//! shell's own delivery classification — no child-death race. The
//! trapped control (shape B) pins that a trapped wait still runs its
//! trap and CONTINUES the line with rc=130.

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
    // Handshake on a computed marker (the tty echo of the input line
    // cannot contain it): the shell is up and executing lines.
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

#[test]
fn issue1825_wait_untrapped_ctrl_c_discards_line_and_session_survives() {
    if !script_available() {
        return;
    }
    let mut session = spawn_session();
    // The background job ignores SIGINT and keeps sleeping, so only
    // the shell's own untrapped-SIGINT verdict can end the wait.
    session.send_line("(trap '' INT; sleep 30) &");
    session.pump(Duration::from_millis(600));
    session.send_line("wait; echo AFTER1825");
    session.pump(Duration::from_millis(1200));
    session.send_ctrl_c();
    session.pump(Duration::from_millis(1200));
    session.send_line("echo NEXT_$?");
    // If the session died at the ^C (the pre-fix behavior), script(1)
    // exits with it and NEXT_130 never appears.
    session.wait_for("NEXT_130", Duration::from_secs(15));
    // Output-side assertion, same discipline as the #1639 pins: strip
    // the typed line's own echo (first occurrence) before checking
    // the forbidden marker.
    assert!(
        session.transcript.contains("wait; echo AFTER1825"),
        "typed line's echo not found in transcript; transcript: {:?}",
        session.transcript
    );
    let output_side = session.transcript.replacen("wait; echo AFTER1825", "", 1);
    assert!(
        !output_side.contains("AFTER1825"),
        "AFTER1825 ran despite the Ctrl-C discard; transcript: {:?}",
        session.transcript
    );
    session.finish();
}

#[test]
fn issue1825_wait_trapped_ctrl_c_runs_trap_and_continues_line() {
    if !script_available() {
        return;
    }
    let mut session = spawn_session();
    session.send_line("trap 'echo TRAPPED1825' INT");
    session.pump(Duration::from_millis(400));
    session.send_line("sleep 30 &");
    session.pump(Duration::from_millis(600));
    session.send_line("wait; echo AFTER1825 rc=$?");
    session.pump(Duration::from_millis(1200));
    session.send_ctrl_c();
    // The trap fires as the interrupted wait ends; the list then
    // CONTINUES (no discard for a trapped SIGINT).
    session.wait_for("AFTER1825 rc=130", Duration::from_secs(15));
    assert!(
        session.transcript.contains("TRAPPED1825"),
        "transcript: {:?}",
        session.transcript
    );
    session.send_line("echo NEXT_$?");
    session.wait_for("NEXT_0", Duration::from_secs(10));
    session.finish();
}
