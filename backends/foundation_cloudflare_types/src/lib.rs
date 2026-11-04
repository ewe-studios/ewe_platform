//! Shared Cloudflare D1/R2 wire types.
//!
//! WHY: `foundation_db` speaks the Cloudflare D1 and R2 REST APIs. It cannot
//! depend on `foundation_deployment_cloudflare` for those types: that crate
//! depends on `foundation_deployment`, which depends on `foundation_db`. This
//! leaf crate depends on neither, so both storage crates can use it.
//!
//! WHAT: Every type here is generated. [`generated`] holds the cross-group
//! shared resource types the Cloudflare generator hoists out of the per-group
//! crates, plus the D1 `/query` request and response types and the R2
//! object-listing types that `foundation_db` needs. The names `foundation_db`
//! uses are re-exported at the crate root.
//!
//! HOW: Nothing in this crate is written by hand. Change a type by changing the
//! generator or an overlay, then run `genapi generate cloudflare`:
//!
//! - The D1 `/query` request is `D1BatchQuery`, an untagged enum for the spec's
//!   `oneOf` (a single query, or `{ "batch": [...] }`). The generator emits it
//!   because `d1_batch_query` is named in `CLOUDFLARE_TYPED_UNIONS`.
//! - The R2 listing endpoint is absent from the vendored `OpenAPI` spec. The
//!   overlay `artefacts/cloud_providers/cloudflare/overlays/r2-list-objects.json`
//!   adds it, so the generator produces `R2ListObjectsResponse` and its parts.
//! - `d1-query.json` in the same directory types the two row counters as
//!   integers. The D1 parameters stay strings, as the spec has them: the REST
//!   API takes strings only, so `foundation_db` types each value in SQL.
//! - The D1 `/raw` response is `ApiEnvelope<Vec<D1RawResultResponse>>`.
//!
//! `foundation_codegentools::codegen::CLOUDFLARE_HOISTED_SCHEMAS` names the
//! schemas that are emitted here although only one group uses them.

/// Generated Cloudflare shared resource types (feature 22 E3, feature 52).
pub mod generated;

pub use generated::{
    ApiEnvelope, D1BatchQuery, D1QueryDatabaseResponse, D1QueryMeta, D1QueryResultResponse,
    D1RawResultResponse, D1SingleQuery,
    R2ListObjectsResponse, R2ListObjectsResultInfo, R2ObjectInfo,
};
