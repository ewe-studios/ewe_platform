//! Shared auth_state tests.

use foundation_auth::shared::auth_state::{AuthEvent, AuthState, AuthStateMachine};

#[test]
fn test_initial_state() {
    let sm = AuthStateMachine::new();
    assert_eq!(sm.current(), AuthState::Unauthenticated);
    assert!(!sm.is_refreshing());
}

#[test]
fn test_successful_auth_flow() {
    let mut sm = AuthStateMachine::new();

    sm.transition_to(AuthEvent::AuthenticateStarted).unwrap();
    assert_eq!(sm.current(), AuthState::Authenticating);

    sm.transition_to(AuthEvent::AuthenticateCompleted).unwrap();
    assert_eq!(sm.current(), AuthState::Authenticated);
    assert!(sm.current().can_make_request());
}

#[test]
fn test_token_expiry_and_refresh() {
    let mut sm = AuthStateMachine::new();

    sm.transition_to(AuthEvent::AuthenticateStarted).unwrap();
    sm.transition_to(AuthEvent::AuthenticateCompleted).unwrap();

    sm.transition_to(AuthEvent::TokenExpired).unwrap();
    assert_eq!(sm.current(), AuthState::TokenExpired);

    sm.transition_to(AuthEvent::RefreshStarted).unwrap();
    assert!(sm.is_refreshing());

    let id1 = sm.enqueue_request();
    let id2 = sm.enqueue_request();
    assert_eq!(sm.queue_len(), 2);

    let drained = sm.handle_event(AuthEvent::RefreshCompleted).unwrap();
    assert_eq!(drained.len(), 2);
    assert_eq!(drained[0].id, id1);
    assert_eq!(drained[1].id, id2);
    assert_eq!(sm.queue_len(), 0);
    assert_eq!(sm.current(), AuthState::Authenticated);
}

#[test]
fn test_refresh_failure() {
    let mut sm = AuthStateMachine::new();

    sm.transition_to(AuthEvent::AuthenticateStarted).unwrap();
    sm.transition_to(AuthEvent::AuthenticateCompleted).unwrap();
    sm.transition_to(AuthEvent::TokenExpired).unwrap();
    sm.transition_to(AuthEvent::RefreshStarted).unwrap();

    sm.transition_to(AuthEvent::RefreshFailed).unwrap();
    assert_eq!(sm.current(), AuthState::Failed);
    assert!(sm.current().is_terminal());
}

#[test]
fn test_recovery_from_failure() {
    let mut sm = AuthStateMachine::new();

    sm.transition_to(AuthEvent::AuthenticateStarted).unwrap();
    sm.transition_to(AuthEvent::AuthenticateCompleted).unwrap();
    sm.transition_to(AuthEvent::TokenExpired).unwrap();
    sm.transition_to(AuthEvent::RefreshStarted).unwrap();
    sm.transition_to(AuthEvent::RefreshFailed).unwrap();

    sm.transition_to(AuthEvent::AuthenticateStarted).unwrap();
    sm.transition_to(AuthEvent::AuthenticateCompleted).unwrap();
    assert_eq!(sm.current(), AuthState::Authenticated);
}
