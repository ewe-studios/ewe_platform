//! Core domain logic tests (spec-57, F008).
//!
//! Tests for types, validation, serialization, error shapes, icon SSRF
//! guard, security stamp verification, and notification framing.

use foundation_keychain::core::auth::{verify_security_stamp, BitwardenClaims};
use foundation_keychain::core::error::{AppError, ValidationError};
use foundation_keychain::core::models::cipher::{
    CipherCreateRequest, CipherType, CipherUpdateRequest,
};
use foundation_keychain::core::models::folder::{Folder, FolderCreateRequest};
use foundation_keychain::core::models::org::{Organization, OrganizationCreateRequest};
use foundation_keychain::core::models::send::{Send, SendCreateRequest, SendType};
use foundation_keychain::core::models::user::{
    KdfType, PreloginResponse, RegisterRequest, UpdateProfileRequest,
};
use foundation_keychain::core::notifications::{Notification, UpdateType};
use foundation_keychain::core::util::validate_icon_domain;

// ── AppError ────────────────────────────────────────────────────────────

#[test]
fn app_error_produces_correct_status_codes() {
    assert_eq!(AppError::BadRequest("bad".into()).status(), 400);
    assert_eq!(AppError::Unauthorized.status(), 401);
    assert_eq!(AppError::Forbidden.status(), 403);
    assert_eq!(AppError::NotFound("missing".into()).status(), 404);
    assert_eq!(AppError::Conflict("dup".into()).status(), 409);
    assert_eq!(AppError::RateLimited.status(), 429);
    assert_eq!(AppError::Internal("boom".into()).status(), 500);
}

#[test]
fn app_error_json_includes_error_and_description() {
    let err = AppError::BadRequest("name is required".into());
    let json = err.to_json();
    assert_eq!(json["error"], "invalid_request");
    assert!(json["error_description"].as_str().unwrap().contains("name"));
    assert!(json["ErrorModel"]["Message"].as_str().unwrap().contains("name"));
}

#[test]
fn unauthorized_json_follows_bitwarden_shape() {
    let json = AppError::Unauthorized.to_json();
    assert_eq!(json["error"], "invalid_token");
}

// ── Icon SSRF guard ─────────────────────────────────────────────────────

#[test]
fn icon_rejects_internal_ips() {
    assert!(!validate_icon_domain("https://127.0.0.1/icon.png"));
    assert!(!validate_icon_domain("http://localhost/favicon.ico"));
    assert!(!validate_icon_domain("https://10.0.0.1/icon"));
    assert!(!validate_icon_domain("https://192.168.1.1/favicon"));
    assert!(!validate_icon_domain("https://172.16.0.1/icon"));
}

#[test]
fn icon_allows_public_hosts() {
    assert!(validate_icon_domain("https://example.com/icon.png"));
    assert!(validate_icon_domain("https://github.com/favicon.ico"));
    assert!(validate_icon_domain("https://cdn.example.org/icons/google.svg"));
}

// ── Security stamp verification ─────────────────────────────────────────

#[test]
fn matching_stamps_pass() {
    assert!(verify_security_stamp(Some("abc123"), "abc123"));
}

#[test]
fn mismatched_stamps_fail() {
    assert!(!verify_security_stamp(Some("old-stamp"), "new-stamp"));
}

#[test]
fn missing_stamp_passes_for_backwards_compat() {
    assert!(verify_security_stamp(None, "any-stamp"));
}

// ── Notification framing ────────────────────────────────────────────────

#[test]
fn notification_msgpack_encodes() {
    let n = Notification::new(
        UpdateType::SyncCipherCreate,
        Some(serde_json::json!({"id": "abc123", "revisionDate": "2024-01-01T00:00:00Z"})),
    );
    let bytes = n.to_msgpack().expect("serialize");
    assert!(!bytes.is_empty());
}

#[test]
fn notification_without_payload_encodes() {
    let n = Notification::new(UpdateType::SyncLogOut, None);
    let bytes = n.to_msgpack().expect("serialize");
    assert!(!bytes.is_empty());
}

#[test]
fn all_update_types_are_distinct() {
    let types = [
        UpdateType::SyncCipherUpdate,
        UpdateType::SyncCipherCreate,
        UpdateType::SyncLoginDelete,
        UpdateType::SyncFolderDelete,
        UpdateType::SyncCiphers,
        UpdateType::SyncVault,
        UpdateType::SyncOrgKeys,
        UpdateType::SyncFolderCreate,
        UpdateType::SyncFolderUpdate,
        UpdateType::SyncCipherDelete,
        UpdateType::SyncSettings,
        UpdateType::SyncLogOut,
        UpdateType::SyncSendCreate,
        UpdateType::SyncSendUpdate,
        UpdateType::SyncSendDelete,
        UpdateType::AuthRequest,
        UpdateType::AuthRequestResponse,
    ];
    // All values are unique
    let values: Vec<u8> = types.iter().map(|t| *t as u8).collect();
    let mut deduped = values.clone();
    deduped.sort();
    deduped.dedup();
    assert_eq!(values.len(), deduped.len(), "update type values must be unique");
}

// ── Cipher model serde ───────────────────────────────────────────────────

#[test]
fn cipher_create_request_deserializes() {
    // Bitwarden type codes are integers (CipherType::Login == 1).
    let json = r#"{"type": 1, "name": "My Login", "login": {"username": "alice", "password": "secret"}}"#;
    let req: CipherCreateRequest = serde_json::from_str(json).expect("deserialize");
    assert_eq!(req.cipher_type, CipherType::Login);
    assert_eq!(req.name, "My Login");
    assert_eq!(req.login.unwrap().username.unwrap(), "alice");
}

#[test]
fn cipher_update_partial_changes() {
    let json = r#"{"name": "Updated", "favorite": true}"#;
    let req: CipherUpdateRequest = serde_json::from_str(json).expect("deserialize");
    assert_eq!(req.name.unwrap(), "Updated");
    assert!(req.favorite.unwrap());
    assert!(req.notes.is_none());
}

// ── Folder model serde ───────────────────────────────────────────────────

#[test]
fn folder_serializes_correct_casing() {
    let f = Folder {
        id: "folder-1".into(),
        name: "Work".into(),
        revision_date: chrono::Utc::now(),
    };
    let json = serde_json::to_string(&f).expect("serialize");
    assert!(json.contains("revisionDate"));
}

#[test]
fn folder_create_request_deserializes() {
    let json = r#"{"name": "Personal"}"#;
    let req: FolderCreateRequest = serde_json::from_str(json).expect("deserialize");
    assert_eq!(req.name, "Personal");
}

// ── Send model serde ─────────────────────────────────────────────────────

#[test]
fn send_create_request_with_password() {
    // SendType::Text == 0 on the wire.
    let json = r#"{"name": "Shared doc", "type": 0, "password": "pw123", "maxAccessCount": 5}"#;
    let req: SendCreateRequest = serde_json::from_str(json).expect("deserialize");
    assert_eq!(req.send_type, SendType::Text);
    assert_eq!(req.password.unwrap(), "pw123");
    assert_eq!(req.max_access_count.unwrap(), 5);
}

// ── Organization model ───────────────────────────────────────────────────

#[test]
fn org_create_request_deserializes() {
    let json = r#"{"name": "My Org", "billingEmail": "billing@example.com"}"#;
    let req: OrganizationCreateRequest = serde_json::from_str(json).expect("deserialize");
    assert_eq!(req.name, "My Org");
    assert_eq!(req.billing_email.unwrap(), "billing@example.com");
}

// ── User / Profile ──────────────────────────────────────────────────────

#[test]
fn prelogin_response_has_kdf_fields() {
    let resp = PreloginResponse {
        kdf: KdfType::Pbkdf2Sha256,
        kdf_iterations: 100_000,
        kdf_memory: None,
        kdf_parallelism: None,
    };
    let json = serde_json::to_string(&resp).expect("serialize");
    // KdfType serializes as its integer code (Pbkdf2Sha256 == 0), Bitwarden-style.
    assert!(json.contains("\"kdf\":0"), "json: {json}");
}

#[test]
fn register_request_accepts_optional_fields() {
    let json = r#"{"email": "a@b.com", "name": "Alice", "masterPasswordHash": "hash123"}"#;
    let req: RegisterRequest = serde_json::from_str(json).expect("deserialize");
    assert_eq!(req.email, "a@b.com");
    assert_eq!(req.master_password_hash, "hash123");
    assert!(req.kdf.is_none());
}

#[test]
fn update_profile_partial() {
    let json = r#"{"name": "New Name"}"#;
    let req: UpdateProfileRequest = serde_json::from_str(json).expect("deserialize");
    assert_eq!(req.name.unwrap(), "New Name");
    assert!(req.master_password_hint.is_none());
}

// ── Validation error ────────────────────────────────────────────────────

#[test]
fn validation_error_format() {
    let ve = ValidationError::new("Name is required", "cipher");
    let json = serde_json::to_string(&ve).expect("serialize");
    assert!(json.contains("Name is required"));
    assert!(json.contains("cipher"));
}

// ── BitwardenClaims ─────────────────────────────────────────────────────

#[test]
fn bitwarden_claims_round_trip() {
    let claims = BitwardenClaims {
        sstamp: Some("stamp-123".into()),
        orgowner: None,
        orgadmin: Some("org-1".into()),
        orguser: None,
        orgmanager: None,
        premium: Some(true),
        device: Some("device-abc".into()),
    };
    let json = serde_json::to_string(&claims).expect("serialize");
    let back: BitwardenClaims = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.sstamp.unwrap(), "stamp-123");
    assert_eq!(back.orgadmin.unwrap(), "org-1");
    assert!(back.premium.unwrap());
    assert!(back.orgowner.is_none());
}
