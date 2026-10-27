//! Compile-time anchors for the 1.5.0 Session threading contract
//! (Session API design, Q5 — decided 2026-10-25: (a)).
//!
//! The contract is explicit: a session is single-threaded — one
//! session per thread, `!Send` by construction — and hosts that need
//! sessions on several threads create one per thread. Today the unit
//! a host drives is `Executor`, and its `!Send`/`!Sync` nature is a
//! *fact* of its fields (the `Rc<RefCell<…>>` handles: history
//! provider, session history, bind registry, function bodies, fd
//! endpoints, timed input) rather than a stated invariant. Nothing in
//! the tree asserted it, so a future refactor swapping an `Rc` for an
//! `Arc` would silently revoke the contract while every test stayed
//! green.
//!
//! These assertions make the property mechanical: this file compiles
//! only while `Executor` is neither `Send` nor `Sync`. If it stops
//! compiling, the threading contract changed shape — that is a design
//! event for the Session API line, not a test to delete.

/// Negative auto-trait assertion (the `static_assertions` pattern):
/// the `_` in `AmbiguousIfImpl<_>` is inferable only when exactly one
/// impl applies. If `T` implements the auto trait, both the blanket
/// impl and the bounded impl apply, inference is ambiguous, and the
/// file fails to compile.
macro_rules! assert_not_impl {
    ($x:ty: $t:path) => {
        const _: fn() = || {
            trait AmbiguousIfImpl<A> {
                fn some_item() {}
            }

            impl<T: ?Sized> AmbiguousIfImpl<()> for T {}

            impl<T: ?Sized + $t> AmbiguousIfImpl<u8> for T {}

            let _ = <$x as AmbiguousIfImpl<_>>::some_item;
        };
    };
}

assert_not_impl!(rubash::Executor: Send);
assert_not_impl!(rubash::Executor: Sync);

#[test]
fn executor_is_single_threaded_by_construction() {
    // The substance of this test is above, at compile time: reaching
    // this line at all means `Executor` was neither `Send` nor `Sync`
    // when this target was built.
}
