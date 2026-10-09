//! Sync API — portable handler (spec-57, F008 Stage 1).
//!
//! WHY: `GET /api/sync` is the endpoint every client hits after login — it
//! returns the full vault snapshot in one payload. Ported from OrangeVault
//! `api/sync.rs`, decoupled from any transport.
//!
//! WHAT: `sync` assembles the user's profile + folders + ciphers (+ empty
//! collections/sends and default domains for the personal vault; org collections
//! and sends fill in as those verticals land).
//!
//! HOW: reuses the per-entity handlers ([`accounts::account`],
//! [`folders::list`], [`ciphers::list`]) so the sync payload stays consistent
//! with the individual endpoints.

use crate::core::api::{accounts, ciphers, folders, orgs, sends};
use crate::core::context::KeychainContext;
use crate::core::error::AppResult;
use crate::core::models::sync::{EquivalentDomains, SyncData};
use crate::core::store::orgs as org_store;

/// `GET /api/sync` — the full vault snapshot for the user.
pub async fn sync(ctx: &KeychainContext, user_uuid: &str) -> AppResult<SyncData> {
    let profile = accounts::account(ctx, user_uuid).await?;
    let folders = folders::list(ctx, user_uuid).await?;
    let ciphers = ciphers::list(ctx, user_uuid).await?;
    let sends = sends::list(ctx, user_uuid).await?;

    // Collections the user can see = collections of every org they belong to.
    let mut collections = Vec::new();
    for membership in org_store::find_memberships_by_user(ctx.db(), user_uuid).await? {
        for collection in orgs::list_collections(ctx, user_uuid, &membership.org_uuid).await? {
            collections.push(serde_json::to_value(collection).unwrap_or_default());
        }
    }

    Ok(SyncData {
        profile: Some(profile),
        folders,
        ciphers,
        collections,
        sends,
        domains: Some(EquivalentDomains {
            equivalent_domains: None,
            global_equivalent_domains: None,
        }),
    })
}
