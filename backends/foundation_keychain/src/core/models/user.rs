//! User account models — prelogin, register, profile, KDF (spec-57, F008).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KdfType {
    Pbkdf2Sha256 = 0,
    Argon2id = 1,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreloginResponse {
    pub kdf: KdfType,
    pub kdf_iterations: i32,
    pub kdf_memory: Option<i32>,
    pub kdf_parallelism: Option<i32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserAccount {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
    pub email_verified: bool,
    pub culture: String,
    pub premium: bool,
    pub two_factor_enabled: bool,
    pub security_stamp: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterRequest {
    pub email: String,
    pub name: Option<String>,
    pub master_password_hash: String,
    pub master_password_hint: Option<String>,
    pub key: Option<String>,
    pub kdf: Option<KdfType>,
    pub kdf_iterations: Option<i32>,
    pub kdf_memory: Option<i32>,
    pub kdf_parallelism: Option<i32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileResponse {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
    pub email_verified: bool,
    pub culture: String,
    pub premium: bool,
    pub two_factor_enabled: bool,
    pub security_stamp: String,
    pub organizations: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasswordHintRequest {
    pub email: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProfileRequest {
    pub name: Option<String>,
    pub master_password_hint: Option<String>,
}

// ── Login / token response (POST /identity/connect/token) ───────────────────

/// The full Bitwarden token response returned by `connect/token`.
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub expires_in: i64,
    pub token_type: String,
    pub refresh_token: String,
    #[serde(rename = "Key")]
    pub key: Option<String>,
    #[serde(rename = "PrivateKey")]
    pub private_key: Option<String>,
    #[serde(rename = "Kdf")]
    pub kdf: i32,
    #[serde(rename = "KdfIterations")]
    pub kdf_iterations: i32,
    #[serde(rename = "KdfMemory", skip_serializing_if = "Option::is_none")]
    pub kdf_memory: Option<i32>,
    #[serde(rename = "KdfParallelism", skip_serializing_if = "Option::is_none")]
    pub kdf_parallelism: Option<i32>,
    #[serde(rename = "unofficialServer")]
    pub unofficial_server: bool,
    #[serde(rename = "UserDecryptionOptions")]
    pub user_decryption_options: UserDecryptionOptions,
    #[serde(rename = "TwoFactorToken", skip_serializing_if = "Option::is_none")]
    pub two_factor_token: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UserDecryptionOptions {
    #[serde(rename = "HasMasterPassword")]
    pub has_master_password: bool,
    #[serde(rename = "MasterPasswordUnlock")]
    pub master_password_unlock: Option<MasterPasswordUnlock>,
    #[serde(rename = "Object")]
    pub object: &'static str,
}

#[derive(Debug, Serialize)]
pub struct MasterPasswordUnlock {
    #[serde(rename = "Kdf")]
    pub kdf: MasterPasswordUnlockKdf,
    #[serde(rename = "MasterKeyEncryptedUserKey")]
    pub master_key_encrypted_user_key: String,
    #[serde(rename = "MasterKeyWrappedUserKey")]
    pub master_key_wrapped_user_key: String,
    /// Bitwarden derives the master key with the email as the PBKDF2 salt.
    #[serde(rename = "Salt")]
    pub salt: String,
}

#[derive(Debug, Serialize)]
pub struct MasterPasswordUnlockKdf {
    #[serde(rename = "KdfType")]
    pub kdf_type: i32,
    #[serde(rename = "Iterations")]
    pub iterations: i32,
    #[serde(rename = "Memory")]
    pub memory: Option<i32>,
    #[serde(rename = "Parallelism")]
    pub parallelism: Option<i32>,
}

/// The `POST /identity/connect/token` request (form-urlencoded).
#[derive(Debug, Deserialize, Default)]
pub struct TokenRequest {
    pub grant_type: String,
    pub username: Option<String>,
    #[serde(rename = "password")]
    pub master_password_hash: Option<String>,
    pub refresh_token: Option<String>,
    #[serde(rename = "deviceIdentifier")]
    pub device_identifier: Option<String>,
    #[serde(rename = "deviceName")]
    pub device_name: Option<String>,
    #[serde(rename = "deviceType")]
    pub device_type: Option<i32>,
}
