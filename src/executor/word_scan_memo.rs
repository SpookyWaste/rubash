//! Per-word-pass memo for the pure word-text scans (redundancy-hunt R2).
//!
//! Several executor predicates are pure functions of a word's raw text —
//! `raw_word_is_quoted`, `scan_substitution_spans`, `expand_braces`,
//! `word_contains_brace_group` — yet one `expand_command_word` pass used to
//! recompute them on the SAME text two to four times (the `$@`/`$*`/braced
//! gates each re-ran `raw_word_is_quoted`; the brace trial ran a full
//! `expand_braces` and `expand_braces_with_optional_raw` then ran it again;
//! the substitution-fragment path and the context derivation each ran the
//! span DFA). The answers cannot differ between those calls: nothing in a
//! single word-expansion pass can change what a pure function of the text
//! returns. This memo caches the answers for the duration of one word pass.
//!
//! Lifecycle follows the SubXpassFrame idiom (expand_braced_indices): a
//! frame is pushed by the two word-expansion entries
//! (`expand_command_word`, `expand_word_mut_with_context`) and popped when
//! the pass ends, so entries never outlive their word. Lookups search the
//! frame stack top-down; stores land in the top frame. With no frame
//! installed the lookups miss and callers compute directly, so entries
//! outside a word pass behave exactly as before.

use std::collections::HashMap;
use std::rc::Rc;

use super::substitution_metadata::SubstitutionSpan;

#[derive(Default)]
struct WordScanFrame {
    quoted: HashMap<String, bool>,
    spans: HashMap<String, Rc<Vec<SubstitutionSpan>>>,
    braces: HashMap<String, Rc<Vec<String>>>,
    brace_group: HashMap<String, bool>,
}

thread_local! {
    static WORD_SCAN_MEMO: std::cell::RefCell<Vec<WordScanFrame>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// RAII frame: one word-scan memo scope per word-expansion entry. Nested
/// entries (a word pass reaching `expand_word_mut_with_context`) stack;
/// the pure answers are text-keyed, so frames can share hits safely.
pub(in crate::executor) struct WordScanMemoFrame;

impl WordScanMemoFrame {
    pub(in crate::executor) fn new() -> Self {
        WORD_SCAN_MEMO.with(|frames| frames.borrow_mut().push(WordScanFrame::default()));
        Self
    }
}

impl Drop for WordScanMemoFrame {
    fn drop(&mut self) {
        WORD_SCAN_MEMO.with(|frames| {
            frames.borrow_mut().pop();
        });
    }
}

fn lookup<T: Clone>(pick: impl Fn(&WordScanFrame) -> Option<T>) -> Option<T> {
    WORD_SCAN_MEMO.with(|frames| frames.borrow().iter().rev().find_map(|frame| pick(frame)))
}

fn store(update: impl FnOnce(&mut WordScanFrame)) {
    WORD_SCAN_MEMO.with(|frames| {
        if let Some(frame) = frames.borrow_mut().last_mut() {
            update(frame);
        }
    });
}

pub(in crate::executor) fn memo_raw_word_is_quoted(raw: &str) -> Option<bool> {
    lookup(|frame| frame.quoted.get(raw).copied())
}

pub(in crate::executor) fn memo_store_raw_word_is_quoted(raw: &str, quoted: bool) {
    store(|frame| {
        frame.quoted.insert(raw.to_string(), quoted);
    });
}

pub(in crate::executor) fn memo_substitution_spans(raw: &str) -> Option<Rc<Vec<SubstitutionSpan>>> {
    lookup(|frame| frame.spans.get(raw).cloned())
}

pub(in crate::executor) fn memo_store_substitution_spans(
    raw: &str,
    spans: Rc<Vec<SubstitutionSpan>>,
) {
    store(|frame| {
        frame.spans.insert(raw.to_string(), spans);
    });
}

pub(in crate::executor) fn memo_brace_expansion(text: &str) -> Option<Rc<Vec<String>>> {
    lookup(|frame| frame.braces.get(text).cloned())
}

pub(in crate::executor) fn memo_store_brace_expansion(text: &str, expansion: Rc<Vec<String>>) {
    store(|frame| {
        frame.braces.insert(text.to_string(), expansion);
    });
}

pub(in crate::executor) fn memo_word_contains_brace_group(text: &str) -> Option<bool> {
    lookup(|frame| frame.brace_group.get(text).copied())
}

pub(in crate::executor) fn memo_store_word_contains_brace_group(text: &str, contains: bool) {
    store(|frame| {
        frame.brace_group.insert(text.to_string(), contains);
    });
}
