//! The 1.5.0 Session API, in-tree (API design / rubash#595).
//!
//! This module is the API surface shaped for its future home in the
//! split engine core: it is built in-tree first and moves wholesale
//! when the workspace split lands (PLAN row 4), the same staging the
//! debugger core used (150-c precedent). Until the split widens it,
//! visibility stays `pub(crate)`: the freeze discipline of the API
//! ledger (docs/api-ledger.md) governs the *existing* consumed
//! surface, and no new contract is published from here early.
//!
//! What lives here today is PLAN row 6 (S3's views part): the §5.3
//! state views — variables, options, environment, cwd, positional
//! parameters, aliases, functions, last status — plus the owned
//! snapshot types the debugger's inspection rendering (WP-6) and
//! future frontends render. One rule governs the whole module, from
//! the debugger design §7: **inspection is a rendering of state the
//! engine already maintains; there is no shadow bookkeeping.** A
//! snapshot is captured from the engine's own stores and views at
//! the moment it is taken, and its declaration text is produced by
//! the same printer `declare -p` uses, so the API and the shell can
//! never disagree about what a variable looks like.

pub mod views;

#[cfg(test)]
mod consumer_tests;
