//! Shared session tests.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{Duration, Utc};
use foundation_auth::shared::credential_store::CredentialStorage;
use foundation_auth::shared::session::{
    KeyRing, Session, SessionConfig, SessionManager, SigningKey, TokenSigner,
};
use foundation_auth::CredentialStore;
use foundation_core::valtron::valtron_test;

fn make_manager() -> SessionManager<CredentialStorage> {
    let store = CredentialStorage::memory();
    let key = vec![0xAB; 32];
    SessionManager::new(store, SessionConfig::default(), &key).expect("create session manager")
}

#[valtron_test]
async fn test_create_session() {
    let mgr = make_manager();
    let (session, cookies) = mgr
        .create_session("user_1", Some("127.0.0.1"), Some("TestAgent"))
        .await
        .unwrap();

    assert_eq!(session.user_id, "user_1");
    assert_ne!(session.token.get(), "");
    assert!(!session.revoked);
    assert_ne!(cookies.len(), 0);
}

#[valtron_test]
async fn test_revoke_single_session() {
    let mgr = make_manager();
    let (session, _) = mgr.create_session("user_1", None, None).await.unwrap();

    mgr.revoke_session(&session.id).await.unwrap();

    let stored: Option<Session> = mgr.store().get(&session.id).await.unwrap();
    assert!(stored.is_some_and(|s| s.revoked));
}

#[valtron_test]
async fn test_revoke_all_sessions() {
    let mgr = make_manager();
    mgr.create_session("user_1", None, None).await.unwrap();
    mgr.create_session("user_1", None, None).await.unwrap();
    mgr.create_session("user_2", None, None).await.unwrap();

    let count = mgr.revoke_all_sessions("user_1").await.unwrap();
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

#[test]
fn token_signer_rejects_modified_tag() {
    let signer = TokenSigner::new(KeyRing::single(&[0xAB; 32]).expect("ring"));
    let token_text = signer.sign("plain-token");

    // Flip one bit in the decoded signature, then re-encode it.
    let mut parts = token_text.split('.');
    let plain = parts.next().expect("plain part");
    let kid = parts.next().expect("key id part");
    let sig = parts.next().expect("signature part");
    let mut decoded = URL_SAFE_NO_PAD.decode(sig).expect("decode signature");
    decoded[0] ^= 0xFF;
    let tampered = format!("{plain}.{kid}.{}", URL_SAFE_NO_PAD.encode(&decoded));

    assert!(!signer.verify(&tampered, "plain-token"));
}

#[test]
fn token_signer_rejects_truncated_tag() {
    let signer = TokenSigner::new(KeyRing::single(&[0xAB; 32]).expect("ring"));
    let token_text = signer.sign("plain-token");

    // Drop one byte from the decoded signature, then re-encode it, so the tag
    // is valid base64 but one byte short of the expected 32.
    let mut parts = token_text.split('.');
    let plain = parts.next().expect("plain part");
    let kid = parts.next().expect("key id part");
    let sig = parts.next().expect("signature part");
    let mut decoded = URL_SAFE_NO_PAD.decode(sig).expect("decode signature");
    decoded.truncate(decoded.len() - 1);
    let truncated = format!("{plain}.{kid}.{}", URL_SAFE_NO_PAD.encode(&decoded));

    assert!(!signer.verify(&truncated, "plain-token"));
}

fn key(id: &str, byte: u8) -> SigningKey {
    SigningKey::new(id, &[byte; 32]).expect("signing key")
}

/// Build the old two-part token `token.tag` with a raw key.
fn legacy_token(raw_key: &[u8; 32], plain: &str) -> String {
    use hmac::{Hmac, Mac};
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(raw_key).expect("hmac");
    mac.update(plain.as_bytes());
    format!(
        "{plain}.{}",
        URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
    )
}

#[test]
fn token_has_three_parts_with_the_current_key_id() {
    let signer = TokenSigner::new(KeyRing::new(key("k2", 2)));
    let token_text = signer.sign("plain-token");
    let parts: Vec<&str> = token_text.split('.').collect();
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0], "plain-token");
    assert_eq!(parts[1], "k2");
    assert!(signer.verify(&token_text, "plain-token"));
    assert!(!signer.verify(&token_text, "other-token"));
}

#[test]
fn rotated_key_still_verifies_old_tokens() {
    let old = TokenSigner::new(KeyRing::new(key("k1", 1)));
    let signed_old = old.sign("plain-token");

    let ring = KeyRing::new(key("k2", 2))
        .with_previous(key("k1", 1))
        .expect("ring");
    let rotated = TokenSigner::new(ring);

    assert!(rotated.verify(&signed_old, "plain-token"));
    // New tokens use the new key and the old signer cannot verify them.
    let signed_new = rotated.sign("plain-token");
    assert!(signed_new.contains(".k2."));
    assert!(rotated.verify(&signed_new, "plain-token"));
    assert!(!old.verify(&signed_new, "plain-token"));
}

#[test]
fn removed_previous_key_stops_verifying() {
    let old = TokenSigner::new(KeyRing::new(key("k1", 1)));
    let signed_old = old.sign("plain-token");
    let only_new = TokenSigner::new(KeyRing::new(key("k2", 2)));
    assert!(!only_new.verify(&signed_old, "plain-token"));
}

#[test]
fn unknown_kid_is_rejected() {
    let signer = TokenSigner::new(KeyRing::new(key("k1", 1)));
    let token_text = signer.sign("plain-token");
    let forged = token_text.replace(".k1.", ".zz.");
    assert!(!signer.verify(&forged, "plain-token"));
    // A kid that names the right key but with a tag from another key also fails.
    let other = TokenSigner::new(KeyRing::new(key("k1", 9))).sign("plain-token");
    assert!(!signer.verify(&other, "plain-token"));
}

#[test]
fn malformed_tokens_are_rejected() {
    let signer = TokenSigner::new(KeyRing::new(key("k1", 1)));
    for bad in ["", "a", "a.b.c.d", "a.k1.!!!", "a.k1."] {
        assert!(!signer.verify(bad, "a"), "accepted {bad:?}");
    }
}

#[test]
fn legacy_token_without_kid_verifies_when_allowed() {
    let raw = [7u8; 32];
    let legacy = legacy_token(&raw, "plain-token");
    let signer = TokenSigner::new(KeyRing::single(&raw).expect("ring"));
    assert!(signer.verify(&legacy, "plain-token"));
    assert!(!signer.verify(&legacy, "other-token"));
}

#[test]
fn legacy_token_without_kid_rejected_when_disabled() {
    let raw = [7u8; 32];
    let legacy = legacy_token(&raw, "plain-token");
    let ring = KeyRing::single(&raw)
        .expect("ring")
        .allow_legacy_tokens(false);
    let signer = TokenSigner::new(ring);
    assert!(!signer.verify(&legacy, "plain-token"));
    // Tokens with a key id still work.
    let token_text = signer.sign("plain-token");
    assert!(signer.verify(&token_text, "plain-token"));
}

#[test]
fn key_ring_rejects_bad_keys_and_duplicate_ids() {
    assert!(SigningKey::new("k1", &[1; 31]).is_err());
    assert!(SigningKey::new("", &[1; 32]).is_err());
    assert!(SigningKey::new("a.b", &[1; 32]).is_err());
    assert!(SigningKey::new("12345678901234567", &[1; 32]).is_err());
    assert!(KeyRing::new(key("k1", 1))
        .with_previous(key("k1", 2))
        .is_err());
}

#[test]
fn signing_key_debug_hides_key_bytes() {
    let text = format!("{:?}", key("k1", 0x41));
    assert!(text.contains("k1"));
    assert!(text.contains("redacted"));
    assert!(!text.contains("65"));
}

#[valtron_test]
async fn session_survives_key_rotation() {
    let mgr = SessionManager::with_key_ring(
        CredentialStorage::memory(),
        SessionConfig::default(),
        KeyRing::new(key("k1", 1)),
    );
    let (_session, cookies) = mgr.create_session("user_1", None, None).await.unwrap();
    let token = cookies
        .iter()
        .find(|c| c.name == "session_token")
        .expect("token cookie")
        .value
        .clone();

    mgr.rotate_keys(
        KeyRing::new(key("k2", 2))
            .with_previous(key("k1", 1))
            .unwrap(),
    );
    assert!(mgr.get_session(&token).await.unwrap().is_some());

    // Drop the old key: the session no longer validates.
    mgr.rotate_keys(KeyRing::new(key("k2", 2)));
    assert!(mgr.get_session(&token).await.unwrap().is_none());
}
