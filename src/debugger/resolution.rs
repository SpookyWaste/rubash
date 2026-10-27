//! Breakpoint resolution (debugger design sections 5.1/5.2; WP-8).
//!
//! Resolution answers one question per entry — *can this
//! breakpoint fire?* — from the syntax views of the files the
//! engine has parsed so far ([`crate::api::syntax`]) plus the set
//! of function names the runtime has been seen to define. The
//! verdicts are the design's three states:
//!
//! - **Verified**: the target exists — a line the seam can stop on,
//!   or a function whose definition has appeared.
//! - **Unverified, with reason**: the target provably cannot fire.
//!   The two reason strings are the design's own (section 5.1):
//!   "no command on this line" and "DEBUG trap never fires on this
//!   line". A dead line is reported, never silently accepted.
//! - **Pending**: the question cannot be answered yet — the file
//!   has not been parsed, or the function has not been defined.
//!   Pending is the state of a function breakpoint set before its
//!   definition appears (section 5.2); it resolves the moment the
//!   definition shows up, in a parsed unit's syntax view or at the
//!   function's entry seam.
//!
//! Timing rules, chosen so a frontend always learns the truth
//! exactly once:
//!
//! - An entry inserted through [`DebugSession::set_line_breakpoint`]
//!   / [`DebugSession::set_function_breakpoint`] is adjudicated
//!   synchronously at insertion, against the views already
//!   registered. No event is emitted for the insertion verdict —
//!   the caller is holding the new entry and reads its state
//!   directly.
//! - Every *later* change — a view registration that proves or
//!   kills a pending entry, a runtime definition note — goes
//!   through [`DebugSession::apply_resolution`], which emits
//!   [`DebugEvent::BreakpointResolved`] (DE-11) exactly when the
//!   state actually changes.
//!
//! Resolution is reporting, not gating: an unverified or pending
//! entry still occupies its table slot, and the consult's hit
//! accounting does not consult resolution state. Views only grow
//! (a file's parsed units accumulate), so verdicts move
//! monotonically toward verified — except that a line first judged
//! "no command" can be re-judged if a later unit of the same file
//! attributes a command to it, which re-adjudication handles by
//! simply recomputing from the grown view.

use super::breakpoints::{BreakpointId, BreakpointResolution, BreakpointTarget};
use super::event::DebugEvent;
use super::session::DebugSession;
use crate::api::syntax::{LineClass, SyntaxView};

/// The design's verdict for a line that carries no command
/// (section 5.1).
pub(crate) const REASON_NO_COMMAND: &str = "no command on this line";
/// The design's verdict for a line whose command the seam never
/// fires at (section 5.1).
pub(crate) const REASON_NEVER_FIRES: &str = "DEBUG trap never fires on this line";

impl DebugSession {
    /// Set a line breakpoint (design 5.1), adjudicated immediately
    /// against the syntax views registered so far: verified for a
    /// stoppable line, unverified with the design's reason for a
    /// dead one, pending when the file has not been parsed yet.
    pub fn set_line_breakpoint(&mut self, file: impl Into<String>, line: u32) -> BreakpointId {
        let file = file.into();
        let id = self.breakpoints_mut().insert_line(&file, line);
        let verdict = self.line_verdict(&file, line);
        self.breakpoints_mut().set_resolution(id, verdict);
        id
    }

    /// Set a function breakpoint (design 5.2). A name already
    /// proven defined — by a registered syntax view or by a runtime
    /// definition note — verifies immediately; any other name is
    /// pending, never an error, and resolves when the definition
    /// appears.
    pub fn set_function_breakpoint(&mut self, name: impl Into<String>) -> BreakpointId {
        let name = name.into();
        let id = self.breakpoints_mut().insert_function(&name);
        if self.function_is_defined(&name) {
            self.breakpoints_mut()
                .set_resolution(id, BreakpointResolution::Verified);
        }
        id
    }

    /// Register the syntax view of one parsed reader unit (built
    /// by the executor at the two file-backed group sites) and
    /// re-adjudicate every entry the grown knowledge touches: all
    /// line entries for the view's file, and all function entries
    /// (any file's view may prove a name). Changes emit DE-11.
    pub fn register_syntax_view(&mut self, view: SyntaxView) {
        let file = view.file.clone();
        match self.syntax_views.get_mut(&file) {
            Some(existing) => existing.merge(view),
            None => {
                self.syntax_views.insert(file.clone(), view);
            }
        }
        let line_ids: Vec<BreakpointId> = self
            .breakpoints()
            .iter()
            .filter(|entry| {
                matches!(
                    &entry.target,
                    BreakpointTarget::Line { file: f, .. } if f == &file
                )
            })
            .map(|entry| entry.id)
            .collect();
        for id in line_ids {
            if let Some(entry) = self.breakpoints().get(id) {
                if let BreakpointTarget::Line { file, line } = &entry.target {
                    let verdict = self.line_verdict(file, *line);
                    self.apply_resolution(id, verdict);
                }
            }
        }
        let function_ids: Vec<BreakpointId> = self
            .breakpoints()
            .iter()
            .filter(|entry| matches!(entry.target, BreakpointTarget::Function { .. }))
            .map(|entry| entry.id)
            .collect();
        for id in function_ids {
            self.readjudicate_function(id);
        }
    }

    /// Note that a function definition has been observed at
    /// runtime (the entry seam consult calls this for the function
    /// being entered: a name with no syntax view — defined by
    /// `eval`, say — is proven defined by its first entry).
    /// Pending entries for the name verify, with DE-11.
    pub fn note_function_defined(&mut self, name: &str) {
        if !self.defined_functions.insert(name.to_string()) {
            return;
        }
        let ids: Vec<BreakpointId> = self
            .breakpoints()
            .iter()
            .filter(|entry| {
                matches!(
                    &entry.target,
                    BreakpointTarget::Function { name: n } if n == name
                )
            })
            .map(|entry| entry.id)
            .collect();
        for id in ids {
            self.readjudicate_function(id);
        }
    }

    /// Whether any registered view or runtime note proves `name`
    /// is defined.
    fn function_is_defined(&self, name: &str) -> bool {
        self.defined_functions.contains(name)
            || self
                .syntax_views
                .values()
                .any(|view| view.function_site(name).is_some())
    }

    /// The verdict for one line of one file, from the views alone.
    fn line_verdict(&self, file: &str, line: u32) -> BreakpointResolution {
        let Some(view) = self.syntax_views.get(file) else {
            return BreakpointResolution::Pending;
        };
        match view.classify_line(line) {
            LineClass::Stoppable => BreakpointResolution::Verified,
            LineClass::CommandOnly => BreakpointResolution::Unverified {
                reason: REASON_NEVER_FIRES.to_string(),
            },
            LineClass::NoCommand => BreakpointResolution::Unverified {
                reason: REASON_NO_COMMAND.to_string(),
            },
        }
    }

    /// Recompute one function entry's verdict and apply it.
    fn readjudicate_function(&mut self, id: BreakpointId) {
        let verdict = match self.breakpoints().get(id).map(|entry| &entry.target) {
            Some(BreakpointTarget::Function { name }) => {
                if self.function_is_defined(name) {
                    BreakpointResolution::Verified
                } else {
                    BreakpointResolution::Pending
                }
            }
            _ => return,
        };
        self.apply_resolution(id, verdict);
    }

    /// Install a verdict, emitting DE-11 when — and only when —
    /// the state actually changes.
    fn apply_resolution(&mut self, id: BreakpointId, verdict: BreakpointResolution) {
        let changed = self
            .breakpoints()
            .get(id)
            .is_some_and(|entry| entry.resolution != verdict);
        if changed {
            self.breakpoints_mut().set_resolution(id, verdict.clone());
            self.emit(DebugEvent::BreakpointResolved {
                id,
                resolution: verdict,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{REASON_NEVER_FIRES, REASON_NO_COMMAND};
    use crate::api::syntax::SyntaxView;
    use crate::debugger::breakpoints::BreakpointResolution;
    use crate::debugger::event::DebugEvent;
    use crate::debugger::session::DebugSession;

    fn view_of(file: &str, source: &str) -> SyntaxView {
        let tokens = crate::lexer::tokenize(source);
        let ast = crate::parser::parse(&tokens);
        SyntaxView::from_ast(file, &ast)
    }

    fn resolution(
        session: &DebugSession,
        id: crate::debugger::BreakpointId,
    ) -> BreakpointResolution {
        session
            .breakpoints()
            .get(id)
            .expect("entry exists")
            .resolution
            .clone()
    }

    #[test]
    fn line_breakpoint_on_an_unparsed_file_is_pending_then_resolves() {
        let mut session = DebugSession::new();
        let id = session.set_line_breakpoint("a.sh", 2);
        assert_eq!(resolution(&session, id), BreakpointResolution::Pending);
        session.drain_events();
        session.register_syntax_view(view_of("a.sh", "echo one\necho two\n"));
        assert_eq!(resolution(&session, id), BreakpointResolution::Verified);
        let events = session.drain_events();
        assert_eq!(
            events,
            vec![DebugEvent::BreakpointResolved {
                id,
                resolution: BreakpointResolution::Verified
            }],
            "the resolution change is reported exactly once (DE-11)"
        );
    }

    #[test]
    fn line_breakpoint_set_after_the_view_verifies_immediately() {
        let mut session = DebugSession::new();
        session.register_syntax_view(view_of("a.sh", "echo one\necho two\n"));
        session.drain_events();
        let id = session.set_line_breakpoint("a.sh", 1);
        assert_eq!(resolution(&session, id), BreakpointResolution::Verified);
        assert!(
            session.drain_events().is_empty(),
            "insertion verdicts are synchronous: no DE-11 is emitted"
        );
    }

    #[test]
    fn dead_lines_carry_the_designs_two_reasons() {
        let mut session = DebugSession::new();
        session.register_syntax_view(view_of(
            "a.sh",
            "# comment\nif\n  test -f x\nthen\n  echo yes\nfi\n",
        ));
        let comment = session.set_line_breakpoint("a.sh", 1);
        assert_eq!(
            resolution(&session, comment),
            BreakpointResolution::Unverified {
                reason: REASON_NO_COMMAND.to_string()
            }
        );
        let bare_if = session.set_line_breakpoint("a.sh", 2);
        assert_eq!(
            resolution(&session, bare_if),
            BreakpointResolution::Unverified {
                reason: REASON_NEVER_FIRES.to_string()
            },
            "a command node starts on the bare `if` line but no seam fires at it"
        );
        let then_line = session.set_line_breakpoint("a.sh", 4);
        assert_eq!(
            resolution(&session, then_line),
            BreakpointResolution::Unverified {
                reason: REASON_NO_COMMAND.to_string()
            }
        );
        let body = session.set_line_breakpoint("a.sh", 5);
        assert_eq!(resolution(&session, body), BreakpointResolution::Verified);
    }

    #[test]
    fn a_pending_dead_line_reports_unverified_when_its_file_loads() {
        let mut session = DebugSession::new();
        let id = session.set_line_breakpoint("a.sh", 3);
        assert_eq!(resolution(&session, id), BreakpointResolution::Pending);
        session.drain_events();
        session.register_syntax_view(view_of("a.sh", "echo one\n# note\necho three\n"));
        assert_eq!(
            resolution(&session, id),
            BreakpointResolution::Verified,
            "line 3 carries the second echo in this fixture"
        );
        let id = session.set_line_breakpoint("a.sh", 2);
        assert_eq!(
            resolution(&session, id),
            BreakpointResolution::Unverified {
                reason: REASON_NO_COMMAND.to_string()
            }
        );
    }

    #[test]
    fn function_breakpoint_pends_until_a_view_proves_the_definition() {
        let mut session = DebugSession::new();
        let id = session.set_function_breakpoint("helper");
        assert_eq!(resolution(&session, id), BreakpointResolution::Pending);
        session.drain_events();
        session.register_syntax_view(view_of("lib.sh", "echo top\n"));
        assert_eq!(
            resolution(&session, id),
            BreakpointResolution::Pending,
            "a view of some other file's functions proves nothing"
        );
        assert!(session.drain_events().is_empty(), "no change, no event");
        session.register_syntax_view(view_of("lib.sh", "helper() {\n  echo h\n}\n"));
        assert_eq!(resolution(&session, id), BreakpointResolution::Verified);
        let events = session.drain_events();
        assert_eq!(
            events,
            vec![DebugEvent::BreakpointResolved {
                id,
                resolution: BreakpointResolution::Verified
            }]
        );
    }

    #[test]
    fn function_breakpoint_verifies_immediately_when_already_defined() {
        let mut session = DebugSession::new();
        session.register_syntax_view(view_of("lib.sh", "helper() {\n  echo h\n}\n"));
        let id = session.set_function_breakpoint("helper");
        assert_eq!(resolution(&session, id), BreakpointResolution::Verified);
    }

    #[test]
    fn runtime_definition_note_resolves_a_pending_function_breakpoint() {
        let mut session = DebugSession::new();
        let id = session.set_function_breakpoint("eval_made");
        assert_eq!(resolution(&session, id), BreakpointResolution::Pending);
        session.drain_events();
        session.note_function_defined("eval_made");
        assert_eq!(resolution(&session, id), BreakpointResolution::Verified);
        let events = session.drain_events();
        assert_eq!(events.len(), 1, "exactly one DE-11");
        session.note_function_defined("eval_made");
        assert!(
            session.drain_events().is_empty(),
            "a repeated note changes nothing and reports nothing"
        );
    }

    #[test]
    fn re_registering_a_view_emits_no_duplicate_events() {
        let mut session = DebugSession::new();
        let id = session.set_line_breakpoint("a.sh", 1);
        session.register_syntax_view(view_of("a.sh", "echo one\n"));
        session.drain_events();
        session.register_syntax_view(view_of("a.sh", "echo one\n"));
        assert_eq!(resolution(&session, id), BreakpointResolution::Verified);
        assert!(session.drain_events().is_empty());
    }
}
