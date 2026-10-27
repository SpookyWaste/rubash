//! On-demand stack growth for the recursive tree walkers (rubash#1801).
//!
//! The parser and the executor walk the command tree recursively, exactly
//! like GNU: `execute_cmd.c` dispatches compound commands by recursive C
//! calls, and the only depth limit GNU knows is FUNCNEST for function
//! calls (`execute_cmd.c:324-325` `funcnest`/`funcnest_max`, enforced at
//! `execute_cmd.c:5200` only when FUNCNEST > 0; `variables.c:5945`
//! `sv_funcnest`) — compound nesting itself is bounded only by the real
//! stack. GNU owns its stack (the shell process's main thread), but this
//! crate is a library: hosts call `Executor::execute_ast` and
//! `parser::parse` on threads they own, and the walkers' debug call chains
//! burn ~150KB of stack per nesting level (the figure `src/main.rs` cites
//! when it gives the binary driver a 512MiB stack thread). On an ordinary
//! host thread — libtest's 2MiB test threads, an embedder's worker — a
//! script nested a handful of levels, or a function chain a few calls
//! deep, overflowed the stack and took the whole process down (SIGSEGV /
//! SIGABRT in the executor_tests harness, rubash#1801).
//!
//! The fix sits at the resource layer, not the semantics layer: the two
//! recursion hubs (`parser::parse_loop::run_command_loop` and
//! `Executor::execute_ast_inner`, through which every nested command list
//! funnels) pass through [`ensure_stack`], which grows the current
//! thread's stack when the remainder runs low. No depth is counted and no
//! limit is imposed — behavior is identical at every depth; the stack
//! simply stops being the binding constraint before memory is, matching
//! the contract GNU gets from owning its process.

/// Remaining-stack floor: when less than this is left, grow before
/// descending further. One walker level can burn ~250KB of debug stack
/// (fat opt-0 frames along the dispatch chain), so the floor keeps several
/// levels of headroom under the deepest single inter-hub chain observed.
const STACK_RED_ZONE_BYTES: usize = 1024 * 1024;

/// Size of each freshly grown stack segment. The reservation is virtual —
/// pages are committed on use — so an 8MiB segment costs address space,
/// not RAM, until the recursion actually descends into it.
const STACK_SEGMENT_BYTES: usize = 8 * 1024 * 1024;

/// Run `f` on the current thread, growing the stack first if the remaining
/// stack is below the red-zone floor. Growth never migrates threads:
/// `Executor` is a single-thread session object (it holds `Rc` state), so
/// `stacker` switches only the stack segment under the same thread.
pub(crate) fn ensure_stack<R>(f: impl FnOnce() -> R) -> R {
    stacker::maybe_grow(STACK_RED_ZONE_BYTES, STACK_SEGMENT_BYTES, f)
}
