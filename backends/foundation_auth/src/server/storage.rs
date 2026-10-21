//! Storage-backed operations for the IdP server.
//!
//! Every function here talks to an [`AsyncQueryStore`] (Turso/libsql natively,
//! D1 on Workers) and is awaited by the async handlers. Multi-row reads hand
//! back a lazily-pulled stream, so an unbounded scan can't run out of memory.

use std::sync::Arc;

use foundation_db::core::storage_provider::{AsyncQueryStore, AsyncQueryStream, DataValue, SqlRow};
use foundation_db::{
    AsyncAuthStore, AsyncKeyValueStore, AsyncPasskeyStore, AsyncStorageItemStream, AsyncTosStore,
    StorageError, StorageResult, StoredPasskey, StoredTosAcceptance,
};
use futures_lite::StreamExt;

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

// ─── Query helpers ───────────────────────────────────────────────────────────

fn query_err(e: StorageError) -> StorageOpError {
    StorageOpError::Query(e.to_string())
}

fn parse_err(e: StorageError) -> StorageOpError {
    StorageOpError::Parse(e.to_string())
}

/// Pull the first row from a query stream. Stops after one row instead of
/// draining the stream.
async fn first_row(mut stream: AsyncQueryStream) -> Result<Option<SqlRow>, StorageOpError> {
    match stream.next().await {
        Some(Ok(row)) => Ok(Some(row)),
        Some(Err(e)) => Err(query_err(e)),
        None => Ok(None),
    }
}

/// Run a query and return its first row.
async fn query_one(
    store: &dyn AsyncQueryStore,
    sql: &str,
    params: &[DataValue],
) -> Result<Option<SqlRow>, StorageOpError> {
    let stream = store.query_async(sql, params).await.map_err(query_err)?;
    first_row(stream).await
}

/// Run a statement, discarding the affected-row count.
async fn execute(
    store: &dyn AsyncQueryStore,
    sql: &str,
    params: &[DataValue],
) -> Result<(), StorageOpError> {
    store.execute_async(sql, params).await.map(|_| ()).map_err(query_err)
}

// ─── OAuth clients, codes, tokens ────────────────────────────────────────────

pub async fn find_client_by_id(
    store: &dyn AsyncQueryStore,
    client_id: &str,
) -> Result<Option<OAuthClient>, StorageOpError> {
    let sql = "SELECT id, name, client_secret_hash, redirect_uris, grant_types, scopes, is_public, created_at FROM oauth_clients WHERE id = ?";
    match query_one(store, sql, &[DataValue::Text(client_id.to_string())]).await? {
        Some(row) => Ok(Some(parse_client_row(&row)?)),
        None => Ok(None),
    }
}

pub async fn store_auth_code(
    store: &dyn AsyncQueryStore,
    code: &AuthorizationCode,
) -> Result<(), StorageOpError> {
    let sql = "INSERT INTO authorization_codes (code, user_id, client_id, redirect_uri, code_challenge, scope, nonce, expires_at, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)";
    execute(
        store,
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
    .await
}

pub async fn find_auth_code(
    store: &dyn AsyncQueryStore,
    code: &str,
) -> Result<Option<AuthorizationCode>, StorageOpError> {
    let sql = "SELECT code, user_id, client_id, redirect_uri, code_challenge, scope, nonce, expires_at, created_at FROM authorization_codes WHERE code = ?";
    match query_one(store, sql, &[DataValue::Text(code.to_string())]).await? {
        Some(row) => Ok(Some(parse_auth_code_row(&row)?)),
        None => Ok(None),
    }
}

pub async fn delete_auth_code(store: &dyn AsyncQueryStore, code: &str) -> Result<(), StorageOpError> {
    let sql = "DELETE FROM authorization_codes WHERE code = ?";
    execute(store, sql, &[DataValue::Text(code.to_string())]).await
}

pub async fn store_refresh_token(
    store: &dyn AsyncQueryStore,
    token: &RefreshToken,
) -> Result<(), StorageOpError> {
    let sql = "INSERT INTO refresh_tokens (id, user_id, client_id, token_hash, expires_at, rotated_at, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)";
    execute(
        store,
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
    .await
}

pub async fn find_refresh_token_by_hash(
    store: &dyn AsyncQueryStore,
    token_hash: &str,
) -> Result<Option<RefreshToken>, StorageOpError> {
    let sql = "SELECT id, user_id, client_id, token_hash, expires_at, rotated_at, created_at FROM refresh_tokens WHERE token_hash = ?";
    match query_one(store, sql, &[DataValue::Text(token_hash.to_string())]).await? {
        Some(row) => Ok(Some(parse_refresh_token_row(&row)?)),
        None => Ok(None),
    }
}

pub async fn mark_refresh_token_rotated(
    store: &dyn AsyncQueryStore,
    token_hash: &str,
) -> Result<(), StorageOpError> {
    let now = chrono::Utc::now().timestamp_millis();
    let sql = "UPDATE refresh_tokens SET rotated_at = ? WHERE token_hash = ?";
    execute(
        store,
        sql,
        &[DataValue::Integer(now), DataValue::Text(token_hash.to_string())],
    )
    .await
}

pub async fn store_device_code(
    store: &dyn AsyncQueryStore,
    code: &DeviceCode,
) -> Result<(), StorageOpError> {
    let sql = "INSERT INTO device_codes (device_code, user_code, client_id, scope, expires_at, interval_secs, user_id, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)";
    execute(
        store,
        sql,
        &[
            DataValue::Text(code.device_code.clone()),
            DataValue::Text(code.user_code.clone()),
            DataValue::Text(code.client_id.clone()),
            DataValue::Text(code.scope.clone()),
            DataValue::Integer(code.expires_at),
            DataValue::Integer(i64::from(code.interval)),
            DataValue::Text(code.user_id.clone().unwrap_or_default()),
            DataValue::Integer(code.created_at),
        ],
    )
    .await
}

pub async fn find_device_code(
    store: &dyn AsyncQueryStore,
    device_code: &str,
) -> Result<Option<DeviceCode>, StorageOpError> {
    let sql = "SELECT device_code, user_code, client_id, scope, expires_at, interval_secs, user_id, created_at FROM device_codes WHERE device_code = ?";
    match query_one(store, sql, &[DataValue::Text(device_code.to_string())]).await? {
        Some(row) => Ok(Some(parse_device_code_row(&row)?)),
        None => Ok(None),
    }
}

pub async fn approve_device_code(
    store: &dyn AsyncQueryStore,
    device_code: &str,
    user_id: &str,
) -> Result<(), StorageOpError> {
    let sql = "UPDATE device_codes SET user_id = ? WHERE device_code = ?";
    execute(
        store,
        sql,
        &[DataValue::Text(user_id.to_string()), DataValue::Text(device_code.to_string())],
    )
    .await
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

// ─── Users ───────────────────────────────────────────────────────────────────

pub async fn find_user_by_email(
    store: &dyn AsyncQueryStore,
    email: &str,
) -> Result<Option<User>, StorageOpError> {
    let sql = "SELECT id, email, username, password_hash, email_verified, email_verified_at, created_at, updated_at, metadata, failed_login_attempts, locked_until, deleted_at FROM users WHERE email = ?";
    match query_one(store, sql, &[DataValue::Text(email.to_string())]).await? {
        Some(row) => Ok(Some(parse_user_row(&row)?)),
        None => Ok(None),
    }
}

/// Update a user's failed login attempts and lockout state.
pub async fn update_user_lockout(
    store: &dyn AsyncQueryStore,
    user_id: &str,
    failed_attempts: u32,
    locked_until: Option<i64>,
) -> Result<(), StorageOpError> {
    let sql = "UPDATE users SET failed_login_attempts = ?, locked_until = ?, updated_at = ? WHERE id = ?";
    let now = chrono::Utc::now().timestamp_millis();
    let locked = locked_until.unwrap_or(0);
    execute(
        store,
        sql,
        &[
            DataValue::Integer(i64::from(failed_attempts)),
            DataValue::Integer(locked),
            DataValue::Integer(now),
            DataValue::Text(user_id.to_string()),
        ],
    )
    .await
}

/// Find a user by their ID.
pub async fn find_user_by_id(
    store: &dyn AsyncQueryStore,
    user_id: &str,
) -> Result<Option<User>, StorageOpError> {
    let sql = "SELECT id, email, username, password_hash, email_verified, email_verified_at, created_at, updated_at, metadata, failed_login_attempts, locked_until, deleted_at FROM users WHERE id = ?";
    match query_one(store, sql, &[DataValue::Text(user_id.to_string())]).await? {
        Some(row) => Ok(Some(parse_user_row(&row)?)),
        None => Ok(None),
    }
}

/// Update user profile fields (username only for now — email changes require re-verification).
pub async fn update_user_profile(
    store: &dyn AsyncQueryStore,
    user_id: &str,
    username: Option<&str>,
) -> Result<(), StorageOpError> {
    let username = username.unwrap_or("");
    let now = chrono::Utc::now().timestamp_millis();
    let sql = "UPDATE users SET username = ?, updated_at = ? WHERE id = ?";
    execute(
        store,
        sql,
        &[
            DataValue::Text(username.to_string()),
            DataValue::Integer(now),
            DataValue::Text(user_id.to_string()),
        ],
    )
    .await
}

/// Update a user's password hash.
pub async fn update_user_password(
    store: &dyn AsyncQueryStore,
    user_id: &str,
    new_hash: &str,
) -> Result<(), StorageOpError> {
    let now = chrono::Utc::now().timestamp_millis();
    let sql = "UPDATE users SET password_hash = ?, updated_at = ? WHERE id = ?";
    execute(
        store,
        sql,
        &[
            DataValue::Text(new_hash.to_string()),
            DataValue::Integer(now),
            DataValue::Text(user_id.to_string()),
        ],
    )
    .await
}

/// Soft-delete a user by setting `deleted_at`.
pub async fn soft_delete_user(store: &dyn AsyncQueryStore, user_id: &str) -> Result<(), StorageOpError> {
    let now = chrono::Utc::now().timestamp_millis();
    let sql = "UPDATE users SET deleted_at = ?, updated_at = ? WHERE id = ?";
    execute(
        store,
        sql,
        &[
            DataValue::Integer(now),
            DataValue::Integer(now),
            DataValue::Text(user_id.to_string()),
        ],
    )
    .await
}

/// Create a new user in the database.
///
/// # Errors
///
/// `Parse` if the user's metadata can't be serialized; `Query` on insert
/// failure.
pub async fn create_user(store: &dyn AsyncQueryStore, user: &User) -> Result<(), StorageOpError> {
    let sql = "INSERT INTO users (id, email, username, password_hash, email_verified, email_verified_at, created_at, updated_at, metadata, failed_login_attempts, locked_until, deleted_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 0, NULL, NULL)";
    let now = chrono::Utc::now().timestamp_millis();
    let email_verified_at = if user.email_verified { Some(now) } else { None };
    let metadata = serde_json::to_string(&user.metadata)
        .map_err(|e| StorageOpError::Parse(format!("user metadata: {e}")))?;
    execute(
        store,
        sql,
        &[
            DataValue::Text(user.id.clone()),
            DataValue::Text(user.email.clone()),
            DataValue::Text(user.username.clone().unwrap_or_default()),
            DataValue::Text(user.password_hash.clone().unwrap_or_default()),
            DataValue::Integer(i64::from(user.email_verified)),
            DataValue::Integer(email_verified_at.unwrap_or(0)),
            DataValue::Integer(now),
            DataValue::Integer(now),
            DataValue::Text(metadata),
        ],
    )
    .await
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
        metadata: if metadata.is_empty() {
            None
        } else {
            Some(serde_json::from_str(&metadata).map_err(|e| StorageOpError::Parse(format!("user metadata: {e}")))?)
        },
        failed_login_attempts: failed_login_attempts as u32,
        locked_until: if locked_until_val == 0 { None } else { Some(locked_until_val) },
        deleted_at: if deleted_at_val == 0 { None } else { Some(deleted_at_val) },
    })
}

fn parse_json_vec<T: serde::de::DeserializeOwned>(value: &str) -> Result<Vec<T>, StorageOpError> {
    serde_json::from_str(value).map_err(|e| StorageOpError::Parse(e.to_string()))
}

// ─── Passkeys ────────────────────────────────────────────────────────────────

pub async fn store_passkey(store: &dyn AsyncQueryStore, passkey: &Passkey) -> Result<(), StorageOpError> {
    let sql = "INSERT INTO passkeys (id, user_id, name, credential_id, credential_public_key, counter, created_at, last_used_at) VALUES (?, ?, ?, ?, ?, ?, ?, NULL)";
    execute(
        store,
        sql,
        &[
            DataValue::Text(passkey.id.clone()),
            DataValue::Text(passkey.user_id.clone()),
            DataValue::Text(passkey.name.clone()),
            DataValue::Blob(passkey.credential_id.clone()),
            DataValue::Blob(passkey.credential_public_key.clone()),
            DataValue::Integer(i64::from(passkey.counter)),
            DataValue::Integer(passkey.created_at),
        ],
    )
    .await
}

/// Stream a user's passkeys, one at a time (nothing is collected).
pub async fn find_passkeys_by_user(
    store: &dyn AsyncQueryStore,
    user_id: &str,
) -> Result<AsyncStorageItemStream<'static, StoredPasskey>, StorageOpError> {
    let sql = "SELECT id, user_id, name, credential_id, credential_public_key, counter, created_at, last_used_at FROM passkeys WHERE user_id = ?";
    let stream = store
        .query_async(sql, &[DataValue::Text(user_id.to_string())])
        .await
        .map_err(query_err)?;
    let mapped = stream.map(|row_res| {
        row_res.and_then(|row| {
            parse_passkey_row(&row)
                .map(|pk| passkey_to_stored(&pk))
                .map_err(op_to_storage_err)
        })
    });
    Ok(Box::pin(mapped))
}

pub async fn find_passkey_by_id(
    store: &dyn AsyncQueryStore,
    passkey_id: &str,
) -> Result<Option<Passkey>, StorageOpError> {
    let sql = "SELECT id, user_id, name, credential_id, credential_public_key, counter, created_at, last_used_at FROM passkeys WHERE id = ?";
    match query_one(store, sql, &[DataValue::Text(passkey_id.to_string())]).await? {
        Some(row) => Ok(Some(parse_passkey_row(&row)?)),
        None => Ok(None),
    }
}

pub async fn find_passkey_by_credential_id(
    store: &dyn AsyncQueryStore,
    credential_id: &[u8],
) -> Result<Option<Passkey>, StorageOpError> {
    let sql = "SELECT id, user_id, name, credential_id, credential_public_key, counter, created_at, last_used_at FROM passkeys WHERE credential_id = ?";
    match query_one(store, sql, &[DataValue::Blob(credential_id.to_vec())]).await? {
        Some(row) => Ok(Some(parse_passkey_row(&row)?)),
        None => Ok(None),
    }
}

pub async fn update_passkey_counter(
    store: &dyn AsyncQueryStore,
    passkey_id: &str,
    counter: u32,
) -> Result<(), StorageOpError> {
    let sql = "UPDATE passkeys SET counter = ?, last_used_at = ? WHERE id = ?";
    let now = chrono::Utc::now().timestamp_millis();
    execute(
        store,
        sql,
        &[
            DataValue::Integer(i64::from(counter)),
            DataValue::Integer(now),
            DataValue::Text(passkey_id.to_string()),
        ],
    )
    .await
}

pub async fn update_passkey_name(
    store: &dyn AsyncQueryStore,
    passkey_id: &str,
    name: &str,
) -> Result<(), StorageOpError> {
    let sql = "UPDATE passkeys SET name = ? WHERE id = ?";
    execute(
        store,
        sql,
        &[DataValue::Text(name.to_string()), DataValue::Text(passkey_id.to_string())],
    )
    .await
}

pub async fn delete_passkey(store: &dyn AsyncQueryStore, passkey_id: &str) -> Result<(), StorageOpError> {
    let sql = "DELETE FROM passkeys WHERE id = ?";
    execute(store, sql, &[DataValue::Text(passkey_id.to_string())]).await
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

pub async fn store_tos_acceptance(
    store: &dyn AsyncQueryStore,
    acceptance: &TosAcceptance,
) -> Result<(), StorageOpError> {
    let sql = "INSERT INTO tos_acceptances (user_id, tos_version, accepted_at, ip_address) VALUES (?, ?, ?, ?)";
    execute(
        store,
        sql,
        &[
            DataValue::Text(acceptance.user_id.clone()),
            DataValue::Text(acceptance.tos_version.clone()),
            DataValue::Integer(acceptance.accepted_at),
            DataValue::Text(acceptance.ip_address.clone().unwrap_or_default()),
        ],
    )
    .await
}

pub async fn find_tos_acceptance(
    store: &dyn AsyncQueryStore,
    user_id: &str,
    tos_version: &str,
) -> Result<Option<TosAcceptance>, StorageOpError> {
    let sql = "SELECT user_id, tos_version, accepted_at, ip_address FROM tos_acceptances WHERE user_id = ? AND tos_version = ?";
    let params = [
        DataValue::Text(user_id.to_string()),
        DataValue::Text(tos_version.to_string()),
    ];
    match query_one(store, sql, &params).await? {
        Some(row) => Ok(Some(parse_tos_acceptance_row(&row)?)),
        None => Ok(None),
    }
}

pub async fn find_latest_tos_acceptance(
    store: &dyn AsyncQueryStore,
    user_id: &str,
) -> Result<Option<TosAcceptance>, StorageOpError> {
    let sql = "SELECT user_id, tos_version, accepted_at, ip_address FROM tos_acceptances WHERE user_id = ? ORDER BY accepted_at DESC LIMIT 1";
    match query_one(store, sql, &[DataValue::Text(user_id.to_string())]).await? {
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

// ─── HandlerStorage ─────────────────────────────────────────────────────────

/// The IdP server's storage: an async SQL store plus a key-value cache.
///
/// Implements [`AsyncAuthStore`] (passkeys, ToS, user lookup) over the SQL
/// store.
pub struct HandlerStorage<KV: AsyncKeyValueStore> {
    pub query_store: Arc<dyn AsyncQueryStore>,
    /// Shared cache for short-lived two-step handshake state (PoW challenges,
    /// WebAuthn ceremony state). Generic over the KV backend so the same
    /// handler works with an in-memory cache (single process) or a persistent
    /// store (Turso/D1) for multi-instance, crash-safe operation.
    pub cache: KV,
}

impl<KV: AsyncKeyValueStore> HandlerStorage<KV> {
    pub fn new(query_store: Arc<dyn AsyncQueryStore>, cache: KV) -> Self {
        Self { query_store, cache }
    }
}

// ─── Conversion helpers ─────────────────

fn passkey_to_stored(pk: &Passkey) -> StoredPasskey {
    StoredPasskey {
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

fn stored_to_passkey(sk: &StoredPasskey) -> Passkey {
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

fn tos_to_stored(ta: &TosAcceptance) -> StoredTosAcceptance {
    StoredTosAcceptance {
        user_id: ta.user_id.clone(),
        tos_version: ta.tos_version.clone(),
        accepted_at: ta.accepted_at,
        ip_address: ta.ip_address.clone(),
    }
}

fn stored_to_tos(st: &StoredTosAcceptance) -> TosAcceptance {
    TosAcceptance {
        user_id: st.user_id.clone(),
        tos_version: st.tos_version.clone(),
        accepted_at: st.accepted_at,
        ip_address: st.ip_address.clone(),
    }
}

// ─── AsyncAuthStore impl ─────────────────────────────────────────────────────

fn map_storage_err(e: StorageOpError) -> String {
    e.to_string()
}

fn op_to_storage_err(e: StorageOpError) -> StorageError {
    match e {
        StorageOpError::Parse(s) => StorageError::SqlConversion(s),
        other => StorageError::Backend(other.to_string()),
    }
}

#[async_trait::async_trait]
impl<KV: AsyncKeyValueStore> AsyncPasskeyStore for HandlerStorage<KV> {
    async fn store_passkey_async(&self, passkey: &StoredPasskey) -> Result<(), String> {
        let our_pk = stored_to_passkey(passkey);
        store_passkey(self.query_store.as_ref(), &our_pk).await.map_err(map_storage_err)
    }

    async fn find_passkeys_by_user_async<'a>(
        &'a self,
        user_id: &'a str,
    ) -> StorageResult<AsyncStorageItemStream<'a, StoredPasskey>> {
        find_passkeys_by_user(self.query_store.as_ref(), user_id)
            .await
            .map_err(op_to_storage_err)
    }

    async fn find_passkey_by_id_async<'a>(&'a self, passkey_id: &'a str) -> StorageResult<Option<StoredPasskey>> {
        find_passkey_by_id(self.query_store.as_ref(), passkey_id)
            .await
            .map(|opt| opt.as_ref().map(passkey_to_stored))
            .map_err(op_to_storage_err)
    }

    async fn find_passkey_by_credential_id_async<'a>(
        &'a self,
        credential_id: &'a [u8],
    ) -> StorageResult<Option<StoredPasskey>> {
        find_passkey_by_credential_id(self.query_store.as_ref(), credential_id)
            .await
            .map(|opt| opt.as_ref().map(passkey_to_stored))
            .map_err(op_to_storage_err)
    }

    async fn update_passkey_counter_async(&self, passkey_id: &str, counter: u32) -> Result<(), String> {
        update_passkey_counter(self.query_store.as_ref(), passkey_id, counter)
            .await
            .map_err(map_storage_err)
    }

    async fn update_passkey_name_async(&self, passkey_id: &str, name: &str) -> Result<(), String> {
        update_passkey_name(self.query_store.as_ref(), passkey_id, name)
            .await
            .map_err(map_storage_err)
    }

    async fn delete_passkey_async(&self, passkey_id: &str) -> Result<(), String> {
        delete_passkey(self.query_store.as_ref(), passkey_id).await.map_err(map_storage_err)
    }
}

#[async_trait::async_trait]
impl<KV: AsyncKeyValueStore> AsyncTosStore for HandlerStorage<KV> {
    async fn store_tos_acceptance_async(&self, acceptance: &StoredTosAcceptance) -> Result<(), String> {
        let our_ta = stored_to_tos(acceptance);
        store_tos_acceptance(self.query_store.as_ref(), &our_ta).await.map_err(map_storage_err)
    }

    async fn find_tos_acceptance_async<'a>(
        &'a self,
        user_id: &'a str,
        tos_version: &'a str,
    ) -> StorageResult<Option<StoredTosAcceptance>> {
        find_tos_acceptance(self.query_store.as_ref(), user_id, tos_version)
            .await
            .map(|opt| opt.as_ref().map(tos_to_stored))
            .map_err(op_to_storage_err)
    }

    async fn find_latest_tos_acceptance_async<'a>(
        &'a self,
        user_id: &'a str,
    ) -> StorageResult<Option<StoredTosAcceptance>> {
        find_latest_tos_acceptance(self.query_store.as_ref(), user_id)
            .await
            .map(|opt| opt.as_ref().map(tos_to_stored))
            .map_err(op_to_storage_err)
    }
}

#[async_trait::async_trait]
impl<KV: AsyncKeyValueStore> AsyncAuthStore for HandlerStorage<KV> {
    async fn find_user_by_email_async(&self, email: &str) -> StorageResult<Option<(String, bool)>> {
        find_user_by_email(self.query_store.as_ref(), email)
            .await
            .map(|opt| {
                opt.map(|u| {
                    let has_pw = u.has_password();
                    (u.id, has_pw)
                })
            })
            .map_err(op_to_storage_err)
    }
}
