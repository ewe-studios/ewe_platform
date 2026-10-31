# Feature 18 — foundation_auth — learnings

## Root causes

- **E2.** The tag compare used `==`. The `expected.len() == actual.len()` guard
  already rejected a truncated tag, so only the constant-time compare was missing.
  `hmac`'s `verify_slice` does the constant-time compare and the length check.
- **E4.** The header compare used `format!("{:?}", header)`. `SimpleHeader`'s
  `Debug` form is not its canonical name (`Custom("Authorization")`), so a
  case-insensitive compare against `"authorization"` failed. `SimpleHeader`
  already implements `Display` with the canonical lowercase name.
- **E5.** `as u32` on an `i64` wraps a negative or oversized value silently.
  `u32::try_from` reports it as a `Parse` error.
- **E7.** `constant_time_eq` in `password_hash.rs` is not only used by the
  native verifiers. The wasm `pbkdf2_verify` (`wasm-pbkdf2`) also calls it, so
  the correct gate is `any(not(wasm), wasm-pbkdf2)`, not `not(wasm)` alone.

## Dead ends

- **E7 `uuid`.** At first `uuid` looked native-only, but it is also used by
  `wasm_bindgen/session.rs` under `wasm-bindgen-session`. It is feature-gated,
  not native-only.
- **E7 `time`.** The `time` crate is unused; only `std::time` is used. It is
  removed, not made conditional.

## Caveats

- **E6.** Vendored OpenSSL and BoringSSL (`boring-sys`, pulled by `webauthn-rs`)
  export overlapping symbols, so the linker prints `duplicate symbol` warnings.
  The link succeeds. This is inherent to `webauthn-rs` 0.5 using both crypto
  stacks, not a defect this entry can fix.
