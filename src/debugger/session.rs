//! The debug session state machine (debugger design sections 3/4;
//! WP-1 gap register G-01/G-07/G-08).
//!
//! The session is the only semantic owner of the debugger: frontends
//! translate their input into session operations and render session
//! state back; neither contains stop logic. This type is that
//! owner's skeleton — the state machine, the breakpoint table, the
//! pending event queue, and the two flags (entry stop, asynchronous
//! stop) — with no consult wiring yet: nothing in the executor asks
//! it anything, and with the gate disarmed the whole module is inert.
//!
//! State shape:
//!
//! ```text
//!            new()
//!              │
//!              ▼
//!          Stopped(entry) ──resume──▶ Running ──stop──▶ Stopped
//!              │                        │  ▲                 │
//!              └────── exit / die ──────┴──┴──── exit / die ─┘
//!                                   ▼
//!                              Terminated
//! ```
//!
//! The session starts stopped at the entry stop: the debuggee has
//! not executed anything, so setup work (setting breakpoints) and
//! the first resume both happen from that stop, exactly as the
//! design's Q1 ruling describes for both frontends.

use std::collections::VecDeque;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};

use super::breakpoints::BreakpointTable;
use super::event::{DebugEvent, StopInfo};
use super::gate::SESSION_ACTIVE;
use crate::api::views::InspectionSnapshot;

/// The three states of the design's session model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Parked at a stop; the stop loop owns the executor thread.
    Stopped,
    /// The debuggee is executing under an installed arming.
    Running,
    /// The debuggee is gone; the session only reports.
    Terminated,
}

/// How a terminated session ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Termination {
    /// The debuggee walked its exit path with this final status.
    Exited {
        /// Final exit status.
        status: i32,
    },
    /// An untrapped fatal signal killed the debuggee.
    Killed {
        /// The killing signal number.
        signal: i32,
    },
}

impl Termination {
    /// The shell-conventional status of the termination: the exit
    /// status itself, or 128 + signal for a signal death.
    pub fn status(&self) -> i32 {
        match *self {
            Termination::Exited { status } => status,
            Termination::Killed { signal } => 128 + signal,
        }
    }
}

/// The arming a resume installs (design section 6: stepping is
/// arming).
///
/// A resume decision installs a one-shot rule — *stop at the next
/// admitted seam satisfying the rule* — and the state machine only
/// records which rule is in force. The predicates that evaluate a
/// rule against a live seam belong to the stepping work package;
/// the data lives here because the session is the rule's owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepArming {
    /// No rule: run until a breakpoint, a watch change, an async
    /// stop, or the end.
    Continue,
    /// Stop at the next admitted seam, in whatever frame.
    StepInto,
    /// Stop at the next admitted seam no deeper than `max_depth`.
    StepOver {
        /// The frame depth the resume started from.
        max_depth: usize,
    },
    /// Stop when the frame at `frame_depth` returns, in its caller.
    Finish {
        /// The depth of the frame being finished.
        frame_depth: usize,
    },
}

/// A rejected state transition. The machine is small, so a rejected
/// transition always means a caller bug; it is reported, never
/// papered over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionError {
    /// `resume` requires the stopped state.
    NotStopped,
    /// `stop` requires the running state.
    NotRunning,
    /// The session has terminated; no transition is legal.
    AlreadyTerminated,
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            SessionError::NotStopped => "session is not stopped",
            SessionError::NotRunning => "session is not running",
            SessionError::AlreadyTerminated => "session has terminated",
        };
        f.write_str(text)
    }
}

impl std::error::Error for SessionError {}

/// The debug session core.
///
/// One session serves one debuggee on its owning thread. The
/// asynchronous stop flag is the single field another context (the
/// boundary poll's signal path, in a later work package) touches,
/// so it is atomic; everything else is plain data under the
/// single-threaded contract.
#[derive(Debug)]
pub struct DebugSession {
    state: SessionState,
    last_stop: Option<StopInfo>,
    arming: StepArming,
    breakpoints: BreakpointTable,
    async_stop: AtomicBool,
    termination: Option<Termination>,
    pending_events: VecDeque<DebugEvent>,
    inspection: Option<InspectionSnapshot>,
    /// Breakpoint resolution knowledge (design 5.1/5.2; WP-8):
    /// the syntax view of every file the engine has parsed a unit
    /// of, keyed by the engine's own file identity. The
    /// adjudication logic lives in `super::resolution`; the fields
    /// live here because the session is the knowledge's owner.
    pub(crate) syntax_views: std::collections::HashMap<String, crate::api::syntax::SyntaxView>,
    /// Function names a runtime definition note has proven defined
    /// (names no syntax view covers — `eval`-defined functions).
    pub(crate) defined_functions: std::collections::HashSet<String>,
}

impl DebugSession {
    /// A new session, stopped at the entry stop.
    ///
    /// Construction queues [`DebugEvent::SessionStarted`] and the
    /// entry [`DebugEvent::Stopped`] (DE-01 includes the entry stop)
    /// for the frontend to drain. Construction does **not** arm the
    /// process gate: activation is the separate, explicit
    /// [`DebugSession::activate`] step, so merely building a session
    /// (or a test) never changes engine behavior.
    pub fn new() -> Self {
        let entry_stop = StopInfo::entry();
        let mut session = Self {
            state: SessionState::Stopped,
            last_stop: Some(entry_stop.clone()),
            arming: StepArming::Continue,
            breakpoints: BreakpointTable::new(),
            async_stop: AtomicBool::new(false),
            termination: None,
            pending_events: VecDeque::new(),
            inspection: None,
            syntax_views: std::collections::HashMap::new(),
            defined_functions: std::collections::HashSet::new(),
        };
        session.emit(DebugEvent::SessionStarted);
        session.emit(DebugEvent::Stopped(entry_stop));
        session
    }

    /// Arm the process gate (design 4.4): from here on, seam
    /// admission may consult the session.
    ///
    /// Gate pairing is the session driver's job, deliberately kept
    /// out of the state transitions so the machine stays pure (and
    /// unit-testable without touching process state): whoever
    /// activates a session deactivates it once the session has
    /// terminated — a terminated session must never leave the engine
    /// consulting it.
    pub fn activate(&self) {
        SESSION_ACTIVE.arm();
    }

    /// Disarm the process gate: the debugger-off path is again one
    /// atomic load.
    pub fn deactivate(&self) {
        SESSION_ACTIVE.disarm();
    }

    /// The current state.
    pub fn state(&self) -> SessionState {
        self.state
    }

    /// Whether the debuggee is executing.
    pub fn is_running(&self) -> bool {
        self.state == SessionState::Running
    }

    /// Whether the session is parked at a stop.
    pub fn is_stopped(&self) -> bool {
        self.state == SessionState::Stopped
    }

    /// Whether the debuggee is gone.
    pub fn is_terminated(&self) -> bool {
        self.state == SessionState::Terminated
    }

    /// The most recent stop, if any (the entry stop counts).
    pub fn last_stop(&self) -> Option<&StopInfo> {
        self.last_stop.as_ref()
    }

    /// The inspection snapshot captured for the current stop
    /// (PLAN row 6 / WP-6). The executor captures it from the live
    /// engine state at the moment of the stop and installs it here
    /// before the stop loop runs; frontends only render it. It is a
    /// point-in-time capture, not a live view: after a resume it is
    /// stale by definition until the next stop replaces it.
    pub fn inspection(&self) -> Option<&InspectionSnapshot> {
        self.inspection.as_ref()
    }

    /// Install the inspection snapshot for the stop being served.
    /// Consult-side only: frontends read through
    /// [`DebugSession::inspection`].
    pub fn set_inspection(&mut self, inspection: InspectionSnapshot) {
        self.inspection = Some(inspection);
    }

    /// The arming installed by the most recent resume.
    pub fn arming(&self) -> StepArming {
        self.arming
    }

    /// How the session ended, once terminated.
    pub fn termination(&self) -> Option<Termination> {
        self.termination
    }

    /// The session's breakpoint table.
    pub fn breakpoints(&self) -> &BreakpointTable {
        &self.breakpoints
    }

    /// The session's breakpoint table, mutably.
    pub fn breakpoints_mut(&mut self) -> &mut BreakpointTable {
        &mut self.breakpoints
    }

    /// Queue an event for the frontend. Events raised by state
    /// transitions are queued by the transition methods themselves;
    /// this entry point is for the consult-side producers (frame
    /// returns, watch changes, resolution updates, output).
    pub fn emit(&mut self, event: DebugEvent) {
        self.pending_events.push_back(event);
    }

    /// Take all queued events, oldest first.
    pub fn drain_events(&mut self) -> Vec<DebugEvent> {
        self.pending_events.drain(..).collect()
    }

    /// Number of events waiting for the frontend.
    pub fn pending_event_count(&self) -> usize {
        self.pending_events.len()
    }

    /// Resume the debuggee under an arming (design section 6).
    ///
    /// Legal only from the stopped state — including the entry
    /// stop, which is how a session starts its debuggee.
    pub fn resume(&mut self, arming: StepArming) -> Result<(), SessionError> {
        match self.state {
            SessionState::Terminated => Err(SessionError::AlreadyTerminated),
            SessionState::Running => Err(SessionError::NotStopped),
            SessionState::Stopped => {
                self.state = SessionState::Running;
                self.arming = arming;
                Ok(())
            }
        }
    }

    /// Park the session at a stop decided by the consult.
    ///
    /// Legal only from the running state. Installs the stop record,
    /// clears the arming (an arming is consumed by the stop it
    /// produced), and queues the [`DebugEvent::Stopped`] event.
    pub fn stop(&mut self, info: StopInfo) -> Result<(), SessionError> {
        match self.state {
            SessionState::Terminated => Err(SessionError::AlreadyTerminated),
            SessionState::Stopped => Err(SessionError::NotRunning),
            SessionState::Running => {
                self.state = SessionState::Stopped;
                self.arming = StepArming::Continue;
                self.last_stop = Some(info.clone());
                self.emit(DebugEvent::Stopped(info));
                Ok(())
            }
        }
    }

    /// End the session: the debuggee walked its exit path.
    ///
    /// Queues [`DebugEvent::SessionExited`]. The driver deactivates
    /// the gate on termination (see [`DebugSession::activate`]).
    pub fn exit(&mut self, status: i32) -> Result<(), SessionError> {
        if self.state == SessionState::Terminated {
            return Err(SessionError::AlreadyTerminated);
        }
        self.state = SessionState::Terminated;
        self.termination = Some(Termination::Exited { status });
        self.emit(DebugEvent::SessionExited { status });
        Ok(())
    }

    /// End the session: an untrapped fatal signal killed the
    /// debuggee (DE-08). Queues [`DebugEvent::DebuggeeDied`].
    pub fn die(&mut self, signal: i32) -> Result<(), SessionError> {
        if self.state == SessionState::Terminated {
            return Err(SessionError::AlreadyTerminated);
        }
        self.state = SessionState::Terminated;
        self.termination = Some(Termination::Killed { signal });
        self.emit(DebugEvent::DebuggeeDied { signal });
        Ok(())
    }

    /// Request an asynchronous stop (design 4.4/9.4).
    ///
    /// Callable from the signal-side context: it only sets the
    /// flag. The boundary poll services it — after dispatching any
    /// signal trap — through [`DebugSession::take_async_stop`] and a
    /// [`DebugEvent::AsyncStopServiced`] event.
    pub fn request_async_stop(&self) {
        self.async_stop.store(true, Ordering::Release);
    }

    /// Whether an asynchronous stop is waiting to be serviced.
    pub fn async_stop_requested(&self) -> bool {
        self.async_stop.load(Ordering::Acquire)
    }

    /// Consume the asynchronous stop request: true exactly once per
    /// request batch.
    pub fn take_async_stop(&mut self) -> bool {
        self.async_stop.swap(false, Ordering::AcqRel)
    }
}

impl Default for DebugSession {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{DebugSession, SessionError, SessionState, StepArming, Termination};
    use crate::debugger::event::{DebugEvent, SourceLocation, StopInfo, StopReason};

    fn seam_stop() -> StopInfo {
        StopInfo {
            reason: StopReason::Step,
            location: Some(SourceLocation::new("a.sh", 4)),
            seam: None,
            frame_depth: 0,
            command_text: Some("echo next".to_string()),
        }
    }

    #[test]
    fn a_new_session_is_stopped_at_the_entry_stop() {
        let session = DebugSession::new();
        assert_eq!(session.state(), SessionState::Stopped);
        assert!(session.is_stopped());
        let stop = session.last_stop().expect("entry stop recorded");
        assert_eq!(stop.reason, StopReason::Entry);
        assert_eq!(session.termination(), None);
    }

    #[test]
    fn a_new_session_queues_started_then_the_entry_stop() {
        let mut session = DebugSession::new();
        let events = session.drain_events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], DebugEvent::SessionStarted);
        assert_eq!(events[0].catalogue_id(), "DE-01");
        match &events[1] {
            DebugEvent::Stopped(info) => assert_eq!(info.reason, StopReason::Entry),
            other => panic!("second event must be the entry stop, got {other:?}"),
        }
        assert_eq!(session.pending_event_count(), 0);
        assert!(session.drain_events().is_empty(), "draining is final");
    }

    #[test]
    fn resume_from_the_entry_stop_starts_the_debuggee() {
        let mut session = DebugSession::new();
        assert_eq!(session.resume(StepArming::StepInto), Ok(()));
        assert!(session.is_running());
        assert_eq!(session.arming(), StepArming::StepInto);
    }

    #[test]
    fn resume_while_running_is_rejected() {
        let mut session = DebugSession::new();
        session.resume(StepArming::Continue).expect("first resume");
        assert_eq!(
            session.resume(StepArming::Continue),
            Err(SessionError::NotStopped)
        );
    }

    #[test]
    fn stop_parks_the_session_and_records_the_stop() {
        let mut session = DebugSession::new();
        session
            .resume(StepArming::StepOver { max_depth: 2 })
            .unwrap();
        let info = seam_stop();
        assert_eq!(session.stop(info.clone()), Ok(()));
        assert!(session.is_stopped());
        assert_eq!(session.last_stop(), Some(&info));
        assert_eq!(
            session.arming(),
            StepArming::Continue,
            "the arming is consumed by the stop it produced"
        );
        let events = session.drain_events();
        assert_eq!(events.last(), Some(&DebugEvent::Stopped(info)));
    }

    #[test]
    fn stop_while_stopped_is_rejected() {
        let mut session = DebugSession::new();
        assert_eq!(session.stop(seam_stop()), Err(SessionError::NotRunning));
    }

    #[test]
    fn exit_from_running_terminates_with_the_status() {
        let mut session = DebugSession::new();
        session.resume(StepArming::Continue).unwrap();
        assert_eq!(session.exit(3), Ok(()));
        assert!(session.is_terminated());
        assert_eq!(
            session.termination(),
            Some(Termination::Exited { status: 3 })
        );
        let events = session.drain_events();
        assert_eq!(
            events.last(),
            Some(&DebugEvent::SessionExited { status: 3 })
        );
    }

    #[test]
    fn exit_from_the_entry_stop_is_legal() {
        let mut session = DebugSession::new();
        assert_eq!(session.exit(0), Ok(()));
        assert!(session.is_terminated());
    }

    #[test]
    fn death_by_signal_reports_128_plus_signal() {
        let mut session = DebugSession::new();
        session.resume(StepArming::Continue).unwrap();
        assert_eq!(session.die(11), Ok(()));
        assert_eq!(
            session.termination(),
            Some(Termination::Killed { signal: 11 })
        );
        assert_eq!(
            session.termination().expect("termination").status(),
            128 + 11
        );
        let events = session.drain_events();
        assert_eq!(
            events.last(),
            Some(&DebugEvent::DebuggeeDied { signal: 11 })
        );
    }

    #[test]
    fn terminated_sessions_reject_every_transition() {
        let mut session = DebugSession::new();
        session.exit(0).unwrap();
        assert_eq!(
            session.resume(StepArming::Continue),
            Err(SessionError::AlreadyTerminated)
        );
        assert_eq!(
            session.stop(seam_stop()),
            Err(SessionError::AlreadyTerminated)
        );
        assert_eq!(session.exit(1), Err(SessionError::AlreadyTerminated));
        assert_eq!(session.die(9), Err(SessionError::AlreadyTerminated));
    }

    #[test]
    fn async_stop_is_consumed_exactly_once() {
        let mut session = DebugSession::new();
        assert!(!session.async_stop_requested());
        assert!(!session.take_async_stop());
        session.request_async_stop();
        assert!(session.async_stop_requested());
        assert!(session.take_async_stop());
        assert!(!session.async_stop_requested());
        assert!(!session.take_async_stop(), "the request was consumed");
    }

    #[test]
    fn the_breakpoint_table_is_reachable_through_the_session() {
        let mut session = DebugSession::new();
        let id = session.breakpoints_mut().insert_line("a.sh", 9);
        assert_eq!(session.breakpoints().len(), 1);
        assert_eq!(session.breakpoints().get(id).expect("entry").hit_count, 0);
    }

    #[test]
    fn error_display_is_human_text() {
        assert_eq!(
            SessionError::AlreadyTerminated.to_string(),
            "session has terminated"
        );
    }
}
