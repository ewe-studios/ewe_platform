//! Storage-backed operations for the IdP server.

use std::sync::Arc;

use foundation_db::core::storage_provider::{DataValue, QueryStore, SqlRow};

use super::models::{AuthorizationCode, DeviceCode, OAuthClient, RefreshToken, User};

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

#[derive(Clone)]
pub struct HandlerStorage {
    pub query_store: Arc<dyn QueryStore>,
}

impl HandlerStorage {
    pub fn new(query_store: Arc<dyn QueryStore>) -> Self {
        Self { query_store }
    }
}
