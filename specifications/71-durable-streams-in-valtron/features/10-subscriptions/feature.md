---
feature: "Durable Streams subscriptions"
status: "planned"
priority: "high"
depends_on: ["01-durable-stream-types", "06-http-rpc-surface"]
crates: ["foundation_core", "foundation_http"]
---

# Feature 10: Durable Streams subscriptions

_Written in ASD-STE100 Simplified Technical English (STE)._

**Abbreviations.** Hypertext Transfer Protocol (HTTP); JavaScript Object Notation (JSON);
JavaScript Object Signing and Encryption (JOSE); JSON Web Key (JWK); JSON Web Key Set (JWKS);
lease time to live (TTL).

The owner corrected the scope on 2026-11-27: the reference protocol defines subscriptions, and
this spec includes them. The server conformance suite has eight subscription tests. Six of them
are skipped today.

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
- The wake snapshot is acknowledged once. A repeat acknowledgement is idempotent.
- Unsafe webhook URLs are refused.

## 3. Design

- Core: `foundation_core::durable::shared::subscription` holds the subscription configuration,
  the generation, the lease and the wake snapshot. It uses the injected clock.
- HTTP: `foundation_http` serves the `__ds` control endpoints and the webhook deliveries.
- Storage: the subscription state uses the same `DurableStreamVfs` boundary.
- Signing keys come from an injected key source, never a global.

## 4. Tasks

- [ ] **T1 — core state.** Configuration, hash, generation, lease and snapshot. Tests:
      `config_hash_is_stable`, `generation_fences_stale_claim`.
- [ ] **T2 — control endpoints.** Create, confirm, add and remove streams, list. Tests:
      `idempotent_confirm`, `add_and_remove_streams`.
- [ ] **T3 — webhook delivery.** Signed body, retry, failed status, synchronous done ack. Tests:
      `webhook_signature_verifies`, `failed_delivery_sets_failed`.
- [ ] **T4 — pull-wake.** Claim, ack, release, lease expiry. Tests:
      `claim_then_ack_clears`, `lease_expiry_reopens`.
- [ ] **T5 — key discovery.** JWKS endpoint and stable `kid`. Test: `jwks_lists_public_keys`.
- [ ] **T6 — unsafe URL refusal.** Test: `rejects_unsafe_webhook_urls`.
- [ ] **T7 — conformance.** The six skipped server tests pass. `ci/durable_conformance.sh` shows
      0 skips.

## 5. Done when

- The eight subscription tests in the reference server suite pass.
- The conformance gate shows 338 of 338 with 0 skips.
