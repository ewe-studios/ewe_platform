//! Portable request router (spec-57, F008 Stage 2/3).
//!
//! WHY: both backends — native (`foundation_http`) and Workers (`worker-rs`) —
//! need the same method+path → handler mapping. Keeping it here (cross-platform,
//! primitive inputs/outputs) means one route table serves both: the native
//! `ServeAdapter` single-polls [`route`] (its handlers finish in one poll over
//! local Turso), and the Workers `fetch` handler `.await`s it (D1 is truly async).
//!
//! WHAT: [`route`] takes the method, path, raw body, and optional bearer token,
//! authenticates where required, dispatches to the `core/api` handler, and returns
//! `(status, json)`. The transport renders that to its native response type.

use crate::core::api::{accounts, ciphers, events, folders, icons, identity, orgs, sends, sync, two_factor};
use crate::core::auth::verify_access_token;
use crate::core::context::KeychainContext;
use crate::core::error::{AppError, AppResult};
use crate::core::models::cipher::{CipherCreateRequest, CipherUpdateRequest};
use crate::core::models::folder::{FolderCreateRequest, FolderUpdateRequest};
use crate::core::models::org::{
    CollectionCreateRequest, ConfirmMemberRequest, InviteMemberRequest, OrganizationCreateRequest,
};
use crate::core::models::send::{SendCreateRequest, SendUpdateRequest};
use crate::core::models::user::{TokenRequest, UpdateProfileRequest};

/// Serialize a handler result into `(200, json)`.
fn ok<T: serde::Serialize>(value: T) -> AppResult<(u16, serde_json::Value)> {
    let json = serde_json::to_value(value)
        .map_err(|e| AppError::Internal(format!("serialize response: {e}")))?;
    Ok((200, json))
}

fn empty() -> AppResult<(u16, serde_json::Value)> {
    Ok((200, serde_json::Value::Null))
}

fn from_json<T: serde::de::DeserializeOwned>(body: &str) -> AppResult<T> {
    serde_json::from_str(body).map_err(|e| AppError::BadRequest(format!("invalid request body: {e}")))
}

fn json_value(body: &str) -> serde_json::Value {
    serde_json::from_str(body).unwrap_or(serde_json::Value::Null)
}

/// Parse `connect/token`'s `application/x-www-form-urlencoded` body.
pub fn parse_token_request(body: &str) -> TokenRequest {
    use std::collections::HashMap;
    let map: HashMap<String, String> = body
        .split('&')
        .filter_map(|pair| {
            let mut it = pair.splitn(2, '=');
            let key = it.next()?;
            Some((form_decode(key), form_decode(it.next().unwrap_or(""))))
        })
        .collect();
    TokenRequest {
        grant_type: map.get("grant_type").cloned().unwrap_or_default(),
        username: map.get("username").cloned(),
        master_password_hash: map.get("password").cloned(),
        refresh_token: map.get("refresh_token").cloned(),
        device_identifier: map.get("deviceIdentifier").cloned(),
        device_name: map.get("deviceName").cloned(),
        device_type: map.get("deviceType").and_then(|v| v.parse().ok()),
    }
}

fn form_decode(s: &str) -> String {
    let replaced = s.replace('+', " ");
    let raw = replaced.as_bytes();
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%' && i + 2 < raw.len() {
            if let Ok(byte) = u8::from_str_radix(&replaced[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(raw[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Route a request to a handler. `method` is upper-case (`"GET"`, `"POST"`, …);
/// `path` excludes the query string; `bearer` is the token from
/// `Authorization: Bearer …` if present.
pub async fn route(
    ctx: &KeychainContext,
    method: &str,
    path: &str,
    body: &str,
    bearer: Option<&str>,
) -> AppResult<(u16, serde_json::Value)> {
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
    let m = method;
    let s = segments.as_slice();

    // ── Server config (bw initial contact) ──────────────────────────────────
    if s == ["api"] || s == ["api", "config"] {
        return Ok((200, serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
        })));
    }

    // ── Unauthenticated ─────────────────────────────────────────────────────
    match (m, s) {
        ("POST", ["identity", "accounts", "prelogin"])
        | ("POST", ["identity", "accounts", "prelogin", "password"]) => {
            let email = json_value(body)
                .get("email")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            return ok(accounts::prelogin(ctx, &email).await?);
        }
        ("POST", ["identity", "accounts", "register"]) => {
            return ok(accounts::register(ctx, from_json(body)?).await?);
        }
        ("POST", ["identity", "connect", "token"]) => {
            return ok(identity::connect_token(ctx, parse_token_request(body)).await?);
        }
        ("POST", ["api", "sends", "access", id]) => {
            let pw = json_value(body).get("password").and_then(|v| v.as_str()).map(String::from);
            return ok(sends::access(ctx, id, pw.as_deref()).await?);
        }
        (_, ["icons", domain, "icon.png"]) => {
            let icon = icons::fetch_icon(domain).await?;
            return Ok((200, serde_json::json!({
                "contentType": icon.content_type,
                "length": icon.bytes.len(),
            })));
        }
        _ => {}
    }

    // ── Authenticated ───────────────────────────────────────────────────────
    let token = bearer.ok_or(AppError::Unauthorized)?;
    let uid = verify_access_token(ctx.verifier(), token)?.user_uuid;
    let uid = uid.as_str();

    match (m, s) {
        ("GET", ["api", "sync"]) => ok(sync::sync(ctx, uid).await?),
        ("GET", ["api", "accounts", "profile"]) => ok(accounts::get_profile(ctx, uid).await?),
        ("PUT", ["api", "accounts", "profile"]) => {
            let req: UpdateProfileRequest = from_json(body)?;
            ok(accounts::update_profile(ctx, uid, req).await?)
        }
        ("POST", ["api", "accounts", "verify-password"]) => {
            let hash = json_value(body)
                .get("masterPasswordHash")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let valid = accounts::verify_master_password(ctx, uid, &hash).await?;
            Ok((200, serde_json::json!({ "valid": valid })))
        }

        // Folders
        ("GET", ["api", "folders"]) => ok(folders::list(ctx, uid).await?),
        ("POST", ["api", "folders"]) => {
            let req: FolderCreateRequest = from_json(body)?;
            ok(folders::create(ctx, uid, req).await?)
        }
        ("GET", ["api", "folders", id]) => ok(folders::get(ctx, uid, id).await?),
        ("PUT", ["api", "folders", id]) => {
            let req: FolderUpdateRequest = from_json(body)?;
            ok(folders::update(ctx, uid, id, req).await?)
        }
        ("DELETE", ["api", "folders", id]) => {
            folders::delete(ctx, uid, id).await?;
            empty()
        }

        // Ciphers
        ("GET", ["api", "ciphers"]) => ok(ciphers::list(ctx, uid).await?),
        ("POST", ["api", "ciphers"]) => {
            let req: CipherCreateRequest = from_json(body)?;
            ok(ciphers::create(ctx, uid, req).await?)
        }
        ("GET", ["api", "ciphers", id]) => ok(ciphers::get(ctx, uid, id).await?),
        ("PUT", ["api", "ciphers", id]) => {
            let req: CipherUpdateRequest = from_json(body)?;
            ok(ciphers::update(ctx, uid, id, req).await?)
        }
        ("DELETE", ["api", "ciphers", id]) => {
            ciphers::delete(ctx, uid, id).await?;
            empty()
        }
        ("PUT", ["api", "ciphers", id, "delete"]) => ok(ciphers::soft_delete(ctx, uid, id).await?),
        ("PUT", ["api", "ciphers", id, "restore"]) => ok(ciphers::restore(ctx, uid, id).await?),

        // Sends
        ("GET", ["api", "sends"]) => ok(sends::list(ctx, uid).await?),
        ("POST", ["api", "sends"]) => {
            let req: SendCreateRequest = from_json(body)?;
            ok(sends::create(ctx, uid, req).await?)
        }
        ("GET", ["api", "sends", id]) => ok(sends::get(ctx, uid, id).await?),
        ("PUT", ["api", "sends", id]) => {
            let req: SendUpdateRequest = from_json(body)?;
            ok(sends::update(ctx, uid, id, req).await?)
        }
        ("DELETE", ["api", "sends", id]) => {
            sends::delete(ctx, uid, id).await?;
            empty()
        }

        // Two-factor
        ("GET", ["api", "two-factor", "get-authenticator"]) => {
            ok(two_factor::get_authenticator(ctx, uid).await?)
        }
        ("POST", ["api", "two-factor", "authenticator"]) => {
            let token = json_value(body).get("token").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            ok(two_factor::enable_authenticator(ctx, uid, &token).await?)
        }
        ("DELETE", ["api", "two-factor", "authenticator"]) => {
            let token = json_value(body).get("token").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            two_factor::disable_authenticator(ctx, uid, &token).await?;
            empty()
        }

        // Events
        ("POST", ["api", "events", "collect"]) => {
            let evs: Vec<events::EventRequest> = from_json(body)?;
            events::collect(ctx, uid, evs).await?;
            empty()
        }
        ("GET", ["api", "events"]) => ok(events::list(ctx, uid).await?),

        // Organizations
        ("POST", ["api", "organizations"]) => {
            let email = accounts::account(ctx, uid).await?.email;
            let req: OrganizationCreateRequest = from_json(body)?;
            ok(orgs::create(ctx, uid, &email, req).await?)
        }
        ("GET", ["api", "organizations", id]) => ok(orgs::get(ctx, uid, id).await?),
        ("DELETE", ["api", "organizations", id]) => {
            orgs::delete(ctx, uid, id).await?;
            empty()
        }
        ("GET", ["api", "organizations", id, "collections"]) => {
            ok(orgs::list_collections(ctx, uid, id).await?)
        }
        ("POST", ["api", "organizations", id, "collections"]) => {
            let req: CollectionCreateRequest = from_json(body)?;
            ok(orgs::create_collection(ctx, uid, id, req).await?)
        }
        ("DELETE", ["api", "organizations", id, "collections", cid]) => {
            orgs::delete_collection(ctx, uid, id, cid).await?;
            empty()
        }
        ("GET", ["api", "organizations", id, "users"]) => ok(orgs::list_members(ctx, uid, id).await?),
        ("POST", ["api", "organizations", id, "users", "invite"]) => {
            let req: InviteMemberRequest = from_json(body)?;
            ok(orgs::invite_member(ctx, uid, id, req).await?)
        }
        ("POST", ["api", "organizations", id, "users", mid, "confirm"]) => {
            let req: ConfirmMemberRequest = from_json(body)?;
            orgs::confirm_member(ctx, uid, id, mid, req).await?;
            empty()
        }

        _ => Err(AppError::NotFound(format!("no route for {method} {path}"))),
    }
}
