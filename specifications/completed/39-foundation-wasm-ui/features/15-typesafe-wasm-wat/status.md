# Feature 15 — Status: COMPLETE (2026-06-11)

All six success criteria met.

| Criterion | Evidence |
|---|---|
| 1. Byte-identical parse→serialize on real `.wasm` | `foundation_codegen/tests/wasm_roundtrip_tests.rs::real_modules_round_trip_byte_identically` — all 23 workspace fixtures (2 e2e + 21 legacy integration modules). |
| 2. Derives in `foundation_macros` | `src/wasmbin_codec.rs` (Wasmbin / WasmbinCountable / Visit, from wasmbin-derive v0.2.4); no companion crate; paths resolve via proc-macro-crate. |
| 3. WAT ⇄ binary round-trips | `foundation_codegen::wasm::wat` (feature `wat`): `from_wat`/`to_wat`; tests prove text→model→text stability and a binary fixpoint after one text round-trip on the real e2e module. |
| 4. CLI inspect/edit/convert/validate | `foundation_codegentools::cli::wasm` (feature `cli`): inspect, validate (deep Visit forces every Lazy payload to decode), convert (.wasm ⇄ .wat by extension), edit add-custom-section / rename-export. e2e-tested in `tests/wasm_cli_tests.rs`. |
| 5. Apache-2.0 attribution | Workspace `NOTICE`; vendored license at `foundation_codegen/src/wasm/vendor/LICENSE.wasmbin`; module README with §4(b) modification statements + author write-up link; per-file port notes; upstream marker v0.9.2. |
| 6. Type-safe edit demonstrated | Round-trip tests: export enumeration (the F13 `__fwt_` discovery path) + append-only custom-section injection (original bytes a strict prefix — `Lazy<T>` minimal diff); same edits exposed via the CLI. |

## Boundary decision (WAT grammar)

The WAT GRAMMAR rides the `wat` crate and printing rides `wasmprinter` — both small,
isolated, **opt-in** behind `foundation_codegen`'s `wat` feature, exactly the F14
boundary rules the spec authorizes for "unavoidable third-party WAT grammar".
Versions must stay paired (`wat` 1.NNN ↔ `wasmprinter` 0.NNN): mismatched pairs emit
quoted-identifier syntax the older parser rejects (hit with 1.207/0.224; pinned 0.207).

## Recorded follow-ups (not blocking)

- **Owned WAT printer** over the typed model (decision 031 "owned where feasible") —
  replaces the `wasmprinter` half of the boundary when prioritized.
- **Reference auto-cleanup** (walrus-style tombstones + pre-serialize relocation) —
  explicitly deferred by the spec (`features.md` "Future enhancement").
- F16 (walrus transform port) remains deferred per decision 031.
