//! Storage-backed operations for the IdP server.

use std::sync::Arc;

use foundation_db::core::storage_provider::{DataValue, QueryStore, SqlRow};

use super::models::{AuthorizationCode, DeviceCode, OAuthClient, Passkey, RefreshToken, TosAcceptance, User};

#[derive(Debug)]
pub enum StorageOpError {
    Query(String),
    NotFound(String),
    Parse(String),
}

impl core::fmt::Display for StorageOpError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Query(s) => write!(f, "Storage query error: {s}"),
            Self::NotFound(s) => write!(f, "Not found: {s}"),
            Self::Parse(s) => write!(f, "Parse error: {s}"),
        }
    }
}

impl std::error::Error for StorageOpError {}

pub fn find_client_by_id(
    store: &dyn QueryStore,
    client_id: &str,
) -> Result<Option<OAuthClient>, StorageOpError> {
    let sql = "SELECT id, name, client_secret_hash, redirect_uris, grant_types, scopes, is_public, created_at FROM oauth_clients WHERE id = ?";
    let mut stream = store
        .query(sql, &[DataValue::Text(client_id.to_string())])
        .map_err(|e| StorageOpError::Query(e.to_string()))?;
    let row = match collect_one_row(&mut stream)? {
        Some(r) => r,
        None => return Ok(None),
    };
    Ok(Some(parse_client_row(&row)?))
}

pub fn store_auth_code(
    store: &dyn QueryStore,
    code: &AuthorizationCode,
) -> Result<(), StorageOpError> {
    let sql = "INSERT INTO authorization_codes (code, user_id, client_id, redirect_uri, code_challenge, scope, nonce, expires_at, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)";
    store
        .execute(
            sql,
            &[
                DataValue::Text(code.code.clone()),
                DataValue::Text(code.user_id.clone()),
                DataValue::Text(code.client_id.clone()),
                DataValue::Text(code.redirect_uri.clone()),
                DataValue::Text(code.code_challenge.clone().unwrap_or_default()),
                DataValue::Text(code.scope.clone()),
                DataValue::Text(code.nonce.clone().unwrap_or_default()),
                DataValue::Integer(code.expires_at),
                DataValue::Integer(code.created_at),
            ],
        )
        .map_err(|e| StorageOpError::Query(e.to_string()))?;
    Ok(())
}

pub fn find_auth_code(
    store: &dyn QueryStore,
    code: &str,
) -> Result<Option<AuthorizationCode>, StorageOpError> {
    let sql = "SELECT code, user_id, client_id, redirect_uri, code_challenge, scope, nonce, expires_at, created_at FROM authorization_codes WHERE code = ?";
    let mut stream = store
        .query(sql, &[DataValue::Text(code.to_string())])
        .map_err(|e| StorageOpError::Query(e.to_string()))?;
    let row = match collect_one_row(&mut stream)? {
        Some(r) => r,
        None => return Ok(None),
    };
    Ok(Some(parse_auth_code_row(&row)?))
}

pub fn delete_auth_code(store: &dyn QueryStore, code: &str) -> Result<(), StorageOpError> {
    let sql = "DELETE FROM authorization_codes WHERE code = ?";
    store
        .execute(sql, &[DataValue::Text(code.to_string())])
        .map_err(|e| StorageOpError::Query(e.to_string()))?;
    Ok(())
}

pub fn store_refresh_token(
    store: &dyn QueryStore,
    token: &RefreshToken,
) -> Result<(), StorageOpError> {
    let sql = "INSERT INTO refresh_tokens (id, user_id, client_id, token_hash, expires_at, rotated_at, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)";
    store
        .execute(
            sql,
            &[
                DataValue::Text(token.id.clone()),
                DataValue::Text(token.user_id.clone()),
                DataValue::Text(token.client_id.clone()),
                DataValue::Text(token.token_hash.clone()),
                DataValue::Integer(token.expires_at),
                DataValue::Integer(token.rotated_at.unwrap_or(0)),
                DataValue::Integer(token.created_at),
            ],
        )
        .map_err(|e| StorageOpError::Query(e.to_string()))?;
    Ok(())
}

pub fn find_refresh_token_by_hash(
    store: &dyn QueryStore,
    token_hash: &str,
) -> Result<Option<RefreshToken>, StorageOpError> {
    let sql = "SELECT id, user_id, client_id, token_hash, expires_at, rotated_at, created_at FROM refresh_tokens WHERE token_hash = ?";
    let mut stream = store
        .query(sql, &[DataValue::Text(token_hash.to_string())])
        .map_err(|e| StorageOpError::Query(e.to_string()))?;
    let row = match collect_one_row(&mut stream)? {
        Some(r) => r,
        None => return Ok(None),
    };
    Ok(Some(parse_refresh_token_row(&row)?))
}

pub fn mark_refresh_token_rotated(
    store: &dyn QueryStore,
    token_hash: &str,
) -> Result<(), StorageOpError> {
    let now = chrono::Utc::now().timestamp_millis();
    let sql = "UPDATE refresh_tokens SET rotated_at = ? WHERE token_hash = ?";
    store
        .execute(
            sql,
            &[DataValue::Integer(now), DataValue::Text(token_hash.to_string())],
        )
        .map_err(|e| StorageOpError::Query(e.to_string()))?;
    Ok(())
}

pub fn store_device_code(
    store: &dyn QueryStore,
    code: &DeviceCode,
) -> Result<(), StorageOpError> {
    let sql = "INSERT INTO device_codes (device_code, user_code, client_id, scope, expires_at, interval_secs, user_id, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)";
    store
        .execute(
            sql,
            &[
                DataValue::Text(code.device_code.clone()),
                DataValue::Text(code.user_code.clone()),
                DataValue::Text(code.client_id.clone()),
                DataValue::Text(code.scope.clone()),
                DataValue::Integer(code.expires_at),
                DataValue::Integer(code.interval as i64),
                DataValue::Text(code.user_id.clone().unwrap_or_default()),
                DataValue::Integer(code.created_at),
            ],
        )
        .map_err(|e| StorageOpError::Query(e.to_string()))?;
    Ok(())
}

pub fn find_device_code(
    store: &dyn QueryStore,
    device_code: &str,
) -> Result<Option<DeviceCode>, StorageOpError> {
    let sql = "SELECT device_code, user_code, client_id, scope, expires_at, interval_secs, user_id, created_at FROM device_codes WHERE device_code = ?";
    let mut stream = store
        .query(sql, &[DataValue::Text(device_code.to_string())])
        .map_err(|e| StorageOpError::Query(e.to_string()))?;
    let row = match collect_one_row(&mut stream)? {
        Some(r) => r,
        None => return Ok(None),
    };
    Ok(Some(parse_device_code_row(&row)?))
}

pub fn approve_device_code(
    store: &dyn QueryStore,
    device_code: &str,
    user_id: &str,
) -> Result<(), StorageOpError> {
    let sql = "UPDATE device_codes SET user_id = ? WHERE device_code = ?";
    store
        .execute(
            sql,
            &[DataValue::Text(user_id.to_string()), DataValue::Text(device_code.to_string())],
        )
        .map_err(|e| StorageOpError::Query(e.to_string()))?;
    Ok(())
}

// -- Row parsing --

fn parse_client_row(row: &SqlRow) -> Result<OAuthClient, StorageOpError> {
    let redirect_uris: String = row.get_by_name("redirect_uris").map_err(parse_err)?;
    let grant_types: String = row.get_by_name("grant_types").map_err(parse_err)?;
    let scopes: String = row.get_by_name("scopes").map_err(parse_err)?;
    Ok(OAuthClient {
        id: row.get_by_name("id").map_err(parse_err)?,
        name: row.get_by_name("name").map_err(parse_err)?,
        client_secret_hash: row.get_by_name("client_secret_hash").map_err(parse_err)?,
        redirect_uris: parse_json_vec(&redirect_uris)?,
        grant_types: parse_json_vec(&grant_types)?,
        scopes: parse_json_vec(&scopes)?,
        is_public: row.get_by_name("is_public").map_err(parse_err)?,
        created_at: row.get_by_name("created_at").map_err(parse_err)?,
    })
}

fn parse_auth_code_row(row: &SqlRow) -> Result<AuthorizationCode, StorageOpError> {
    let code_challenge: String = row.get_by_name("code_challenge").map_err(parse_err)?;
    let nonce: String = row.get_by_name("nonce").map_err(parse_err)?;
    Ok(AuthorizationCode {
        code: row.get_by_name("code").map_err(parse_err)?,
        user_id: row.get_by_name("user_id").map_err(parse_err)?,
        client_id: row.get_by_name("client_id").map_err(parse_err)?,
        redirect_uri: row.get_by_name("redirect_uri").map_err(parse_err)?,
        code_challenge: if code_challenge.is_empty() { None } else { Some(code_challenge) },
        scope: row.get_by_name("scope").map_err(parse_err)?,
        nonce: if nonce.is_empty() { None } else { Some(nonce) },
        expires_at: row.get_by_name("expires_at").map_err(parse_err)?,
        created_at: row.get_by_name("created_at").map_err(parse_err)?,
    })
}

fn parse_refresh_token_row(row: &SqlRow) -> Result<RefreshToken, StorageOpError> {
    let rotated_at: i64 = row.get_by_name("rotated_at").map_err(parse_err)?;
    Ok(RefreshToken {
        id: row.get_by_name("id").map_err(parse_err)?,
        user_id: row.get_by_name("user_id").map_err(parse_err)?,
        client_id: row.get_by_name("client_id").map_err(parse_err)?,
        token_hash: row.get_by_name("token_hash").map_err(parse_err)?,
        expires_at: row.get_by_name("expires_at").map_err(parse_err)?,
        rotated_at: if rotated_at == 0 { None } else { Some(rotated_at) },
        created_at: row.get_by_name("created_at").map_err(parse_err)?,
    })
}

fn parse_device_code_row(row: &SqlRow) -> Result<DeviceCode, StorageOpError> {
    let user_id: String = row.get_by_name("user_id").map_err(parse_err)?;
    Ok(DeviceCode {
        device_code: row.get_by_name("device_code").map_err(parse_err)?,
        user_code: row.get_by_name("user_code").map_err(parse_err)?,
        client_id: row.get_by_name("client_id").map_err(parse_err)?,
        scope: row.get_by_name("scope").map_err(parse_err)?,
        expires_at: row.get_by_name("expires_at").map_err(parse_err)?,
        interval: row.get_by_name::<i64>("interval_secs").map_err(parse_err)? as u32,
        user_id: if user_id.is_empty() { None } else { Some(user_id) },
        created_at: row.get_by_name("created_at").map_err(parse_err)?,
    })
}

pub fn find_user_by_email(
    store: &dyn QueryStore,
    email: &str,
) -> Result<Option<User>, StorageOpError> {
    let sql = "SELECT id, email, username, password_hash, email_verified, email_verified_at, created_at, updated_at, metadata, failed_login_attempts, locked_until, deleted_at FROM users WHERE email = ?";
    let mut stream = store
        .query(sql, &[DataValue::Text(email.to_string())])
        .map_err(|e| StorageOpError::Query(e.to_string()))?;
    let row = match collect_one_row(&mut stream)? {
        Some(r) => r,
        None => return Ok(None),
    };
    Ok(Some(parse_user_row(&row)?))
}

/// Update a user's failed login attempts and lockout state.
pub fn update_user_lockout(
    store: &dyn QueryStore,
    user_id: &str,
    failed_attempts: u32,
    locked_until: Option<i64>,
) -> Result<(), StorageOpError> {
    let sql = "UPDATE users SET failed_login_attempts = ?, locked_until = ?, updated_at = ? WHERE id = ?";
    let now = chrono::Utc::now().timestamp_millis();
    let locked = locked_until.unwrap_or(0);
    store
        .execute(sql, &[
            DataValue::Integer(failed_attempts as i64),
            DataValue::Integer(locked),
            DataValue::Integer(now),
            DataValue::Text(user_id.to_string()),
        ])
        .map(|_| ())
        .map_err(|e| StorageOpError::Query(e.to_string()))
}

/// Create a new user in the database.
pub fn create_user(
    store: &dyn QueryStore,
    user: &User,
) -> Result<(), StorageOpError> {
    let sql = "INSERT INTO users (id, email, username, password_hash, email_verified, email_verified_at, created_at, updated_at, metadata, failed_login_attempts, locked_until, deleted_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 0, NULL, NULL)";
    let now = chrono::Utc::now().timestamp_millis();
    let email_verified_at = if user.email_verified { Some(now) } else { None };
    let metadata = serde_json::to_string(&user.metadata).unwrap_or_else(|_| "null".into());
    store
        .execute(sql, &[
            DataValue::Text(user.id.clone()),
            DataValue::Text(user.email.clone()),
            DataValue::Text(user.username.clone().unwrap_or_default()),
            DataValue::Text(user.password_hash.clone().unwrap_or_default()),
            DataValue::Integer(if user.email_verified { 1 } else { 0 }),
            DataValue::Integer(email_verified_at.unwrap_or(0)),
            DataValue::Integer(now),
            DataValue::Integer(now),
            DataValue::Text(metadata),
        ])
        .map(|_| ())
        .map_err(|e| StorageOpError::Query(e.to_string()))
}

fn parse_user_row(row: &SqlRow) -> Result<User, StorageOpError> {
    let password_hash: String = row.get_by_name("password_hash").map_err(parse_err)?;
    let metadata: String = row.get_by_name("metadata").map_err(parse_err)?;
    let username_val: String = row.get_by_name("username").map_err(parse_err)?;
    let email_verified_at_val: i64 = row.get_by_name("email_verified_at").map_err(parse_err)?;
    let locked_until_val: i64 = row.get_by_name("locked_until").map_err(parse_err)?;
    let deleted_at_val: i64 = row.get_by_name("deleted_at").map_err(parse_err)?;
    let failed_login_attempts: i64 = row.get_by_name("failed_login_attempts").map_err(parse_err)?;
    Ok(User {
        id: row.get_by_name("id").map_err(parse_err)?,
        email: row.get_by_name("email").map_err(parse_err)?,
        username: if username_val.is_empty() { None } else { Some(username_val) },
        password_hash: if password_hash.is_empty() { None } else { Some(password_hash) },
        email_verified: row.get_by_name("email_verified").map_err(parse_err)?,
        email_verified_at: if email_verified_at_val == 0 { None } else { Some(email_verified_at_val) },
        created_at: row.get_by_name("created_at").map_err(parse_err)?,
        updated_at: row.get_by_name("updated_at").map_err(parse_err)?,
        metadata: if metadata.is_empty() { None } else { serde_json::from_str(&metadata).ok() },
        failed_login_attempts: failed_login_attempts as u32,
        locked_until: if locked_until_val == 0 { None } else { Some(locked_until_val) },
        deleted_at: if deleted_at_val == 0 { None } else { Some(deleted_at_val) },
    })
}

fn parse_json_vec<T: serde::de::DeserializeOwned>(value: &str) -> Result<Vec<T>, StorageOpError> {
    serde_json::from_str(value).map_err(|e| StorageOpError::Parse(e.to_string()))
}

fn collect_one_row<'a>(
    stream: &mut foundation_db::core::storage_provider::StorageItemStream<'a, SqlRow>,
) -> Result<Option<SqlRow>, StorageOpError> {
    use foundation_core::valtron::Stream;
    for item in stream {
        match item {
            Stream::Next(Ok(row)) => return Ok(Some(row)),
            Stream::Next(Err(e)) => return Err(StorageOpError::Query(e.to_string())),
            Stream::Ignore | Stream::Init | Stream::Delayed(_) | Stream::Pending(_) | Stream::Wait | Stream::Spread(_) => {}
        }
    }
    Ok(None)
}

fn parse_err(e: foundation_db::core::errors::StorageError) -> StorageOpError {
    StorageOpError::Parse(e.to_string())
}

// ─── Passkeys ────────────────────────────────────────────────────────────────

pub fn store_passkey(
    store: &dyn QueryStore,
    passkey: &Passkey,
) -> Result<(), StorageOpError> {
    let sql = "INSERT INTO passkeys (id, user_id, name, credential_id, credential_public_key, counter, created_at, last_used_at) VALUES (?, ?, ?, ?, ?, ?, ?, NULL)";
    store
        .execute(sql, &[
            DataValue::Text(passkey.id.clone()),
            DataValue::Text(passkey.user_id.clone()),
            DataValue::Text(passkey.name.clone()),
            DataValue::Blob(passkey.credential_id.clone()),
            DataValue::Blob(passkey.credential_public_key.clone()),
            DataValue::Integer(passkey.counter as i64),
            DataValue::Integer(passkey.created_at),
        ])
        .map(|_| ())
        .map_err(|e| StorageOpError::Query(e.to_string()))
}

pub fn find_passkeys_by_user(
    store: &dyn QueryStore,
    user_id: &str,
) -> Result<Vec<Passkey>, StorageOpError> {
    let sql = "SELECT id, user_id, name, credential_id, credential_public_key, counter, created_at, last_used_at FROM passkeys WHERE user_id = ?";
    let mut stream = store
        .query(sql, &[DataValue::Text(user_id.to_string())])
        .map_err(|e| StorageOpError::Query(e.to_string()))?;
    let mut results = Vec::new();
    for item in stream {
        match item {
            foundation_core::valtron::Stream::Next(Ok(row)) => {
                results.push(parse_passkey_row(&row)?);
            }
            foundation_core::valtron::Stream::Next(Err(e)) => {
                return Err(StorageOpError::Query(e.to_string()));
            }
            _ => {}
        }
    }
    Ok(results)
}

pub fn find_passkey_by_id(
    store: &dyn QueryStore,
    passkey_id: &str,
) -> Result<Option<Passkey>, StorageOpError> {
    let sql = "SELECT id, user_id, name, credential_id, credential_public_key, counter, created_at, last_used_at FROM passkeys WHERE id = ?";
    let mut stream = store
        .query(sql, &[DataValue::Text(passkey_id.to_string())])
        .map_err(|e| StorageOpError::Query(e.to_string()))?;
    match collect_one_row(&mut stream)? {
        Some(row) => Ok(Some(parse_passkey_row(&row)?)),
        None => Ok(None),
    }
}

pub fn find_passkey_by_credential_id(
    store: &dyn QueryStore,
    credential_id: &[u8],
) -> Result<Option<Passkey>, StorageOpError> {
    let sql = "SELECT id, user_id, name, credential_id, credential_public_key, counter, created_at, last_used_at FROM passkeys WHERE credential_id = ?";
    let mut stream = store
        .query(sql, &[DataValue::Blob(credential_id.to_vec())])
        .map_err(|e| StorageOpError::Query(e.to_string()))?;
    match collect_one_row(&mut stream)? {
        Some(row) => Ok(Some(parse_passkey_row(&row)?)),
        None => Ok(None),
    }
}

pub fn update_passkey_counter(
    store: &dyn QueryStore,
    passkey_id: &str,
    counter: u32,
) -> Result<(), StorageOpError> {
    let sql = "UPDATE passkeys SET counter = ?, last_used_at = ? WHERE id = ?";
    let now = chrono::Utc::now().timestamp_millis();
    store
        .execute(sql, &[
            DataValue::Integer(counter as i64),
            DataValue::Integer(now),
            DataValue::Text(passkey_id.to_string()),
        ])
        .map(|_| ())
        .map_err(|e| StorageOpError::Query(e.to_string()))
}

pub fn update_passkey_name(
    store: &dyn QueryStore,
    passkey_id: &str,
    name: &str,
) -> Result<(), StorageOpError> {
    let sql = "UPDATE passkeys SET name = ? WHERE id = ?";
    store
        .execute(sql, &[
            DataValue::Text(name.to_string()),
            DataValue::Text(passkey_id.to_string()),
        ])
        .map(|_| ())
        .map_err(|e| StorageOpError::Query(e.to_string()))
}

pub fn delete_passkey(
    store: &dyn QueryStore,
    passkey_id: &str,
) -> Result<(), StorageOpError> {
    let sql = "DELETE FROM passkeys WHERE id = ?";
    store
        .execute(sql, &[DataValue::Text(passkey_id.to_string())])
        .map(|_| ())
        .map_err(|e| StorageOpError::Query(e.to_string()))
}

fn parse_passkey_row(row: &SqlRow) -> Result<Passkey, StorageOpError> {
    let credential_id: Vec<u8> = row.get_by_name("credential_id").map_err(parse_err)?;
    let credential_public_key: Vec<u8> = row.get_by_name("credential_public_key").map_err(parse_err)?;
    let last_used_at_val: i64 = row.get_by_name("last_used_at").map_err(parse_err)?;
    Ok(Passkey {
        id: row.get_by_name("id").map_err(parse_err)?,
        user_id: row.get_by_name("user_id").map_err(parse_err)?,
        name: row.get_by_name("name").map_err(parse_err)?,
        credential_id,
        credential_public_key,
        counter: row.get_by_name::<i64>("counter").map_err(parse_err)? as u32,
        created_at: row.get_by_name("created_at").map_err(parse_err)?,
        last_used_at: if last_used_at_val == 0 { None } else { Some(last_used_at_val) },
    })
}

// ─── ToS Acceptances ────────────────────────────────────────────────────────

pub fn store_tos_acceptance(
    store: &dyn QueryStore,
    acceptance: &TosAcceptance,
) -> Result<(), StorageOpError> {
    let sql = "INSERT INTO tos_acceptances (user_id, tos_version, accepted_at, ip_address) VALUES (?, ?, ?, ?)";
    store
        .execute(sql, &[
            DataValue::Text(acceptance.user_id.clone()),
            DataValue::Text(acceptance.tos_version.clone()),
            DataValue::Integer(acceptance.accepted_at),
            DataValue::Text(acceptance.ip_address.clone().unwrap_or_default()),
        ])
        .map(|_| ())
        .map_err(|e| StorageOpError::Query(e.to_string()))
}

pub fn find_tos_acceptance(
    store: &dyn QueryStore,
    user_id: &str,
    tos_version: &str,
) -> Result<Option<TosAcceptance>, StorageOpError> {
    let sql = "SELECT user_id, tos_version, accepted_at, ip_address FROM tos_acceptances WHERE user_id = ? AND tos_version = ?";
    let mut stream = store
        .query(sql, &[
            DataValue::Text(user_id.to_string()),
            DataValue::Text(tos_version.to_string()),
        ])
        .map_err(|e| StorageOpError::Query(e.to_string()))?;
    match collect_one_row(&mut stream)? {
        Some(row) => Ok(Some(parse_tos_acceptance_row(&row)?)),
        None => Ok(None),
    }
}

pub fn find_latest_tos_acceptance(
    store: &dyn QueryStore,
    user_id: &str,
) -> Result<Option<TosAcceptance>, StorageOpError> {
    let sql = "SELECT user_id, tos_version, accepted_at, ip_address FROM tos_acceptances WHERE user_id = ? ORDER BY accepted_at DESC LIMIT 1";
    let mut stream = store
        .query(sql, &[DataValue::Text(user_id.to_string())])
        .map_err(|e| StorageOpError::Query(e.to_string()))?;
    match collect_one_row(&mut stream)? {
        Some(row) => Ok(Some(parse_tos_acceptance_row(&row)?)),
        None => Ok(None),
    }
}

fn parse_tos_acceptance_row(row: &SqlRow) -> Result<TosAcceptance, StorageOpError> {
    let ip_address: String = row.get_by_name("ip_address").map_err(parse_err)?;
    Ok(TosAcceptance {
        user_id: row.get_by_name("user_id").map_err(parse_err)?,
        tos_version: row.get_by_name("tos_version").map_err(parse_err)?,
        accepted_at: row.get_by_name("accepted_at").map_err(parse_err)?,
        ip_address: if ip_address.is_empty() { None } else { Some(ip_address) },
    })
}

#[derive(Clone)]
pub struct HandlerStorage {
    pub query_store: Arc<dyn QueryStore>,
}

impl HandlerStorage {
    pub fn new(query_store: Arc<dyn QueryStore>) -> Self {
        Self { query_store }
    }

    // Conversion helpers for the trait impls below.
    fn passkey_to_stored(pk: &Passkey) -> foundation_db::StoredPasskey {
        foundation_db::StoredPasskey {
            id: pk.id.clone(),
            user_id: pk.user_id.clone(),
            name: pk.name.clone(),
            credential_id: pk.credential_id.clone(),
            credential_public_key: pk.credential_public_key.clone(),
            counter: pk.counter,
            created_at: pk.created_at,
            last_used_at: pk.last_used_at,
        }
    }

    fn stored_to_passkey(sk: &foundation_db::StoredPasskey) -> Passkey {
        Passkey {
            id: sk.id.clone(),
            user_id: sk.user_id.clone(),
            name: sk.name.clone(),
            credential_id: sk.credential_id.clone(),
            credential_public_key: sk.credential_public_key.clone(),
            counter: sk.counter,
            created_at: sk.created_at,
            last_used_at: sk.last_used_at,
        }
    }

    fn tos_to_stored(ta: &TosAcceptance) -> foundation_db::StoredTosAcceptance {
        foundation_db::StoredTosAcceptance {
            user_id: ta.user_id.clone(),
            tos_version: ta.tos_version.clone(),
            accepted_at: ta.accepted_at,
            ip_address: ta.ip_address.clone(),
        }
    }

    fn stored_to_tos(st: &foundation_db::StoredTosAcceptance) -> TosAcceptance {
        TosAcceptance {
            user_id: st.user_id.clone(),
            tos_version: st.tos_version.clone(),
            accepted_at: st.accepted_at,
            ip_address: st.ip_address.clone(),
        }
    }
}

// ─── AuthStore trait impl ────────────────────────────────────────────────────

fn map_storage_err(e: StorageOpError) -> String {
    e.to_string()
}

impl foundation_db::PasskeyStore for HandlerStorage {
    fn store_passkey(&self, passkey: &foundation_db::StoredPasskey) -> Result<(), String> {
        let our_pk = Self::stored_to_passkey(passkey);
        store_passkey(self.query_store.as_ref(), &our_pk).map_err(map_storage_err)
    }

    fn find_passkeys_by_user(&self, user_id: &str) -> Result<Vec<foundation_db::StoredPasskey>, String> {
        find_passkeys_by_user(self.query_store.as_ref(), user_id)
            .map(|pks| pks.iter().map(Self::passkey_to_stored).collect())
            .map_err(map_storage_err)
    }

    fn find_passkey_by_id(&self, passkey_id: &str) -> Result<Option<foundation_db::StoredPasskey>, String> {
        find_passkey_by_id(self.query_store.as_ref(), passkey_id)
            .map(|opt| opt.as_ref().map(Self::passkey_to_stored))
            .map_err(map_storage_err)
    }

    fn find_passkey_by_credential_id(&self, credential_id: &[u8]) -> Result<Option<foundation_db::StoredPasskey>, String> {
        find_passkey_by_credential_id(self.query_store.as_ref(), credential_id)
            .map(|opt| opt.as_ref().map(Self::passkey_to_stored))
            .map_err(map_storage_err)
    }

    fn update_passkey_counter(&self, passkey_id: &str, counter: u32) -> Result<(), String> {
        update_passkey_counter(self.query_store.as_ref(), passkey_id, counter).map_err(map_storage_err)
    }

    fn update_passkey_name(&self, passkey_id: &str, name: &str) -> Result<(), String> {
        update_passkey_name(self.query_store.as_ref(), passkey_id, name).map_err(map_storage_err)
    }

    fn delete_passkey(&self, passkey_id: &str) -> Result<(), String> {
        delete_passkey(self.query_store.as_ref(), passkey_id).map_err(map_storage_err)
    }
}

impl foundation_db::TosStore for HandlerStorage {
    fn store_tos_acceptance(&self, acceptance: &foundation_db::StoredTosAcceptance) -> Result<(), String> {
        let our_ta = Self::stored_to_tos(acceptance);
        store_tos_acceptance(self.query_store.as_ref(), &our_ta).map_err(map_storage_err)
    }

    fn find_tos_acceptance(&self, user_id: &str, tos_version: &str) -> Result<Option<foundation_db::StoredTosAcceptance>, String> {
        find_tos_acceptance(self.query_store.as_ref(), user_id, tos_version)
            .map(|opt| opt.as_ref().map(Self::tos_to_stored))
            .map_err(map_storage_err)
    }

    fn find_latest_tos_acceptance(&self, user_id: &str) -> Result<Option<foundation_db::StoredTosAcceptance>, String> {
        find_latest_tos_acceptance(self.query_store.as_ref(), user_id)
            .map(|opt| opt.as_ref().map(Self::tos_to_stored))
            .map_err(map_storage_err)
    }
}
