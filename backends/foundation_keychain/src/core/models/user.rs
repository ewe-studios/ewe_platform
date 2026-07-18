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
