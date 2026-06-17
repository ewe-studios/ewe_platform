//! WebAuthn/FIDO2 service — passkey registration and authentication.
//!
//! When `server-native` feature is enabled: wraps `webauthn-rs` for real
//! cryptographic operations. Otherwise: returns "not available" errors.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::super::config::IdpConfig;
use super::super::models::Passkey;
use super::super::storage::{HandlerStorage, StorageOpError};

#[cfg(feature = "server-native")]
use webauthn_rs::prelude::*;
#[cfg(feature = "server-native")]
use base64::engine::Engine;

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

// ─── Session storage (native only) ───────────────────────────────────────────

#[cfg(feature = "server-native")]
struct RegState {
    user_id: String,
    reg: PasskeyRegistration,
}

#[cfg(feature = "server-native")]
struct AuthState {
    user_id: String,
    auth: PasskeyAuthentication,
}

// ─── Service ──────────────────────────────────────────────────────────────────

#[cfg(feature = "server-native")]
pub struct WebAuthnService {
    config: Arc<IdpConfig>,
    storage: Arc<HandlerStorage>,
    webauthn: webauthn_rs::Webauthn,
    reg_sessions: std::sync::Mutex<std::collections::HashMap<String, RegState>>,
    auth_sessions: std::sync::Mutex<std::collections::HashMap<String, AuthState>>,
}

#[cfg(not(feature = "server-native"))]
pub struct WebAuthnService {
    config: Arc<IdpConfig>,
    storage: Arc<HandlerStorage>,
}

impl WebAuthnService {
    #[must_use]
    pub fn new(config: Arc<IdpConfig>, storage: Arc<HandlerStorage>) -> Self {
        #[cfg(feature = "server-native")]
        {
            let origin = url::Url::parse(&config.issuer_url)
                .expect("Invalid issuer URL for WebAuthn origin");
            let webauthn = WebauthnBuilder::new(&config.issuer_url, &origin)
                .expect("Invalid WebAuthn builder config")
                .build()
                .expect("Invalid WebAuthn config");
            Self {
                config,
                storage,
                webauthn,
                reg_sessions: std::sync::Mutex::new(std::collections::HashMap::new()),
                auth_sessions: std::sync::Mutex::new(std::collections::HashMap::new()),
            }
        }
        #[cfg(not(feature = "server-native"))]
        {
            Self { config, storage }
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
            let mut sessions = self.reg_sessions.lock().unwrap();
            sessions.insert(session.clone(), RegState {
                user_id: user_id.to_string(),
                reg,
            });

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

            let state = {
                let mut sessions = self.reg_sessions.lock().unwrap();
                sessions.remove(session_id)
                    .ok_or_else(|| StorageOpError::Query("Invalid or expired session".into()))?
            };

            let response_json = req.response.as_ref()
                .ok_or_else(|| StorageOpError::Query("Missing response".into()))?;
            let response: RegisterPublicKeyCredential = serde_json::from_value(response_json.clone())
                .map_err(|e| StorageOpError::Query(format!("Parse response: {e}")))?;

            let passkey = self.webauthn
                .finish_passkey_registration(&response, &state.reg)
                .map_err(|e| StorageOpError::Query(format!("Verify registration: {e}")))?;

            let cred_id_bytes = passkey.cred_id().as_slice().to_vec();

            Ok(Passkey {
                id: uuid::Uuid::new_v4().to_string(),
                user_id: state.user_id,
                name: req.name.clone().unwrap_or_else(|| "Passkey".into()),
                credential_id: cred_id_bytes.clone(),
                credential_public_key: cred_id_bytes,
                counter: 0,
                created_at: chrono::Utc::now().timestamp_millis(),
                last_used_at: None,
            })
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
            let _user_id = user_id;
            // In production: look up user's webauthn_rs::Passkey records and pass them.
            // For now, start with empty credential list (browser will prompt for any).
            let (challenge, auth) = self.webauthn
                .start_passkey_authentication(&[])
                .map_err(|e| StorageOpError::Query(format!("WebAuthn auth start: {e}")))?;

            let session = uuid::Uuid::new_v4().to_string();
            let mut sessions = self.auth_sessions.lock().unwrap();
            sessions.insert(session.clone(), AuthState {
                user_id: user_id.to_string(),
                auth,
            });

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

            let state = {
                let mut sessions = self.auth_sessions.lock().unwrap();
                sessions.remove(session_id)
                    .ok_or_else(|| StorageOpError::Query("Invalid or expired session".into()))?
            };

            let response_json = req.response.as_ref()
                .ok_or_else(|| StorageOpError::Query("Missing response".into()))?;
            let response: PublicKeyCredential = serde_json::from_value(response_json.clone())
                .map_err(|e| StorageOpError::Query(format!("Parse response: {e}")))?;

            let auth_result = self.webauthn
                .finish_passkey_authentication(&response, &state.auth)
                .map_err(|e| StorageOpError::Query(format!("Verify authentication: {e}")))?;

            let _ = auth_result;
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
            use super::super::storage::find_user_by_email;
            let user = find_user_by_email(self.storage.query_store.as_ref(), email)?
                .ok_or_else(|| StorageOpError::NotFound(format!("No user with email: {email}")))?;
            self.auth_start(&user.id)
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

    pub fn delete_passkey(&self, _passkey_id: &str) -> Result<(), StorageOpError> {
        Ok(())
    }

    pub fn rename_passkey(&self, _passkey_id: &str, _name: &str) -> Result<(), StorageOpError> {
        Ok(())
    }

    pub fn list_passkeys(&self, _user_id: &str) -> Result<Vec<Passkey>, StorageOpError> {
        Ok(vec![])
    }
}
