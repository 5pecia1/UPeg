//! The ambient cancellation scope: installation, nesting, and the
//! guarantee that a panicking tool cannot leak a token into the next
//! dispatch on the same thread.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use super::{CancellationToken, active_cancellation, with_cancellation};

#[test]
fn new_token_is_not_cancelled() {
    let token = CancellationToken::new();

    assert!(!token.is_cancelled());
}

#[test]
fn cancellation_is_shared_across_clones() {
    let token = CancellationToken::new();
    let clone = token.clone();

    clone.cancel();

    assert!(
        token.is_cancelled(),
        "cancelling a clone is visible on the original"
    );
}

#[test]
fn cancellation_does_not_revert() {
    let token = CancellationToken::new();

    token.cancel();
    token.cancel();

    assert!(token.is_cancelled());
}

#[test]
fn without_installation_there_is_no_ambient_token() {
    assert!(
        active_cancellation().is_none(),
        "a dispatch that installed nothing pays no cancellation polling cost"
    );
}

#[test]
fn installed_token_is_visible_inside_scope() {
    let token = CancellationToken::new();

    let seen_token = with_cancellation(token.clone(), active_cancellation);

    let seen_token = seen_token.expect("an ambient token must exist inside the scope");
    token.cancel();
    assert!(
        seen_token.is_cancelled(),
        "the captured token observes the same latch as the installed token"
    );
}

#[test]
fn leaving_scope_restores_previous_token() {
    let inner = CancellationToken::new();

    with_cancellation(CancellationToken::new(), || {
        with_cancellation(inner.clone(), || {
            inner.cancel();
            assert!(
                active_cancellation()
                    .expect("token of the nested scope")
                    .is_cancelled(),
                "the nested inner token wins for that span"
            );
        });
        assert!(
            !active_cancellation()
                .expect("restored outer token")
                .is_cancelled(),
            "cancelling the inner scope must not leak into the outer token"
        );
    });

    assert!(
        active_cancellation().is_none(),
        "no token exists outside the scope"
    );
}

#[test]
fn panic_does_not_leak_token_into_next_dispatch() {
    let token = CancellationToken::new();

    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_cancellation(token, || panic!("the tool panicked"));
    }));

    assert!(outcome.is_err(), "the panic propagates unchanged");
    assert!(
        active_cancellation().is_none(),
        "the previous scope is restored even during unwind"
    );
}

#[test]
fn token_can_be_cancelled_from_another_thread() {
    let token = CancellationToken::new();
    let observed = Arc::new(AtomicBool::new(false));

    let canceller = {
        let token = token.clone();
        thread::spawn(move || token.cancel())
    };
    canceller.join().expect("canceller thread finishes");
    observed.store(token.is_cancelled(), Ordering::Relaxed);

    assert!(
        observed.load(Ordering::Relaxed),
        "cancellation by the consumer thread is visible to the producer thread"
    );
}

#[test]
fn installed_scope_is_inherited_by_nested_dispatch() {
    // A `Chain` runs its steps on the dispatching thread, so a step's
    // invoker must see the token the *call* was installed with.
    let token = CancellationToken::new();

    let token_seen_by_step = with_cancellation(token.clone(), || {
        fn chain_step() -> Option<CancellationToken> {
            active_cancellation()
        }
        chain_step()
    });

    token.cancel();
    assert!(
        token_seen_by_step
            .expect("a nested call also sees the ambient token")
            .is_cancelled()
    );
}
