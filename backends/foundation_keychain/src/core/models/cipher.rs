//! Cipher models — the core data type in Bitwarden (spec-57, F008).
//!
//! Ciphers represent stored credentials: logins, cards, identities, notes.
//! Each cipher belongs to a user or an organization collection.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Cipher type classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CipherType {
    Login = 1,
    SecureNote = 2,
    Card = 3,
    Identity = 4,
}

/// Secure note subtype.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SecureNoteType {
    Generic = 0,
}

/// A stored credential item.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cipher {
    pub id: String,
    pub organization_id: Option<String>,
    #[serde(rename = "type")]
    pub cipher_type: CipherType,
    pub name: String,
    pub notes: Option<String>,
    pub fields: Option<Vec<CipherField>>,
    /// Login-specific data (null for non-login types).
    pub login: Option<LoginData>,
    pub card: Option<CardData>,
    pub identity: Option<IdentityData>,
    pub secure_note: Option<SecureNoteData>,
    /// Encrypted data string (client-side encryption blob).
    pub data: Option<String>,
    pub favorite: bool,
    pub reprompt: CipherRepromptType,
    pub revision_date: DateTime<Utc>,
    pub creation_date: DateTime<Utc>,
    pub deleted_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CipherField {
    pub name: Option<String>,
    pub value: Option<String>,
    #[serde(rename = "type")]
    pub field_type: i32,
    pub linked_id: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginData {
    pub uri: Option<String>,
    pub uris: Option<Vec<LoginUri>>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub totp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginUri {
    pub uri: Option<String>,
    pub r#match: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardData {
    pub cardholder_name: Option<String>,
    pub brand: Option<String>,
    pub number: Option<String>,
    pub exp_month: Option<String>,
    pub exp_year: Option<String>,
    pub code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityData {
    pub title: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address1: Option<String>,
    pub address2: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecureNoteData {
    #[serde(rename = "type")]
    pub note_type: SecureNoteType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CipherRepromptType {
    None = 0,
    Password = 1,
}

// ── Request / Response DTOs ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CipherCreateRequest {
    #[serde(rename = "type")]
    pub cipher_type: CipherType,
    pub name: String,
    pub notes: Option<String>,
    pub fields: Option<Vec<CipherField>>,
    pub login: Option<LoginData>,
    pub card: Option<CardData>,
    pub identity: Option<IdentityData>,
    pub secure_note: Option<SecureNoteData>,
    pub data: Option<String>,
    pub favorite: Option<bool>,
    pub organization_id: Option<String>,
    pub collection_ids: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CipherUpdateRequest {
    pub name: Option<String>,
    pub notes: Option<String>,
    pub fields: Option<Vec<CipherField>>,
    pub login: Option<LoginData>,
    pub card: Option<CardData>,
    pub identity: Option<IdentityData>,
    pub secure_note: Option<SecureNoteData>,
    pub data: Option<String>,
    pub favorite: Option<bool>,
    pub folder_id: Option<String>,
}

impl Default for CipherRepromptType {
    fn default() -> Self {
        Self::None
    }
}
