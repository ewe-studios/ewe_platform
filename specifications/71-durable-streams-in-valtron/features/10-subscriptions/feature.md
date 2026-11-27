---
feature: "Durable Streams subscriptions"
status: "planned"
priority: "high"
depends_on: ["01-durable-stream-types", "06-http-rpc-surface"]
crates: ["foundation_event_stream", "foundation_core", "foundation_auth"]
---

# Feature 10: Durable Streams subscriptions

_Written in ASD-STE100 Simplified Technical English (STE)._

**Abbreviations.** Hypertext Transfer Protocol (HTTP); JavaScript Object Notation (JSON);
JavaScript Object Signing and Encryption (JOSE); JSON Web Key (JWK); JSON Web Key Set (JWKS);
lease time to live (TTL).

The owner corrected the scope on 2026-11-27: the reference protocol defines subscriptions, and
this spec includes them. The server conformance suite has six subscription tests. All six were
skipped before this feature.

## 1. Goal

Add the reserved `__ds` subscription surface. A subscription is a durable cursor that wakes a
worker when one or more streams have new data. Delivery is by webhook or by pull-wake.

## 2. Contract (protocol section 7)

- Control endpoints live under the reserved `__ds` prefix.
- A subscription names explicit streams or a pattern. The configuration hash is idempotent.
- Webhook delivery is signed with an asymmetric key. Public keys are discoverable as a JWK Set
  (JWKS).
- Pull-wake uses a claim, acknowledge and release flow with a lease and a generation fence.
- A stale generation is refused.
- The wake snapshot is acknowledged once. A repeat acknowledgement for a closed wake is refused
  with `409 FENCED`.
- Deletion writes a tombstone. A restart must not restore a deleted subscription.
- Unsafe webhook URLs are refused.

## 3. Design

The owner proposed one home for the work: a new `foundation_event_stream` crate. It owns
subscriptions and brings the pieces together. The server core stays transport-neutral.

- `foundation_event_stream` holds the subscription configuration, the configuration hash, the
  generation, the lease, the wake snapshot, the webhook delivery and the pull-wake flow. It uses
  the injected clock and `DurableStreamVfs` for its state.
- The `__ds` control endpoints are `Serve` handlers in `foundation_event_stream`, mounted in the
  host application beside the Durable Streams handler.
- Webhook signing and the JSON Web Key Set (JWKS) reuse `foundation_auth`:
  `foundation_auth::shared::jwks::{Jwk, Jwks, JwksManager}` and the signing key types. Do not
  add a second key model or a second key endpoint.
- The object store and the streams come from `foundation_core::durable`.
- `foundation_event_stream` re-exports `foundation_core::durable` as
  `foundation_event_stream::durable`. A host imports the stream types and the subscription API from
  one crate. The re-export carries the whole durable module, not a copy of the types.
- A host supplies the key source and the HTTP transport. The crate reads no global.

## 4. Tasks

- [x] **T0 — crate.** Create `foundation_event_stream` with `shared/`, `native/` and `cf/`
      layout. Re-export `foundation_core::durable` as `foundation_event_stream::durable`. Re-export
      the `foundation_auth` JWKS types it uses. Tests: `crate_builds_wasm`,
      `durable_types_match_the_core`.
- [x] **T1 — core state.** Configuration, hash, generation, lease and snapshot. Tests:
      `config_hash_is_stable`, `generation_fences_stale_claim`.
- [x] **T2 — control endpoints.** Create, confirm, add and remove streams, list. Tests:
      `idempotent_confirm`, `add_and_remove_streams`.
- [x] **T3 — webhook delivery.** Signed body, retry, failed status, synchronous done ack. Tests:
      `webhook_signature_verifies`, `failed_delivery_sets_failed`.
- [x] **T4 — pull-wake.** Claim, ack, release, lease expiry. Tests:
      `claim_then_ack_clears`, `lease_expiry_reopens`.
- [x] **T5 — key discovery.** JWKS endpoint and stable `kid`. Test: `jwks_lists_public_keys`.
- [x] **T6 — unsafe URL refusal.** Test: `rejects_unsafe_webhook_urls`.
- [x] **T7 — conformance.** The six skipped server tests pass. `ci/durable_conformance.sh` shows
      0 skips.

## 5. Done when

- The six subscription tests in the reference server suite pass.
- The conformance gate shows 338 of 338 with 0 skips.
