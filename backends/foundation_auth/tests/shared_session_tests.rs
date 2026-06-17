//! Shared session tests.

use chrono::{Duration, Utc};
use foundation_auth::shared::credential_store::CredentialStorage;
use foundation_auth::shared::session::{Session, SessionConfig, SessionManager};
use foundation_auth::CredentialStore;

fn init_valtron() {
    foundation_core::valtron::single::initialize_pool(42);
}

fn make_manager() -> SessionManager<CredentialStorage> {
    init_valtron();
    let store = CredentialStorage::memory();
    let key = vec![0xAB; 32];
    SessionManager::new(store, SessionConfig::default(), &key).expect("create session manager")
}

#[test]
fn test_create_session() {
    let mgr = make_manager();
    let (session, cookies) = mgr
        .create_session("user_1", Some("127.0.0.1"), Some("TestAgent"))
        .unwrap();

    assert_eq!(session.user_id, "user_1");
    assert!(!session.token.get().is_empty());
    assert!(!session.revoked);
    assert!(!cookies.is_empty());
}

#[test]
fn test_revoke_single_session() {
    let mgr = make_manager();
    let (session, _) = mgr.create_session("user_1", None, None).unwrap();

    mgr.revoke_session(&session.id).unwrap();

    let stored: Option<Session> = mgr.store().get(&session.id).unwrap();
    assert!(stored.is_some_and(|s| s.revoked));
}

#[test]
fn test_revoke_all_sessions() {
    let mgr = make_manager();
    mgr.create_session("user_1", None, None).unwrap();
    mgr.create_session("user_1", None, None).unwrap();
    mgr.create_session("user_2", None, None).unwrap();

    let count = mgr.revoke_all_sessions("user_1").unwrap();
    assert_eq!(count, 2);
}

#[test]
fn test_session_is_valid() {
    let session = Session {
        id: "test".to_string(),
        user_id: "user_1".to_string(),
        token: foundation_auth::shared::types::ConfidentialText::new("tok".to_string()),
        created_at: Utc::now(),
        expires_at: Utc::now() + Duration::hours(1),
        ip_address: None,
        user_agent: None,
        last_active_at: Utc::now(),
        revoked: false,
    };
    assert!(session.is_valid());

    let expired = Session {
        expires_at: Utc::now() - Duration::hours(1),
        ..session.clone()
    };
    assert!(!expired.is_valid());

    let revoked = Session {
        revoked: true,
        ..session.clone()
    };
    assert!(!revoked.is_valid());
}

#[test]
fn test_signing_key_too_short() {
    let store = CredentialStorage::memory();
    let result = SessionManager::new(store, SessionConfig::default(), b"short");
    assert!(result.is_err());
}
