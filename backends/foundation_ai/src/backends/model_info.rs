//! Model metadata lookups shared by the HTTP providers (OpenAI-compatible,
//! OpenAI Responses, Anthropic).
//!
//! WHY: every HTTP provider can ask its vendor for a model's metadata
//! (`GET models/{name}`), and every one of them must do it without blocking
//! on wasm, where a synchronous request cannot wait for a `fetch` without
//! freezing the isolate.
//!
//! WHAT:
//! - [`ModelInfoCache`] — the provider's per-model metadata, shared with the
//!   models it hands out so a model sees a lookup that finishes after it was
//!   created.
//! - [`fetch`] — the async lookup over [`HttpClient::send_async`] (yields on
//!   wasm), parsing the vendor's model object and error body.
//! - [`spawn_lookup`] (wasm) — run a lookup as its own valtron task and store
//!   the result in the cache; `get_model` returns placeholder metadata
//!   immediately.
//! - [`rfc3339_to_unix`] — Anthropic reports `created_at` as an RFC 3339 time.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use foundation_netio::shared::client::{
    body_reader::collect_strings_from_send_safe, http_client::HttpClient,
    request::PreparedRequest,
};
use serde::de::DeserializeOwned;

use crate::backends::openai_provider::format_http_error;
use crate::backends::openai_provider::OpenAIModelInfo;
use crate::errors::{GenerationError, GenerationResult};

/// Per-model metadata keyed by model name, shared between a provider and the
/// models it created.
pub type ModelInfoCache = Arc<Mutex<HashMap<String, OpenAIModelInfo>>>;

/// An empty cache.
#[must_use]
pub fn new_cache() -> ModelInfoCache {
    Arc::new(Mutex::new(HashMap::new()))
}

/// Placeholder metadata: just the model id (the lookup is best-effort;
/// nothing downstream needs more than the id).
#[must_use]
pub fn placeholder(model_name: &str, owned_by: &str) -> OpenAIModelInfo {
    OpenAIModelInfo {
        id: model_name.to_string(),
        object: String::from("model"),
        owned_by: owned_by.to_string(),
        created: 0,
    }
}

/// The cached metadata for `model_name`, or a placeholder.
#[must_use]
pub fn cached_or_placeholder(cache: &ModelInfoCache, model_name: &str, owned_by: &str) -> OpenAIModelInfo {
    cache
        .lock()
        .expect("model cache poisoned")
        .get(model_name)
        .cloned()
        .unwrap_or_else(|| placeholder(model_name, owned_by))
}

/// Whether `model_name` has an entry (looked up, or a placeholder awaiting
/// its lookup).
#[must_use]
pub fn contains(cache: &ModelInfoCache, model_name: &str) -> bool {
    cache
        .lock()
        .expect("model cache poisoned")
        .contains_key(model_name)
}

/// Store `info` for `model_name`.
pub fn store(cache: &ModelInfoCache, model_name: &str, info: OpenAIModelInfo) {
    cache
        .lock()
        .expect("model cache poisoned")
        .insert(model_name.to_string(), info);
}

/// Store `info` unless `model_name` already has an entry — for the placeholder
/// `get_model` records, which must never overwrite a lookup that already
/// finished.
pub fn store_if_absent(cache: &ModelInfoCache, model_name: &str, info: OpenAIModelInfo) {
    cache
        .lock()
        .expect("model cache poisoned")
        .entry(model_name.to_string())
        .or_insert(info);
}

/// Send `req` (a `GET models/{name}`) without blocking and parse the vendor's
/// model object `T` into [`OpenAIModelInfo`].
///
/// The body is a small JSON object needed in full to answer, so it is read
/// whole; an error body is reduced to the vendor's message by `error_detail`.
///
/// # Errors
///
/// When the request fails, the vendor answers non-2xx, or the body is not a
/// model object.
pub async fn fetch<T: DeserializeOwned>(
    client: Arc<dyn HttpClient>,
    req: PreparedRequest,
    error_detail: fn(&str) -> Option<String>,
    convert: fn(T) -> OpenAIModelInfo,
) -> GenerationResult<OpenAIModelInfo> {
    let response = client
        .send_async(req)
        .await
        .map_err(|e| GenerationError::Backend(format!("Request failed: {e}")))?;
    let (status, _headers, body) = response.into_parts();
    parse(status.into(), body, error_detail, convert)
}

/// [`fetch`] over the blocking [`HttpClient::send`] — for native
/// `get_model`, which is synchronous. Never call it on wasm.
///
/// # Errors
///
/// As [`fetch`].
#[cfg(not(target_family = "wasm"))]
pub fn fetch_blocking<T: DeserializeOwned>(
    client: &Arc<dyn HttpClient>,
    req: PreparedRequest,
    error_detail: fn(&str) -> Option<String>,
    convert: fn(T) -> OpenAIModelInfo,
) -> GenerationResult<OpenAIModelInfo> {
    let response = client
        .send(req)
        .map_err(|e| GenerationError::Backend(format!("Request failed: {e}")))?;
    let (status, _headers, body) = response.into_parts();
    parse(status.into(), body, error_detail, convert)
}

fn parse<T: DeserializeOwned>(
    status_code: usize,
    body: foundation_netio::shared::http::SendSafeBody,
    error_detail: fn(&str) -> Option<String>,
    convert: fn(T) -> OpenAIModelInfo,
) -> GenerationResult<OpenAIModelInfo> {
    let body_text = collect_strings_from_send_safe(body)
        .map_err(|e| GenerationError::Generic(format!("Parse error: {e}")))?;
    if !(200..=299).contains(&status_code) {
        let detail = error_detail(&body_text).unwrap_or(body_text);
        return Err(GenerationError::Backend(format_http_error(status_code, &detail)));
    }
    let resp: T = serde_json::from_str(&body_text)
        .map_err(|e| GenerationError::Generic(format!("Parse error: {e}")))?;
    Ok(convert(resp))
}

/// Run `lookup` as its own valtron task; on success its metadata replaces
/// `model_name`'s placeholder in `cache`. A failure keeps the placeholder (the
/// metadata is optional) and is logged.
#[cfg(target_family = "wasm")]
pub fn spawn_lookup<F>(cache: ModelInfoCache, model_name: String, lookup: F)
where
    F: std::future::Future<Output = GenerationResult<OpenAIModelInfo>> + 'static,
{
    let task = async move {
        match lookup.await {
            Ok(info) => store(&cache, &model_name, info),
            Err(e) => {
                tracing::debug!(model = %model_name, error = %e, "model metadata lookup failed; keeping placeholder");
            }
        }
    };
    if let Err(e) = foundation_core::valtron::send(foundation_core::valtron::from_future(task)) {
        tracing::warn!(error = %e, "could not schedule model metadata lookup");
    }
}

/// Seconds since the Unix epoch for an RFC 3339 UTC-or-offset time such as
/// `2025-02-19T00:00:00Z` or `2025-02-19T09:30:00.5+02:00` (fractional
/// seconds are dropped). `None` for anything else or a time before 1970.
#[must_use]
pub fn rfc3339_to_unix(s: &str) -> Option<u64> {
    let b = s.as_bytes();
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || !matches!(b[10], b'T' | b't' | b' ') {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<i64> { s.get(r)?.parse().ok() };
    let (year, month, day) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (hour, minute, second) = (num(11..13)?, num(14..16)?, num(17..19)?);
    if b[13] != b':' || b[16] != b':' || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let mut rest = &s[19..];
    if let Some(frac) = rest.strip_prefix('.') {
        rest = frac.trim_start_matches(|c: char| c.is_ascii_digit());
    }
    let offset = match rest {
        "Z" | "z" => 0,
        _ => {
            let sign = match rest.as_bytes().first()? {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let (h, m) = rest[1..].split_once(':')?;
            sign * (h.parse::<i64>().ok()? * 3600 + m.parse::<i64>().ok()? * 60)
        }
    };
    // Days from civil (Howard Hinnant's algorithm).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let secs = days * 86_400 + hour * 3600 + minute * 60 + second - offset;
    u64::try_from(secs).ok()
}
