//! Upstream auth session store tests.

use foundation_auth::shared::auth_session::memory::MemoryAuthSessionStore;
use foundation_auth::shared::auth_session::{
    AuthSessionError, UpstreamAuthSession, UpstreamAuthSessionStore,
};

#[test]
fn session_insert_and_take_round_trips() {
    let store = MemoryAuthSessionStore::new();
    let session = UpstreamAuthSession::new(
        "state-abc123".into(),
        "google".into(),
        Some("pkce-verifier".into()),
        Some("nonce-123".into()),
        Some("https://app.example.com/dashboard".into()),
        600,
    );

    store.insert(&session).expect("insert");
    let taken = store.take("state-abc123").expect("take");
    assert_eq!(taken.provider_id, "google");
    assert_eq!(taken.code_verifier.unwrap(), "pkce-verifier");
    assert_eq!(taken.nonce.unwrap(), "nonce-123");
    assert_eq!(
        taken.redirect_to.unwrap(),
        "https://app.example.com/dashboard"
    );
}

#[test]
fn take_removes_session_one_time_only() {
    let store = MemoryAuthSessionStore::new();
    let session =
        UpstreamAuthSession::new("state-xyz".into(), "github".into(), None, None, None, 600);
    store.insert(&session).expect("insert");
    let _first = store.take("state-xyz").expect("first take");
    let second = store.take("state-xyz");
    assert!(matches!(second, Err(AuthSessionError::NotFound)));
}

#[test]
fn not_found_returns_error() {
    let store = MemoryAuthSessionStore::new();
    let err = store.take("nonexistent").unwrap_err();
    assert!(matches!(err, AuthSessionError::NotFound));
}

#[test]
fn expired_session_returns_error() {
    let store = MemoryAuthSessionStore::new();
    let mut session = UpstreamAuthSession::new(
        "expired-state".into(),
        "google".into(),
        None,
        None,
        None,
        1, // 1-second TTL
    );
    // Fake expiry — set created_at to 2 seconds ago
    session.created_at = session.created_at.saturating_sub(2000);
    store.insert(&session).expect("insert");
    let err = store.take("expired-state").unwrap_err();
    assert!(matches!(err, AuthSessionError::Expired));
}
