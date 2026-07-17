# Decision 03: Feature Gate Strategy — Mutually Exclusive Backend Features

## Problem

How to gate the Cloudflare vs. native backend code:

1. **Mutually exclusive features**: `backend-cloudflare` vs. `backend-native` — user picks one at compile time
2. **Auto-detected target gates**: `cfg(target_arch = "wasm32")` — automatic based on compile target
3. **Both**: feature gates for explicit control + target gates as fallback

## Analysis

- **Mutually exclusive features**: Explicit, no surprises. Compile-time guarantee that only one backend is compiled. User must consciously choose. Matches how the workspace handles other dual-target crates (e.g., foundation_wireguard's `native-mesh` feature).
- **Auto-detected**: Convenient but error-prone. A developer building for wasm32 on their laptop might accidentally get the Cloudflare backend when they wanted native (if cross-compiling). Harder to test both backends on the same machine.
- **Both**: Overcomplicated. Target gates already provide the wasm32/native split; adding feature gates on top creates confusion.

## Decision: Mutually exclusive features

Two features: `backend-cloudflare` and `backend-native`. At least one must be enabled. Enabling both is a compile error (checked in `lib.rs`).

```rust
#[cfg(all(feature = "backend-cloudflare", feature = "backend-native"))]
compile_error!("Only one of 'backend-cloudflare' or 'backend-native' may be enabled at a time");

#[cfg(not(any(feature = "backend-cloudflare", feature = "backend-native")))]
compile_error!("At least one of 'backend-cloudflare' or 'backend-native' must be enabled");
```

## Consequences

- Two separate builds for testing (one with each feature)
- CI runs both feature combinations
- Documentation must clearly explain which feature to pick
- A future "full" build (both backends in one binary) is possible but deferred
