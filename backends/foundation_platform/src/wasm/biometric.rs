//! Typed WASM wrapper for the `biometric` IPC — Fingerprint / Face ID.

use foundation_wasm::ipc::IpcError;
use super::dispatch_json;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct AuthArgs {
    /// Reason shown to the user (e.g. "Unlock your vault").
    pub reason: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct AuthResult {
    pub success: bool,
    pub error_reason: Option<String>,
}

pub struct BiometricAuth;

impl BiometricAuth {
    /// Prompt the user for biometric authentication.
    /// Supported on Android (fingerprint) and iOS (Face ID / Touch ID).
    /// Returns `ExecutionFailed("unsupported")` on desktop/web.
    pub fn authenticate(args: AuthArgs) -> Result<AuthResult, IpcError> {
        dispatch_json("biometric", "authenticate", args)
    }

    /// Check whether biometric hardware is available.
    pub fn is_available() -> Result<AuthResult, IpcError> {
        dispatch_json("biometric", "is_available", serde_json::json!({}))
    }
}
