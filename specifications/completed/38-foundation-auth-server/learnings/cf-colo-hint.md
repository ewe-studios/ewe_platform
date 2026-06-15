# cf-colo-hint — Learnings Review

**Source**: `/home/darkvoid/Boxxed/@formulas/src.rust/src.gedweb/cf-colo-hint/`
**Reviewed**: 2026-06-07

## Project Overview

A `no_std`, zero-dependency Rust library that maps Cloudflare edge locations (colos, identified by 3-letter IATA codes like "LAX", "AMS") to recommended Durable Objects location hints (e.g., "wnam", "weur"). Solves: when a request arrives at a Cloudflare edge datacenter, which DO region should you instantiate your object in for optimal latency?

**Version**: 0.1.1021 | **License**: MIT | **Dependencies**: none

## Architecture

```
cf-colo-hint/
├── Cargo.toml                          # no_std, zero deps
├── README.md
├── codegen.py                          # Python code generator (~425 lines)
├── refresh.sh                          # Fetch data + regenerate
├── components.json                     # ~227KB, Cloudflare status page data
├── where.durableobjects.live.json      # ~130KB, DO latency/region data
└── src/
    ├── lib.rs                          # Public API, re-exports, doc tests (~99 lines)
    └── generated.rs                    # Auto-generated enums (~2534 lines, DO NOT EDIT)
```

## Key Implementation Details

### Two Enums

**`LocationHint`** (9 variants): `Afr`, `Apac`, `EEur`, `ENam`, `Me`, `Oc`, `Sam`, `WEur`, `WNam`
- `as_str()` → lowercase API code (e.g., `"wnam"`) — used directly in DO API calls
- `name()` → human-readable name
- `parse(s: &str)` → roundtrip parsing
- `ALL` → const slice for iteration

**`Colo`** (~280 variants): Every Cloudflare colo as an enum variant named after its IATA code
- `code()` → 3-letter string
- `name()` → full city/country description
- `location_hint()` → `Option<LocationHint>` — **the core function**
- `from_code(code: &str)` → parsing

### Code Generation Pipeline

The 2534-line `generated.rs` is **not written by hand**. Produced by `codegen.py` from live JSON data:

1. **Fetch** `components.json` from Cloudflare status API (colo names, cities, countries)
2. **Fetch** `where.durableobjects.live.json` from DO latency API (measured latency per colo)
3. **Merge** both sources
4. **Hysteresis**: Apply deadband to prevent oscillation between region flips
   - New region only accepted if ≥15ms better OR ≥20% faster than previous best
   - Prevents noisy measurement flips between refreshes
5. **Generate** Rust code to `generated.rs`

## Borrowable Patterns

### 1. Codegen from Live Data
Fetch external API data, merge multiple sources, apply smoothing/hysteresis, generate Rust enums. Works for any rapidly-changing dataset (IP ranges, currency codes, airport codes). We could use this for generating CF region configs, routing tables, or any data that changes upstream.

### 2. Hysteresis for Noisy Data
When regenerating configs from volatile measurements, apply a deadband to prevent oscillation. Broadly applicable to caching, DNS TTLs, load balancer weights, health check thresholds.

### 3. `const fn` Lookup Tables
Use `match`-based lookups in `const fn` form for zero-runtime-overhead data access. Prefer over `HashMap` for static datasets under ~1000 entries. The entire library resolves to jump tables at compile time.

### 4. `no_std` + Zero-Dep Libraries
For pure-data crates, avoiding dependencies maximizes compatibility (wasm, embedded, etc.). Our `foundation_db` stream types and auth types could benefit from this philosophy — fewer deps = easier wasm compilation.

### 5. `#[non_exhaustive]` for Forward Compatibility
When modeling externally-defined sets that grow over time (ISO codes, API enums, colo codes), use `#[non_exhaustive]` to avoid semver breaks. Both enums here are non-exhaustive because Cloudflare adds new colos regularly.

### 6. Separation of Handwritten vs Generated Code
`lib.rs` = public API, docs, tests. `generated.rs` = pure data with "DO NOT EDIT" header. Humans never touch generated code. Any change is a deliberate `./refresh.sh` run.

### 7. Roundtrip Testing
Always test that `serialize → parse → serialize` is identity:
```rust
for colo in Colo::ALL {
    assert_eq!(Colo::from_code(colo.code()), Some(colo));
}
```

### 8. Best-Effort Semantics with `Option`
`location_hint()` returns `Option<LocationHint>`, not `LocationHint`. ~50 of 280 colos return `None`. Callers must handle gracefully with fallback. Good pattern for incomplete data.

## Applicability to ewe_platform

**Directly useful:**
- The colo-to-region mapping could be vendored or depended on by our Cloudflare Workers deployment for DO routing
- The codegen pattern could be adapted for generating our Cedar policy configs from external sources

**Conceptually useful:**
- Our `foundation_db` AsyncQueryStream/AsyncListStream types follow a similar "pure data + zero deps" philosophy
- The hysteresis pattern could be applied to our JWKS cache TTL logic (prevent flapping on key rotation detection)
- The `const fn` lookup table pattern for static data (OAuth error codes, JWT algorithm mappings)

## Potential Issues Noted

1. Some colos have `None` for `location_hint()` — callers need fallback
2. No versioning of JSON data — stale checkouts get stale mappings
3. Python build-time dependency for regeneration (not pure Rust)
4. No serde support for direct deserialization into enum types
5. Regional grouping can be counterintuitive (AMS → Afr instead of WEur due to latency, not geography)
