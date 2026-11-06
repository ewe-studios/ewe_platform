//! Storage-backed operations for the `IdP` server.
//!
//! Every function here talks to an [`QueryStore`] (Turso/libsql natively,
//! D1 on Workers) and is awaited by the async handlers. Multi-row reads hand
//! back a lazily-pulled stream, so an unbounded scan can't run out of memory.

use std::sync::Arc;

use crate::shared::clock::now_millis;
use foundation_core::valtron::Clock;
use foundation_db::core::storage_provider::{AsyncQueryStream, DataValue, QueryStore, SqlRow};
use foundation_db::{
    AsyncStorageItemStream, AuthStore, KeyValueStore, PasskeyStore, StorageError, StorageResult,
    StoredPasskey, StoredTosAcceptance, TosStore,
};
use futures_lite::StreamExt;

use super::models::{
    AuthorizationCode, DeviceCode, OAuthClient, Passkey, RefreshToken, TosAcceptance, User,
};

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

// Taken by value so it can be passed straight to `map_err`.
#[allow(clippy::needless_pass_by_value)]
fn query_err(e: StorageError) -> StorageOpError {
    StorageOpError::Query(e.to_string())
}

// Taken by value so it can be passed straight to `map_err`.
#[allow(clippy::needless_pass_by_value)]
fn parse_err(e: StorageError) -> StorageOpError {
    StorageOpError::Parse(e.to_string())
}

/// Convert a stored `INTEGER` column (i64) to a `u32`, returning a
/// [`StorageOpError::Parse`] on a negative value or a value over `u32::MAX`.
///
/// # Errors
///
/// Returns [`StorageOpError::Parse`] if the value does not fit in a `u32`.
pub fn int_to_u32(value: i64, column: &str) -> Result<u32, StorageOpError> {
    u32::try_from(value)
        .map_err(|_| StorageOpError::Parse(format!("{column} out of u32 range: {value}")))
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
    store: &dyn QueryStore,
    sql: &str,
    params: &[DataValue],
) -> Result<Option<SqlRow>, StorageOpError> {
    let stream = store.query(sql, params).await.map_err(query_err)?;
    first_row(stream).await
}

/// Run a statement, discarding the affected-row count.
async fn execute(
    store: &dyn QueryStore,
    sql: &str,
    params: &[DataValue],
) -> Result<(), StorageOpError> {
    store
        .execute(sql, params)
        .await
        .map(|_| ())
        .map_err(query_err)
}

// ─── OAuth clients, codes, tokens ────────────────────────────────────────────

/// Find client by id.
///
/// # Errors
///
/// Returns [`StorageOpError`] if the operation fails.
pub async fn find_client_by_id(
    store: &dyn QueryStore,
    client_id: &str,
) -> Result<Option<OAuthClient>, StorageOpError> {
    let sql = "SELECT id, name, client_secret_hash, redirect_uris, grant_types, scopes, is_public, created_at FROM oauth_clients WHERE id = ?";
    match query_one(store, sql, &[DataValue::Text(client_id.to_string())]).await? {
        Some(row) => Ok(Some(parse_client_row(&row)?)),
        None => Ok(None),
    }
}

/// Store a registered OAuth client.
///
/// # Errors
///
/// Returns [`StorageOpError`] if the operation fails.
pub async fn store_oauth_client(
    store: &dyn QueryStore,
    client: &OAuthClient,
) -> Result<(), StorageOpError> {
    let sql = "INSERT INTO oauth_clients (id, name, client_secret_hash, redirect_uris, grant_types, scopes, is_public, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)";
    let json = |v: &Vec<String>| {
        serde_json::to_string(v).map_err(|e| StorageOpError::Parse(e.to_string()))
    };
    execute(
        store,
        sql,
        &[
            DataValue::Text(client.id.clone()),
            DataValue::Text(client.name.clone()),
            DataValue::Text(client.client_secret_hash.clone()),
            DataValue::Text(json(&client.redirect_uris)?),
            DataValue::Text(json(&client.grant_types)?),
            DataValue::Text(json(&client.scopes)?),
            DataValue::Integer(i64::from(client.is_public)),
            DataValue::Integer(client.created_at),
        ],
    )
    .await
}

/// Store auth code.
///
/// # Errors
///
/// Returns [`StorageOpError`] if the operation fails.
pub async fn store_auth_code(
    store: &dyn QueryStore,
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

/// Find auth code.
///
/// # Errors
///
/// Returns [`StorageOpError`] if the operation fails.
pub async fn find_auth_code(
    store: &dyn QueryStore,
    code: &str,
) -> Result<Option<AuthorizationCode>, StorageOpError> {
    let sql = "SELECT code, user_id, client_id, redirect_uri, code_challenge, scope, nonce, expires_at, created_at FROM authorization_codes WHERE code = ?";
    match query_one(store, sql, &[DataValue::Text(code.to_string())]).await? {
        Some(row) => Ok(Some(parse_auth_code_row(&row)?)),
        None => Ok(None),
    }
}

/// Delete auth code.
///
/// # Errors
///
/// Returns [`StorageOpError`] if the operation fails.
pub async fn delete_auth_code(store: &dyn QueryStore, code: &str) -> Result<(), StorageOpError> {
    let sql = "DELETE FROM authorization_codes WHERE code = ?";
    execute(store, sql, &[DataValue::Text(code.to_string())]).await
}

/// Store refresh token.
///
/// # Errors
///
/// Returns [`StorageOpError`] if the operation fails.
pub async fn store_refresh_token(
    store: &dyn QueryStore,
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

/// Find refresh token by hash.
///
/// # Errors
///
/// Returns [`StorageOpError`] if the operation fails.
pub async fn find_refresh_token_by_hash(
    store: &dyn QueryStore,
    token_hash: &str,
) -> Result<Option<RefreshToken>, StorageOpError> {
    let sql = "SELECT id, user_id, client_id, token_hash, expires_at, rotated_at, created_at FROM refresh_tokens WHERE token_hash = ?";
    match query_one(store, sql, &[DataValue::Text(token_hash.to_string())]).await? {
        Some(row) => Ok(Some(parse_refresh_token_row(&row)?)),
        None => Ok(None),
    }
}

/// Mark refresh token rotated.
///
/// # Errors
///
/// Returns [`StorageOpError`] if the operation fails.
pub async fn mark_refresh_token_rotated(
    store: &dyn QueryStore,
    token_hash: &str,
    clock: &dyn Clock,
) -> Result<(), StorageOpError> {
    let now = now_millis(clock);
    let sql = "UPDATE refresh_tokens SET rotated_at = ? WHERE token_hash = ?";
    execute(
        store,
        sql,
        &[
            DataValue::Integer(now),
            DataValue::Text(token_hash.to_string()),
        ],
    )
    .await
}

/// Store device code.
///
/// # Errors
///
/// Returns [`StorageOpError`] if the operation fails.
pub async fn store_device_code(
    store: &dyn QueryStore,
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

/// Find device code.
///
/// # Errors
///
/// Returns [`StorageOpError`] if the operation fails.
pub async fn find_device_code(
    store: &dyn QueryStore,
    device_code: &str,
) -> Result<Option<DeviceCode>, StorageOpError> {
    let sql = "SELECT device_code, user_code, client_id, scope, expires_at, interval_secs, user_id, created_at FROM device_codes WHERE device_code = ?";
    match query_one(store, sql, &[DataValue::Text(device_code.to_string())]).await? {
        Some(row) => Ok(Some(parse_device_code_row(&row)?)),
        None => Ok(None),
    }
}

/// Approve device code.
///
/// # Errors
///
/// Returns [`StorageOpError`] if the operation fails.
pub async fn approve_device_code(
    store: &dyn QueryStore,
    device_code: &str,
    user_id: &str,
) -> Result<(), StorageOpError> {
    let sql = "UPDATE device_codes SET user_id = ? WHERE device_code = ?";
    execute(
        store,
        sql,
        &[
            DataValue::Text(user_id.to_string()),
            DataValue::Text(device_code.to_string()),
        ],
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
        code_challenge: if code_challenge.is_empty() {
            None
        } else {
            Some(code_challenge)
        },
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
        rotated_at: if rotated_at == 0 {
            None
        } else {
            Some(rotated_at)
        },
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
        interval: int_to_u32(
            row.get_by_name::<i64>("interval_secs").map_err(parse_err)?,
            "interval_secs",
        )?,
        user_id: if user_id.is_empty() {
            None
        } else {
            Some(user_id)
        },
        created_at: row.get_by_name("created_at").map_err(parse_err)?,
    })
}

// ─── Users ───────────────────────────────────────────────────────────────────

/// Find user by email.
///
/// # Errors
///
/// Returns [`StorageOpError`] if the operation fails.
pub async fn find_user_by_email(
    store: &dyn QueryStore,
    email: &str,
) -> Result<Option<User>, StorageOpError> {
    let sql = "SELECT id, email, username, password_hash, email_verified, email_verified_at, created_at, updated_at, metadata, failed_login_attempts, locked_until, deleted_at FROM users WHERE email = ?";
    match query_one(store, sql, &[DataValue::Text(email.to_string())]).await? {
        Some(row) => Ok(Some(parse_user_row(&row)?)),
        None => Ok(None),
    }
}

/// Update a user's failed login attempts and lockout state.
///
/// # Errors
///
/// Returns [`StorageOpError`] if the operation fails.
pub async fn update_user_lockout(
    store: &dyn QueryStore,
    user_id: &str,
    failed_attempts: u32,
    locked_until: Option<i64>,
    clock: &dyn Clock,
) -> Result<(), StorageOpError> {
    let sql =
        "UPDATE users SET failed_login_attempts = ?, locked_until = ?, updated_at = ? WHERE id = ?";
    let now = now_millis(clock);
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
///
/// # Errors
///
/// Returns [`StorageOpError`] if the operation fails.
pub async fn find_user_by_id(
    store: &dyn QueryStore,
    user_id: &str,
) -> Result<Option<User>, StorageOpError> {
    let sql = "SELECT id, email, username, password_hash, email_verified, email_verified_at, created_at, updated_at, metadata, failed_login_attempts, locked_until, deleted_at FROM users WHERE id = ?";
    match query_one(store, sql, &[DataValue::Text(user_id.to_string())]).await? {
        Some(row) => Ok(Some(parse_user_row(&row)?)),
        None => Ok(None),
    }
}

/// Update user profile fields (username only for now — email changes require re-verification).
///
/// # Errors
///
/// Returns [`StorageOpError`] if the operation fails.
pub async fn update_user_profile(
    store: &dyn QueryStore,
    user_id: &str,
    username: Option<&str>,
    clock: &dyn Clock,
) -> Result<(), StorageOpError> {
    let now = now_millis(clock);
    let sql = "UPDATE users SET username = ?, updated_at = ? WHERE id = ?";
    execute(
        store,
        sql,
        &[
            username_value(username),
            DataValue::Integer(now),
            DataValue::Text(user_id.to_string()),
        ],
    )
    .await
}

/// Update a user's password hash.
///
/// # Errors
///
/// Returns [`StorageOpError`] if the operation fails.
pub async fn update_user_password(
    store: &dyn QueryStore,
    user_id: &str,
    new_hash: &str,
    clock: &dyn Clock,
) -> Result<(), StorageOpError> {
    let now = now_millis(clock);
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
///
/// # Errors
///
/// Returns [`StorageOpError`] if the operation fails.
pub async fn soft_delete_user(
    store: &dyn QueryStore,
    user_id: &str,
    clock: &dyn Clock,
) -> Result<(), StorageOpError> {
    let now = now_millis(clock);
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
pub async fn create_user(
    store: &dyn QueryStore,
    user: &User,
    clock: &dyn Clock,
) -> Result<(), StorageOpError> {
    let sql = "INSERT INTO users (id, email, username, password_hash, email_verified, email_verified_at, created_at, updated_at, metadata, failed_login_attempts, locked_until, deleted_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 0, NULL, NULL)";
    let now = now_millis(clock);
    let email_verified_at = if user.email_verified { Some(now) } else { None };
    let metadata = serde_json::to_string(&user.metadata)
        .map_err(|e| StorageOpError::Parse(format!("user metadata: {e}")))?;
    execute(
        store,
        sql,
        &[
            DataValue::Text(user.id.clone()),
            DataValue::Text(user.email.clone()),
            username_value(user.username.as_deref()),
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

/// A missing or empty username is stored as NULL: `users.username` is UNIQUE,
/// and SQL allows many NULLs but only one empty string.
fn username_value(username: Option<&str>) -> DataValue {
    match username {
        Some(u) if !u.is_empty() => DataValue::Text(u.to_string()),
        _ => DataValue::Null,
    }
}

fn parse_user_row(row: &SqlRow) -> Result<User, StorageOpError> {
    let password_hash: String = row.get_by_name("password_hash").map_err(parse_err)?;
    let metadata: String = row.get_by_name("metadata").map_err(parse_err)?;
    // NULL (none) or, in rows made before NULL was used, an empty string.
    let username_val: Option<String> = row.get_by_name("username").map_err(parse_err)?;
    let username_val = username_val.filter(|u| !u.is_empty());
    let email_verified_at_val: i64 = row.get_by_name("email_verified_at").map_err(parse_err)?;
    let locked_until_val: i64 = row.get_by_name("locked_until").map_err(parse_err)?;
    let deleted_at_val: i64 = row.get_by_name("deleted_at").map_err(parse_err)?;
    let failed_login_attempts: i64 = row
        .get_by_name("failed_login_attempts")
        .map_err(parse_err)?;
    Ok(User {
        id: row.get_by_name("id").map_err(parse_err)?,
        email: row.get_by_name("email").map_err(parse_err)?,
        username: username_val,
        password_hash: if password_hash.is_empty() {
            None
        } else {
            Some(password_hash)
        },
        email_verified: row.get_by_name("email_verified").map_err(parse_err)?,
        email_verified_at: if email_verified_at_val == 0 {
            None
        } else {
            Some(email_verified_at_val)
        },
        created_at: row.get_by_name("created_at").map_err(parse_err)?,
        updated_at: row.get_by_name("updated_at").map_err(parse_err)?,
        metadata: if metadata.is_empty() {
            None
        } else {
            Some(
                serde_json::from_str(&metadata)
                    .map_err(|e| StorageOpError::Parse(format!("user metadata: {e}")))?,
            )
        },
        failed_login_attempts: int_to_u32(failed_login_attempts, "failed_login_attempts")?,
        locked_until: if locked_until_val == 0 {
            None
        } else {
            Some(locked_until_val)
        },
        deleted_at: if deleted_at_val == 0 {
            None
        } else {
            Some(deleted_at_val)
        },
    })
}

fn parse_json_vec<T: serde::de::DeserializeOwned>(value: &str) -> Result<Vec<T>, StorageOpError> {
    serde_json::from_str(value).map_err(|e| StorageOpError::Parse(e.to_string()))
}

// ─── Passkeys ────────────────────────────────────────────────────────────────

/// Store passkey.
///
/// # Errors
///
/// Returns [`StorageOpError`] if the operation fails.
pub async fn store_passkey(
    store: &dyn QueryStore,
    passkey: &Passkey,
) -> Result<(), StorageOpError> {
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
///
/// # Errors
///
/// Returns [`StorageOpError`] if the operation fails.
pub async fn find_passkeys_by_user(
    store: &dyn QueryStore,
    user_id: &str,
) -> Result<AsyncStorageItemStream<'static, StoredPasskey>, StorageOpError> {
    let sql = "SELECT id, user_id, name, credential_id, credential_public_key, counter, created_at, last_used_at FROM passkeys WHERE user_id = ?";
    let stream = store
        .query(sql, &[DataValue::Text(user_id.to_string())])
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

/// Find passkey by id.
///
/// # Errors
///
/// Returns [`StorageOpError`] if the operation fails.
pub async fn find_passkey_by_id(
    store: &dyn QueryStore,
    passkey_id: &str,
) -> Result<Option<Passkey>, StorageOpError> {
    let sql = "SELECT id, user_id, name, credential_id, credential_public_key, counter, created_at, last_used_at FROM passkeys WHERE id = ?";
    match query_one(store, sql, &[DataValue::Text(passkey_id.to_string())]).await? {
        Some(row) => Ok(Some(parse_passkey_row(&row)?)),
        None => Ok(None),
    }
}

/// Find passkey by credential id.
///
/// # Errors
///
/// Returns [`StorageOpError`] if the operation fails.
pub async fn find_passkey_by_credential_id(
    store: &dyn QueryStore,
    credential_id: &[u8],
) -> Result<Option<Passkey>, StorageOpError> {
    let sql = "SELECT id, user_id, name, credential_id, credential_public_key, counter, created_at, last_used_at FROM passkeys WHERE credential_id = ?";
    match query_one(store, sql, &[DataValue::Blob(credential_id.to_vec())]).await? {
        Some(row) => Ok(Some(parse_passkey_row(&row)?)),
        None => Ok(None),
    }
}

/// Update passkey counter.
///
/// # Errors
///
/// Returns [`StorageOpError`] if the operation fails.
pub async fn update_passkey_counter(
    store: &dyn QueryStore,
    passkey_id: &str,
    counter: u32,
    clock: &dyn Clock,
) -> Result<(), StorageOpError> {
    let sql = "UPDATE passkeys SET counter = ?, last_used_at = ? WHERE id = ?";
    let now = now_millis(clock);
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

/// Update passkey name.
///
/// # Errors
///
/// Returns [`StorageOpError`] if the operation fails.
pub async fn update_passkey_name(
    store: &dyn QueryStore,
    passkey_id: &str,
    name: &str,
) -> Result<(), StorageOpError> {
    let sql = "UPDATE passkeys SET name = ? WHERE id = ?";
    execute(
        store,
        sql,
        &[
            DataValue::Text(name.to_string()),
            DataValue::Text(passkey_id.to_string()),
        ],
    )
    .await
}

/// Delete passkey.
///
/// # Errors
///
/// Returns [`StorageOpError`] if the operation fails.
pub async fn delete_passkey(
    store: &dyn QueryStore,
    passkey_id: &str,
) -> Result<(), StorageOpError> {
    let sql = "DELETE FROM passkeys WHERE id = ?";
    execute(store, sql, &[DataValue::Text(passkey_id.to_string())]).await
}

fn parse_passkey_row(row: &SqlRow) -> Result<Passkey, StorageOpError> {
    let credential_id: Vec<u8> = row.get_by_name("credential_id").map_err(parse_err)?;
    let credential_public_key: Vec<u8> = row
        .get_by_name("credential_public_key")
        .map_err(parse_err)?;
    let last_used_at_val: i64 = row.get_by_name("last_used_at").map_err(parse_err)?;
    Ok(Passkey {
        id: row.get_by_name("id").map_err(parse_err)?,
        user_id: row.get_by_name("user_id").map_err(parse_err)?,
        name: row.get_by_name("name").map_err(parse_err)?,
        credential_id,
        credential_public_key,
        counter: int_to_u32(
            row.get_by_name::<i64>("counter").map_err(parse_err)?,
            "counter",
        )?,
        created_at: row.get_by_name("created_at").map_err(parse_err)?,
        last_used_at: if last_used_at_val == 0 {
            None
        } else {
            Some(last_used_at_val)
        },
    })
}

// ─── ToS Acceptances ────────────────────────────────────────────────────────

/// Store tos acceptance.
///
/// # Errors
///
/// Returns [`StorageOpError`] if the operation fails.
pub async fn store_tos_acceptance(
    store: &dyn QueryStore,
    acceptance: &TosAcceptance,
) -> Result<(), StorageOpError> {
    let sql = "INSERT OR IGNORE INTO tos_acceptances (user_id, tos_version, accepted_at, ip_address) VALUES (?, ?, ?, ?)";
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

/// Find tos acceptance.
///
/// # Errors
///
/// Returns [`StorageOpError`] if the operation fails.
pub async fn find_tos_acceptance(
    store: &dyn QueryStore,
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

/// Find latest tos acceptance.
///
/// # Errors
///
/// Returns [`StorageOpError`] if the operation fails.
pub async fn find_latest_tos_acceptance(
    store: &dyn QueryStore,
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
        ip_address: if ip_address.is_empty() {
            None
        } else {
            Some(ip_address)
        },
    })
}

// ─── HandlerStorage ─────────────────────────────────────────────────────────

/// The `IdP` server's storage: an async SQL store plus a key-value cache.
///
/// Implements [`AuthStore`] (passkeys, `ToS`, user lookup) over the SQL
/// store.
pub struct HandlerStorage<KV: KeyValueStore> {
    pub query_store: Arc<dyn QueryStore>,
    /// Shared cache for short-lived two-step handshake state (`PoW` challenges,
    /// WebAuthn ceremony state). Generic over the KV backend so the same
    /// handler works with an in-memory cache (single process) or a persistent
    /// store (Turso/D1) for multi-instance, crash-safe operation.
    pub cache: KV,
    /// The injected engine clock. The store's own trait methods (`PasskeyStore`,
    /// the cache sweeper) read their timestamps from it.
    pub clock: Arc<dyn Clock>,
}

/// Cache key prefixes whose values carry an `expires_at` field (Unix
/// milliseconds): `ToS` codes, MFA challenges and pending TOTP enrollments, and
/// `PoW` challenges. [`HandlerStorage::sweep_expired`] deletes the expired ones.
pub const EXPIRING_CACHE_PREFIXES: &[&str] = &[
    "tos:code:",
    "mfa:challenge:",
    "mfa:totp_pending:",
    "pow:challenge:",
];

impl<KV: KeyValueStore> HandlerStorage<KV> {
    pub fn new(query_store: Arc<dyn QueryStore>, cache: KV, clock: Arc<dyn Clock>) -> Self {
        Self {
            query_store,
            cache,
            clock,
        }
    }

    /// Delete the expired entries of the short-lived handshake state in the
    /// cache ([`EXPIRING_CACHE_PREFIXES`]). Returns how many it deleted.
    ///
    /// The cache backends have no expiry of their own, so each entry holds its
    /// `expires_at`. A read of an expired entry already returns nothing
    /// (`TosService::take_code`, `MfaService::take_challenge`); this removes the
    /// entries nobody presented. Run it from a timer (for example every few
    /// minutes), like the SQL cleanup in `foundation_db`
    /// (`run_full_cleanup`). It reads every key under the prefixes, so its cost
    /// grows with the number of live entries.
    ///
    /// # Errors
    ///
    /// Returns [`StorageOpError`] if a cache call fails.
    pub async fn sweep_expired(&self) -> Result<usize, StorageOpError> {
        let now = now_millis(self.clock.as_ref());
        let mut removed = 0;
        for prefix in EXPIRING_CACHE_PREFIXES {
            // The key stream ends before the next call on the same store.
            let keys: Vec<String> = {
                let mut stream = self
                    .cache
                    .list_keys(Some(prefix))
                    .await
                    .map_err(|e| StorageOpError::Query(format!("cache list: {e}")))?;
                let mut keys = Vec::new();
                while let Some(key) = futures_lite::StreamExt::next(&mut stream).await {
                    keys.push(key.map_err(|e| StorageOpError::Query(format!("cache list: {e}")))?);
                }
                keys
            };
            for key in keys {
                let value = self
                    .cache
                    .get::<serde_json::Value>(&key)
                    .await
                    .map_err(|e| StorageOpError::Query(format!("cache get: {e}")))?;
                let expired = value
                    .and_then(|v| v.get("expires_at").and_then(serde_json::Value::as_i64))
                    .is_some_and(|at| at <= now);
                if expired {
                    self.cache
                        .delete(&key)
                        .await
                        .map_err(|e| StorageOpError::Query(format!("cache delete: {e}")))?;
                    removed += 1;
                }
            }
        }
        Ok(removed)
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

// ─── AuthStore impl ─────────────────────────────────────────────────────

// Taken by value so it can be passed straight to `map_err`.
#[allow(clippy::needless_pass_by_value)]
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
impl<KV: KeyValueStore> PasskeyStore for HandlerStorage<KV> {
    async fn store_passkey(&self, passkey: &StoredPasskey) -> Result<(), String> {
        let our_pk = stored_to_passkey(passkey);
        store_passkey(self.query_store.as_ref(), &our_pk)
            .await
            .map_err(map_storage_err)
    }

    async fn find_passkeys_by_user<'a>(
        &'a self,
        user_id: &'a str,
    ) -> StorageResult<AsyncStorageItemStream<'a, StoredPasskey>> {
        find_passkeys_by_user(self.query_store.as_ref(), user_id)
            .await
            .map_err(op_to_storage_err)
    }

    async fn find_passkey_by_id<'a>(
        &'a self,
        passkey_id: &'a str,
    ) -> StorageResult<Option<StoredPasskey>> {
        find_passkey_by_id(self.query_store.as_ref(), passkey_id)
            .await
            .map(|opt| opt.as_ref().map(passkey_to_stored))
            .map_err(op_to_storage_err)
    }

    async fn find_passkey_by_credential_id<'a>(
        &'a self,
        credential_id: &'a [u8],
    ) -> StorageResult<Option<StoredPasskey>> {
        find_passkey_by_credential_id(self.query_store.as_ref(), credential_id)
            .await
            .map(|opt| opt.as_ref().map(passkey_to_stored))
            .map_err(op_to_storage_err)
    }

    async fn update_passkey_counter(&self, passkey_id: &str, counter: u32) -> Result<(), String> {
        update_passkey_counter(
            self.query_store.as_ref(),
            passkey_id,
            counter,
            self.clock.as_ref(),
        )
        .await
        .map_err(map_storage_err)
    }

    async fn update_passkey_name(&self, passkey_id: &str, name: &str) -> Result<(), String> {
        update_passkey_name(self.query_store.as_ref(), passkey_id, name)
            .await
            .map_err(map_storage_err)
    }

    async fn delete_passkey(&self, passkey_id: &str) -> Result<(), String> {
        delete_passkey(self.query_store.as_ref(), passkey_id)
            .await
            .map_err(map_storage_err)
    }
}

#[async_trait::async_trait]
impl<KV: KeyValueStore> TosStore for HandlerStorage<KV> {
    async fn store_tos_acceptance(&self, acceptance: &StoredTosAcceptance) -> Result<(), String> {
        let our_ta = stored_to_tos(acceptance);
        store_tos_acceptance(self.query_store.as_ref(), &our_ta)
            .await
            .map_err(map_storage_err)
    }

    async fn find_tos_acceptance<'a>(
        &'a self,
        user_id: &'a str,
        tos_version: &'a str,
    ) -> StorageResult<Option<StoredTosAcceptance>> {
        find_tos_acceptance(self.query_store.as_ref(), user_id, tos_version)
            .await
            .map(|opt| opt.as_ref().map(tos_to_stored))
            .map_err(op_to_storage_err)
    }

    async fn find_latest_tos_acceptance<'a>(
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
impl<KV: KeyValueStore> AuthStore for HandlerStorage<KV> {
    async fn find_user_by_email(&self, email: &str) -> StorageResult<Option<(String, bool)>> {
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
