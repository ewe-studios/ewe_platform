#![cfg(test)]
//! Unit tests for `TlsHandshakeTask` moved into the canonical units test tree.
//!
//! These are non-destructive copies of the original in-crate `#[cfg(test)]`
//! module. They perform lightweight compile-time and debug-string checks to
//! ensure the TLS handshake task exposes the expected types and trait bounds.

use foundation_core::valtron::{NoAction, TaskIterator};
use foundation_core::wire::simple_http::client::{TlsHandshakeState, TlsHandshakeTask};

/// WHY: Verify all TlsHandshakeState variants exist and are constructible
/// WHAT: Ensure enum variants are present (compile-time / runtime sanity)
#[test]
fn test_tls_handshake_state_variants() {
    let _init = TlsHandshakeState::Init;
    let _handshaking = TlsHandshakeState::Handshaking;
    let _complete = TlsHandshakeState::Complete;
    let _error = TlsHandshakeState::Error;

    // Basic equality checks to ensure variants behave as expected
    assert_ne!(_init, _complete);
    assert_eq!(TlsHandshakeState::Init, TlsHandshakeState::Init);
}

/// WHY: Verify Debug formatting for the state contains the variant name
/// WHAT: Ensure Debug impl is present and produces expected substring
#[test]
fn test_tls_handshake_state_debug() {
    let state = TlsHandshakeState::Init;
    let debug_str = format!("{:?}", state);
    assert!(
        debug_str.contains("Init"),
        "Debug string should include variant name"
    );
}

/// WHY: Compile-time check that `TlsHandshakeTask` implements `TaskIterator`
/// WHAT: This ensures the task type satisfies the executor contract
#[test]
fn test_tls_handshake_task_is_task_iterator() {
    // Compile-time check that TlsHandshakeTask implements TaskIterator
    fn assert_task_iterator<T: TaskIterator>() {}
    assert_task_iterator::<TlsHandshakeTask>();
}

/// WHY: Verify associated types of TaskIterator for TlsHandshakeTask are correct
/// WHAT: Ensure Pending = TlsHandshakeState, Ready = (), Spawner = NoAction
#[test]
fn test_tls_handshake_task_associated_types() {
    // If TlsHandshakeTask does not match the required associated types, this will fail to compile.
    fn check_types<T>()
    where
        T: TaskIterator<Pending = TlsHandshakeState, Ready = (), Spawner = NoAction>,
    {
    }

    check_types::<TlsHandshakeTask>();
}
