//! Password Authentication Client (native).
//!
//! Implements username/password login against an IdP endpoint using
//! `NativeHttpClient`. Supports MFA challenge flow.

use foundation_netio::http::NativeHttpClient;
use foundation_netio::shared::http::{SendSafeBody, SimpleHeader};
use serde::{Deserialize, Serialize};

use crate::ConfidentialText;

/// MFA type supported by the IdP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MfaType {
    /// TOTP one-time password.
    Totp,
    /// WebAuthn / FIDO2.
    WebAuthn,
}

impl MfaType {
    fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "webauthn" | "fido2" => MfaType::WebAuthn,
            _ => MfaType::Totp,
        }
    }
}

/// Login request body.
#[derive(Debug, Serialize)]
pub struct LoginRequest<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<&'a str>,
    pub password: &'a str,
}

/// Login response from IdP.
#[derive(Debug, Deserialize)]
pub struct LoginResponse {
    /// Authentication status string from the IdP.
    pub status: String,
    /// Session ID (present on authentication success).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// MFA challenge ID (present when MFA is required).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub challenge_id: Option<String>,
    /// MFA type required (present when MFA is required).
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub mfa_type_str: Option<String>,
    /// Whether MFA is required.
    #[serde(default)]
    pub mfa_required: bool,
    /// Remaining login attempts (present on invalid credentials).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempts_remaining: Option<u32>,
    /// Whether the account is locked.
    #[serde(default)]
    pub locked: bool,
}

/// Parsed login result.
#[derive(Debug)]
pub enum LoginResult {
    /// Authentication succeeded.
    Authenticated {
        session_id: String,
        mfa_required: bool,
    },
    /// MFA challenge required.
    MfaRequired {
        challenge_id: String,
        mfa_type: MfaType,
    },
    /// Credentials were invalid.
    InvalidCredentials {
        attempts_remaining: Option<u32>,
        locked: bool,
    },
}

/// Password authentication client.
#[derive(Clone)]
pub struct PasswordAuthClient {
    base_url: String,
}

impl PasswordAuthClient {
    /// Create a new password auth client.
    #[must_use]
    pub fn new(base_url: String) -> Self {
        Self { base_url }
    }

    /// Attempt login with email and password.
    ///
    /// # Errors
    ///
    /// Returns `PasswordAuthError` on connection failure, server error,
    /// or parse error.
    pub async fn login_email(
        &self,
        email: &str,
        password: &ConfidentialText,
    ) -> Result<LoginResult, PasswordAuthError> {
        self.do_login(LoginRequest {
            email: Some(email),
            username: None,
            password: &password.get(),
        })
        .await
    }

    /// Attempt login with username and password.
    ///
    /// # Errors
    ///
    /// Returns `PasswordAuthError` on connection failure, server error,
    /// or parse error.
    pub async fn login_username(
        &self,
        username: &str,
        password: &ConfidentialText,
    ) -> Result<LoginResult, PasswordAuthError> {
        self.do_login(LoginRequest {
            email: None,
            username: Some(username),
            password: &password.get(),
        })
        .await
    }

    /// Submit MFA code for an existing challenge.
    ///
    /// # Errors
    ///
    /// Returns `PasswordAuthError` on failure.
    pub async fn submit_mfa(
        &self,
        challenge_id: &str,
        code: &str,
    ) -> Result<LoginResult, PasswordAuthError> {
        let url = format!("{}/auth/v1/mfa", self.base_url);
        let body = serde_json::json!({
            "challenge_id": challenge_id,
            "code": code,
        });
        let body_str = serde_json::to_string(&body)
            .map_err(|e| PasswordAuthError::ParseError(e.to_string()))?;

        let resp = Self::post_json(&url, &body_str).await?;
        parse_login_response(resp).await
    }

    /// Internal: execute the login request.
    async fn do_login(
        &self,
        request: LoginRequest<'_>,
    ) -> Result<LoginResult, PasswordAuthError> {
        let url = format!("{}/auth/v1/login", self.base_url);
        let body = serde_json::to_string(&request)
            .map_err(|e| PasswordAuthError::ParseError(e.to_string()))?;

        let resp = Self::post_json(&url, &body).await?;
        parse_login_response(resp).await
    }

    /// POST JSON body and return response.
    async fn post_json(
        url: &str,
        body: &str,
    ) -> Result<serde_json::Value, PasswordAuthError> {
        let client = NativeHttpClient::from_system();
        let resp = client
            .post(url)
            .map_err(|e| PasswordAuthError::ConnectionFailed(e.to_string()))?
            .header(SimpleHeader::CONTENT_TYPE, "application/json")
            .body_text(body.to_string())
            .build_client()
            .map_err(|e| PasswordAuthError::ConnectionFailed(e.to_string()))?
            .send_async()
            .await
            .map_err(|e| PasswordAuthError::ConnectionFailed(e.to_string()))?;

        let status: usize = resp.get_status().into();

        if status == 423 {
            return Err(PasswordAuthError::AccountLocked);
        }

        if status == 401 {
            let body = extract_body(resp.get_body_ref());
            // Return the body as JSON — parse_login_response will handle it
            return serde_json::from_str(&body)
                .map_err(|e| PasswordAuthError::ParseError(e.to_string()));
        }

        if !resp.is_success() {
            let body = extract_body(resp.get_body_ref());
            return Err(PasswordAuthError::ServerError {
                status: status as u16,
                message: body,
            });
        }

        let body = extract_body(resp.get_body_ref());
        serde_json::from_str(&body)
            .map_err(|e| PasswordAuthError::ParseError(e.to_string()))
    }
}

/// Parse the raw JSON response into a LoginResult.
async fn parse_login_response(
    value: serde_json::Value,
) -> Result<LoginResult, PasswordAuthError> {
    let status = value
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    match status.as_str() {
        "authenticated" => Ok(LoginResult::Authenticated {
            session_id: value
                .get("session_id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            mfa_required: value.get("mfa_required").and_then(|v| v.as_bool()).unwrap_or(false),
        }),
        "mfa_required" => {
            let challenge_id = value
                .get("challenge_id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let mfa_type = value
                .get("type")
                .and_then(|v| v.as_str())
                .map(MfaType::from_str)
                .unwrap_or(MfaType::Totp);
            Ok(LoginResult::MfaRequired {
                challenge_id,
                mfa_type,
            })
        }
        "invalid_credentials" | "" => Ok(LoginResult::InvalidCredentials {
            attempts_remaining: value
                .get("attempts_remaining")
                .and_then(|v| v.as_u64())
                .map(|n| n as u32),
            locked: value.get("locked").and_then(|v| v.as_bool()).unwrap_or(false),
        }),
        other => Err(PasswordAuthError::ParseError(format!(
            "unknown login status: {other}"
        ))),
    }
}

fn extract_body(body: &SendSafeBody) -> String {
    match body {
        SendSafeBody::Text(t) => t.clone(),
        SendSafeBody::Bytes(b) => String::from_utf8_lossy(b).to_string(),
        _ => String::new(),
    }
}

/// Password authentication errors.
#[derive(Debug)]
pub enum PasswordAuthError {
    /// Connection to the IdP failed.
    ConnectionFailed(String),
    /// Server returned an error status.
    ServerError { status: u16, message: String },
    /// Response could not be parsed.
    ParseError(String),
    /// Account is locked due to too many failed attempts.
    AccountLocked,
}

impl core::fmt::Display for PasswordAuthError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            PasswordAuthError::ConnectionFailed(s) => {
                write!(f, "Password auth connection failed: {s}")
            }
            PasswordAuthError::ServerError { status, message } => {
                write!(f, "Password auth server error ({status}): {message}")
            }
            PasswordAuthError::ParseError(s) => {
                write!(f, "Password auth parse error: {s}")
            }
            PasswordAuthError::AccountLocked => write!(f, "Account locked"),
        }
    }
}

impl std::error::Error for PasswordAuthError {}
