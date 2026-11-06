// Database columns store KDF parameters and counters as i64; the API wire
// fields are i32/u32 and the stored values are always in range.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

//! Events API — portable audit-log handlers (spec-57, F008 Stage 1).
//!
//! WHY: Bitwarden clients `POST /events/collect` a batch of user-activity events;
//! the owner can list them. A lightweight audit trail — ported (minimally, as in
//! OrangeVault) from `api/events.rs`.

use chrono::{DateTime, Utc};

use crate::core::clock::wall_now_utc;
use serde::{Deserialize, Serialize};

use crate::core::crypto;

use crate::core::context::KeychainContext;
use crate::core::error::AppResult;
use crate::core::store::events as store;
use crate::core::store::events::EventRow;

/// A single event the client reports.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventRequest {
    #[serde(rename = "type")]
    pub event_type: i32,
    pub cipher_id: Option<String>,
    pub date: Option<String>,
}

/// An event in a list response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub event_type: i32,
    pub cipher_id: Option<String>,
    pub date: DateTime<Utc>,
    pub object: &'static str,
}

fn to_response(row: &EventRow) -> EventResponse {
    EventResponse {
        id: row.uuid.clone(),
        event_type: row.atype as i32,
        cipher_id: row.cipher_uuid.clone(),
        date: row.event_date,
        object: "event",
    }
}

/// `POST /api/events/collect` — record a batch of events.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn collect(
    ctx: &KeychainContext,
    user_uuid: &str,
    events: Vec<EventRequest>,
) -> AppResult<()> {
    for event in events {
        let date = event
            .date
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map_or_else(|| wall_now_utc(ctx.clock()), |d| d.with_timezone(&Utc));
        store::insert(
            ctx.db(),
            &EventRow {
                uuid: crypto::new_uuid_v4()?.to_string(),
                user_uuid: user_uuid.to_string(),
                atype: i64::from(event.event_type),
                cipher_uuid: event.cipher_id,
                event_date: date,
            },
            ctx.clock(),
        )
        .await?;
    }
    Ok(())
}

/// `GET /api/events` — the user's recorded events (newest first).
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn list(ctx: &KeychainContext, user_uuid: &str) -> AppResult<Vec<EventResponse>> {
    Ok(store::find_by_user(ctx.db(), user_uuid)
        .await?
        .iter()
        .map(to_response)
        .collect())
}
