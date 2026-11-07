---
feature: "Replace the gRPC status transmute with a checked conversion"
status: "completed"
priority: "high"
depends_on: []
crates: ["foundation_connectrpc"]
issues: [265]
model: "sonnet"
branch: "spec73/f01-connectrpc-grpc-status-conversion"
---

# Feature 01: gRPC status conversion

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Google Remote Procedure Call (gRPC); undefined behaviour (UB);
AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE standard
ASD-STE100.

## 1. Goal

Remove the `unsafe` transmute from `parse_grpc_error_trailer` and close issue
[#265](https://github.com/ewe-studios/ewe_platform/issues/265).

## 2. Problem

1. `backends/foundation_connectrpc/src/shared/protocol/grpc.rs:640` is
   `parse_grpc_error_trailer`. Line 662 builds the status code with
   `let code: Code = unsafe { std::mem::transmute(status) };`.
2. A manual clamp just before it keeps the value in `1..=16`, or sets it to `2`. The check
   is correct today, so the current behaviour is not UB. The defect is latent: the safety
   rests on a hand-written range check and on the enum discriminants. A change to `Code`
   (a gap, a new variant, a new representation) turns it into UB with no compiler error.
3. The issue body calls the invalid value UB. Measure the current clamp first. Record the
   measured state in the PR: the issue is about soundness and readability, not a live crash.

## 3. Requirements

1. No `unsafe` block and no `std::mem::transmute` remains in `parse_grpc_error_trailer`.
2. A value in `1..=16` maps to the same `Code` as now. Any other non-zero value maps to
   `Code::Unknown` (the current clamp uses `2`, which is `Unknown`; keep the same result).
3. Status `0` still means "no error" and returns `None` before the conversion.
4. The conversion is one named function. A reviewer reads one match table, not an enum
   layout.
5. No public API change, so no crate version bump.

## 4. Design

1. Read `backends/foundation_connectrpc/src/shared/error/code.rs`. `Code` is at line 17 and
   its impls start at line 53. Check for an existing checked conversion (`TryFrom<u32>` or a
   constructor) before you write one.
2. If a checked conversion exists, use it. If not, add a private function in
   `shared/protocol/grpc.rs`:
   ```rust
   /// Map a gRPC status code from the wire to [`Code`]. Unknown values map to
   /// `Code::Unknown`, per the gRPC status code specification.
   fn code_from_grpc_status(status: u32) -> Code {
       match status {
           1 => Code::Cancelled,
           // one arm per defined status code, 1 through 16
           _ => Code::Unknown,
       }
   }
   ```
   Use the variant names that `code.rs` defines; do not invent names.
3. `parse_grpc_error_trailer` calls the function. Delete the clamp and the SAFETY comment.
   Keep the comment short: the mapping is explicit.
4. Change no other behaviour and no signature.

## 5. Tasks

- [x] Read `grpc.rs:640-664`, `error/code.rs` and the `Code` definition.
- [x] Decide: existing checked conversion, or a private match table. `Code::from_u32`
      already exists in `shared/error/code.rs`; the fix uses it.
- [x] Replace the transmute.
- [x] Add the table test.
- [x] Run the verify commands. Record the results: fmt 0, clippy `-D warnings` 0, tests 257
      passed and 0 failed (1 doctest ignored), source gate 0.
- [x] Close #265 in the PR body.

## 9. Result (2026-11-07)

Branch `spec73/f01-connectrpc-grpc-status-conversion`, commit `028ca4a15`. Merged into
`spec73/high-severity-fixes` as `48421e32c`.

1. The transform is now `Code::from_u32(status).unwrap_or(Code::Unknown)`. The clamp and the
   `SAFETY` comment are deleted. Behaviour is the same for every input.
2. The crate has no `tests/mod.rs`. It uses top-level integration test files that Cargo
   discovers. The new test is `tests/grpc_status_conversion_tests.rs`.
3. Deviation from requirement 5: `parse_grpc_error_trailer` changed from `pub(crate)` to
   `#[doc(hidden)] pub`, plus `#[must_use]`. An integration test can only reach the public
   API, and the crate already uses this hook pattern (`h2_pump_poll_count`). The documented
   API does not change, so no version bump. The function is marked as a test hook.

## 6. Tests

Tests live in `backends/foundation_connectrpc/tests/` and are registered in its
`tests/mod.rs` (read it first).

1. `grpc_status_conversion_maps_every_wire_value` in `tests/grpc_tests.rs`, or a new
   `tests/grpc_status_conversion_tests.rs` if that file is large:
   - call `parse_grpc_error_trailer` with a `grpc-status` trailer for every value `0..=255`;
   - assert `None` for 0, the expected `Code` for 1 through 16, and `Code::Unknown` for every
     other value;
   - assert it never panics.
2. **Honest limit:** the old code clamps and therefore already maps every value, so this
   behavioural test passes on the old code too. Do not claim it fails before the fix. The
   proof of the change is a source gate, recorded in the PR:
   ```bash
   ! rg -n "mem::transmute" backends/foundation_connectrpc/src/shared/protocol/grpc.rs
   ```
3. If the conversion function is private, the test calls it through
   `parse_grpc_error_trailer`. Make the function public only if the test cannot reach the
   behaviour; then apply the version-bump rule.

## 7. Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo fmt -p foundation_connectrpc
cargo clippy --profile uat -p foundation_connectrpc --no-deps --all-targets -- -D warnings
cargo test --profile uat -p foundation_connectrpc
! rg -n "mem::transmute" backends/foundation_connectrpc/src/shared/protocol/grpc.rs
```

Strict clippy on dependencies is blocked by known issues; `--no-deps` is required.

## 8. Done when

- The transmute and its SAFETY comment are gone.
- The mapping test covers 0 through 255.
- The source gate returns no match.
- #265 is closed by the PR.
