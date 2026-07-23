//! WASM UI API client — HTTP functions for the auth server endpoints.

use alloc::string::String;

/// Login request sent to POST /auth/v1/oidc/authorize
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct LoginRequest {
    pub email: Option<String>,
    pub password: Option<String>,
    pub client_id: Option<String>,
    pub redirect_uri: Option<String>,
    pub totp: Option<String>,
}

/// Login response
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct LoginResponse {
    pub status: String,
    pub email: Option<String>,
    pub challenge_id: Option<String>,
    pub r#type: Option<String>,
    pub attempts_remaining: Option<u32>,
}

/// Registration request
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub preferred_username: Option<String>,
}

/// Password reset request
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ResetRequest {
    pub email: String,
    pub redirect_uri: Option<String>,
}

/// WebAuthn registration options
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct WebAuthnOptions {
    #[serde(rename = "publicKey")]
    pub public_key: Option<serde_json::Value>,
    pub session: String,
}

/// PoW challenge
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct PowChallenge {
    pub difficulty: u32,
    pub challenge: String,
    pub expires_in: u64,
}

/// PoW solve request
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct PowSolveRequest {
    pub challenge: String,
    pub solution: String,
}
