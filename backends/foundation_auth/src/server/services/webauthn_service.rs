//! WebAuthn/FIDO2 service — passkey registration and authentication.
//!
//! When `server-native` feature is enabled: wraps `webauthn-rs` for real
//! cryptographic operations. Otherwise: returns "not available" errors.

use std::sync::Arc;

use foundation_db::{AuthStore, KeyValueStore, PasskeyStore, StoredPasskey};

use serde::{Deserialize, Serialize};

use super::super::config::IdpConfig;
use super::super::models::Passkey;
use super::super::storage::StorageOpError;

#[cfg(feature = "server-native")]
use webauthn_rs::prelude::*;
#[cfg(feature = "server-native")]
use base64::engine::Engine;
#[cfg(feature = "server-native")]
use webauthn_rs::prelude::{PublicKeyCredential, RegisterPublicKeyCredential};

// ─── Public request/response types (always compiled) ─────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebAuthnRegistrationOptions {
    #[serde(rename = "publicKey")]
    pub public_key: serde_json::Value,
    pub session: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebAuthnAuthOptions {
    #[serde(rename = "publicKey")]
    pub public_key: serde_json::Value,
    pub session: String,
}

#[derive(Debug, Deserialize)]
pub struct WebAuthnRegisterFinishRequest {
    pub session: Option<String>,
    pub id: Option<String>,
    pub response: Option<serde_json::Value>,
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WebAuthnAuthFinishRequest {
    pub session: Option<String>,
    pub id: Option<String>,
    pub response: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct PasskeyLoginStartRequest {
    pub email: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PasskeyLoginFinishRequest {
    pub session: Option<String>,
    pub id: Option<String>,
    pub response: Option<serde_json::Value>,
}

// ─── Ceremony state stored in the shared KV cache ────────────────────────────

#[cfg(feature = "server-native")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RegState {
    user_id: String,
    reg: PasskeyRegistration,
}

#[cfg(feature = "server-native")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AuthState {
    user_id: String,
    auth: PasskeyAuthentication,
}

#[cfg(feature = "server-native")]
fn reg_key(session: &str) -> String {
    format!("wa:reg:{session}")
}

#[cfg(feature = "server-native")]
fn auth_key(session: &str) -> String {
    format!("wa:auth:{session}")
}

// ─── Service ──────────────────────────────────────────────────────────────────

/// WebAuthn/FIDO2 service for passkey registration and authentication.
///
/// Ceremony state (registration/auth challenges) is stored in the shared
/// `KeyValueStore` cache, so the issuing endpoint and the verifying endpoint
/// — even if served by different handler instances or nodes — see the same
/// state. The cache key is scoped by session and consumed (deleted) on finish.
#[cfg(feature = "server-native")]
pub struct WebAuthnService<S: AuthStore, KV: KeyValueStore> {
    config: Arc<IdpConfig>,
    store: Arc<S>,
    cache: KV,
    webauthn: webauthn_rs::Webauthn,
}

#[cfg(not(feature = "server-native"))]
pub struct WebAuthnService<S: AuthStore, KV: KeyValueStore> {
    config: Arc<IdpConfig>,
    store: Arc<S>,
    _cache: std::marker::PhantomData<KV>,
}

impl<S: AuthStore, KV: KeyValueStore> WebAuthnService<S, KV> {
    #[must_use]
    pub fn new(config: Arc<IdpConfig>, store: Arc<S>, cache: KV) -> Self {
        #[cfg(feature = "server-native")]
        {
            let origin = url::Url::parse(&config.issuer_url)
                .expect("Invalid issuer URL for WebAuthn origin");
            let webauthn = WebauthnBuilder::new(&config.issuer_url, &origin)
                .expect("Invalid WebAuthn builder config")
                .build()
                .expect("Invalid WebAuthn config");
            Self { config, store, cache, webauthn }
        }
        #[cfg(not(feature = "server-native"))]
        {
            drop(cache);
            Self { config, store, _cache: std::marker::PhantomData }
        }
    }

    pub fn register_start(
        &self,
        user_id: &str,
        email: &str,
    ) -> Result<WebAuthnRegistrationOptions, StorageOpError> {
        #[cfg(feature = "server-native")]
        {
            let uid = uuid::Uuid::parse_str(user_id).unwrap_or_else(|_| uuid::Uuid::new_v4());
            let (challenge, reg) = self.webauthn
                .start_passkey_registration(uid, email, email, None)
                .map_err(|e| StorageOpError::Query(format!("WebAuthn register start: {e}")))?;

            let session = uuid::Uuid::new_v4().to_string();
            self.cache.set(&reg_key(&session), RegState {
                user_id: user_id.to_string(),
                reg,
            }).map_err(|e| StorageOpError::Query(format!("cache set: {e}")))?;

            let public_key = serde_json::to_value(&challenge)
                .unwrap_or_else(|_| serde_json::json!({}));

            Ok(WebAuthnRegistrationOptions { public_key, session })
        }
        #[cfg(not(feature = "server-native"))]
        {
            let _ = (user_id, email);
            Err(StorageOpError::Query("WebAuthn not available — compile with server-native feature".into()))
        }
    }

    pub fn register_finish(
        &self,
        req: &WebAuthnRegisterFinishRequest,
    ) -> Result<Passkey, StorageOpError> {
        #[cfg(feature = "server-native")]
        {
            let session_id = req.session.as_deref()
                .ok_or_else(|| StorageOpError::Query("Missing session".into()))?;

            let state: RegState = self.cache
                .get(&reg_key(session_id))
                .map_err(|e| StorageOpError::Query(format!("cache get: {e}")))?
                .ok_or_else(|| StorageOpError::Query("Invalid or expired session".into()))?;

            // Consume the state (one-time use to prevent replay)
            self.cache.delete(&reg_key(session_id))
                .map_err(|e| StorageOpError::Query(format!("cache delete: {e}")))?;

            let response_json = req.response.as_ref()
                .ok_or_else(|| StorageOpError::Query("Missing response".into()))?;
            let response: RegisterPublicKeyCredential = serde_json::from_value(response_json.clone())
                .map_err(|e| StorageOpError::Query(format!("Parse response: {e}")))?;

            let passkey = self.webauthn
                .finish_passkey_registration(&response, &state.reg)
                .map_err(|e| StorageOpError::Query(format!("Verify registration: {e}")))?;

            let cred_id_bytes = passkey.cred_id().as_slice().to_vec();
            let passkey_cbor = serde_cbor::to_vec(&passkey)
                .map_err(|e| StorageOpError::Query(format!("Serialize passkey: {e}")))?;

            let our_passkey = Passkey {
                id: uuid::Uuid::new_v4().to_string(),
                user_id: state.user_id,
                name: req.name.clone().unwrap_or_else(|| "Passkey".into()),
                credential_id: cred_id_bytes,
                credential_public_key: passkey_cbor,
                counter: 0,
                created_at: chrono::Utc::now().timestamp_millis(),
                last_used_at: None,
            };

            let stored = StoredPasskey {
                id: our_passkey.id.clone(),
                user_id: our_passkey.user_id.clone(),
                name: our_passkey.name.clone(),
                credential_id: our_passkey.credential_id.clone(),
                credential_public_key: our_passkey.credential_public_key.clone(),
                counter: our_passkey.counter,
                created_at: our_passkey.created_at,
                last_used_at: our_passkey.last_used_at,
            };
            self.store.as_ref().store_passkey(&stored)
                .map_err(|e| StorageOpError::Query(e))?;

            Ok(our_passkey)
        }
        #[cfg(not(feature = "server-native"))]
        {
            let _ = req;
            Err(StorageOpError::Query("WebAuthn not available — compile with server-native feature".into()))
        }
    }

    pub fn auth_start(
        &self,
        user_id: &str,
    ) -> Result<WebAuthnAuthOptions, StorageOpError> {
        #[cfg(feature = "server-native")]
        {
            let stored_passkeys = self.store.as_ref().find_passkeys_by_user(user_id)
                .map_err(|e| StorageOpError::Query(e))?;

            let webauthn_passkeys: Vec<webauthn_rs::prelude::Passkey> = stored_passkeys.iter()
                .filter_map(|pk| {
                    serde_cbor::from_slice::<webauthn_rs::prelude::Passkey>(&pk.credential_public_key).ok()
                })
                .collect();

            let (challenge, auth) = self.webauthn
                .start_passkey_authentication(&webauthn_passkeys)
                .map_err(|e| StorageOpError::Query(format!("WebAuthn auth start: {e}")))?;

            let session = uuid::Uuid::new_v4().to_string();
            self.cache.set(&auth_key(&session), AuthState {
                user_id: user_id.to_string(),
                auth,
            }).map_err(|e| StorageOpError::Query(format!("cache set: {e}")))?;

            let public_key = serde_json::to_value(&challenge)
                .unwrap_or_else(|_| serde_json::json!({}));

            Ok(WebAuthnAuthOptions { public_key, session })
        }
        #[cfg(not(feature = "server-native"))]
        {
            let _ = user_id;
            Err(StorageOpError::Query("WebAuthn not available — compile with server-native feature".into()))
        }
    }

    pub fn auth_finish(
        &self,
        req: &WebAuthnAuthFinishRequest,
    ) -> Result<String, StorageOpError> {
        #[cfg(feature = "server-native")]
        {
            let session_id = req.session.as_deref()
                .ok_or_else(|| StorageOpError::Query("Missing session".into()))?;

            let state: AuthState = self.cache
                .get(&auth_key(session_id))
                .map_err(|e| StorageOpError::Query(format!("cache get: {e}")))?
                .ok_or_else(|| StorageOpError::Query("Invalid or expired session".into()))?;

            // Consume the state (one-time use to prevent replay)
            self.cache.delete(&auth_key(session_id))
                .map_err(|e| StorageOpError::Query(format!("cache delete: {e}")))?;

            let response_json = req.response.as_ref()
                .ok_or_else(|| StorageOpError::Query("Missing response".into()))?;
            let response: PublicKeyCredential = serde_json::from_value(response_json.clone())
                .map_err(|e| StorageOpError::Query(format!("Parse response: {e}")))?;

            let cred_id = response.raw_id.clone();
            let passkey = self.store.as_ref().find_passkey_by_credential_id(&cred_id)
                .map_err(|e| StorageOpError::Query(e))?
                .ok_or_else(|| StorageOpError::NotFound("Passkey not found".into()))?;

            let wa_passkey: webauthn_rs::prelude::Passkey =
                serde_cbor::from_slice(&passkey.credential_public_key)
                    .map_err(|e| StorageOpError::Parse(format!("Deserialize passkey: {e}")))?;

            let auth_result = self.webauthn
                .finish_passkey_authentication(&response, &state.auth)
                .map_err(|e| StorageOpError::Query(format!("Verify authentication: {e}")))?;

            if auth_result.needs_update() {
                self.store.as_ref().update_passkey_counter(&passkey.id, auth_result.counter())
                    .map_err(|e| StorageOpError::Query(e))?;
            }

            Ok(state.user_id)
        }
        #[cfg(not(feature = "server-native"))]
        {
            let _ = req;
            Err(StorageOpError::Query("WebAuthn not available — compile with server-native feature".into()))
        }
    }

    pub fn passkey_login_start(
        &self,
        email: &str,
    ) -> Result<WebAuthnAuthOptions, StorageOpError> {
        #[cfg(feature = "server-native")]
        {
            let (user_id, _has_password) = self.store.as_ref().find_user_by_email(email)
                .map_err(|e| StorageOpError::Query(e))?
                .ok_or_else(|| StorageOpError::NotFound(format!("No user with email: {email}")))?;
            self.auth_start(&user_id)
        }
        #[cfg(not(feature = "server-native"))]
        {
            let _ = email;
            Err(StorageOpError::Query("WebAuthn not available — compile with server-native feature".into()))
        }
    }

    pub fn passkey_login_finish(
        &self,
        req: &PasskeyLoginFinishRequest,
    ) -> Result<String, StorageOpError> {
        let auth_req = WebAuthnAuthFinishRequest {
            session: req.session.clone(),
            id: req.id.clone(),
            response: req.response.clone(),
        };
        self.auth_finish(&auth_req)
    }

    pub fn delete_passkey(&self, passkey_id: &str) -> Result<(), StorageOpError> {
        self.store.as_ref().delete_passkey(passkey_id)
            .map_err(|e| StorageOpError::Query(e))
    }

    pub fn rename_passkey(&self, passkey_id: &str, name: &str) -> Result<(), StorageOpError> {
        self.store.as_ref().update_passkey_name(passkey_id, name)
            .map_err(|e| StorageOpError::Query(e))
    }

    pub fn list_passkeys(&self, user_id: &str) -> Result<Vec<Passkey>, StorageOpError> {
        let stored = self.store.as_ref().find_passkeys_by_user(user_id)
            .map_err(|e| StorageOpError::Query(e))?;
        Ok(stored.into_iter().map(|sk| Passkey {
            id: sk.id,
            user_id: sk.user_id,
            name: sk.name,
            credential_id: sk.credential_id,
            credential_public_key: sk.credential_public_key,
            counter: sk.counter,
            created_at: sk.created_at,
            last_used_at: sk.last_used_at,
        }).collect())
    }
}
