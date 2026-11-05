---
feature: "Move inline #[cfg(test)] modules out of lane B crates (rule 7)"
status: "done"
lane: "B"
model: "haiku"
branch: "spec70/f59-f60-f79-lane-b-mechanical"
crates: ["lane B crates"]
catalogue_rows: [29]
---

# Feature 59: inline tests out (lane B)

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Same method as feature 50 (read `../50-lane-a-inline-tests/feature.md`), for lane B crates.
Known files: `foundation_packager/src/{package,files}.rs`, `foundation_html/src/{markup,parsers}.rs`,
`foundation_logging/src/lib.rs`, `foundation_wasmtime/src/lib.rs`,
`foundation_codegen/src/module_path.rs`, `foundation_connectrpc_codegen/src/generator.rs`,
`foundation_testing/src/{netcap/mod.rs,http/sse_server.rs}`,
`foundation_macros/src/{wasm_modes,wasm_logs,connectrpc_service}.rs` and
`foundation_macros/src/html_macro/{scoped_css,parser}.rs`,
`foundation_platform/src/{ipc/mod.rs,ipc/streaming.rs,ewe.rs,multi_app.rs,backend/ipc_dispatch.rs,worker.rs,ota.rs,overlay.rs}`,
`foundation_repl/src/shared/history.rs`, `foundation_deployment_cloudflare/src/workers/durable_object.rs`,
`crates/{template-macro,html-macro}/src/lib.rs`, `crates/watchers/src/config.rs`,
`infrastructure/llama-cpp/src/{llama_backend,log}.rs`. Also check `foundation_jsonschema`
(about 40 files): if it is a vendored copy of an upstream crate, do **not** move its tests;
record that it is vendored in the PR and in `docs/bug-backlog.md` as an accepted exception
the owner must confirm.

Proc-macro crates (`foundation_macros`, `crates/*-macro`) can only test public macros from
`tests/`; test internal helpers through macro expansion, or move the helper into a normal
library crate if it needs direct tests. Say which per file.

## Verify

```bash
rg -l '#\[cfg\(test\)\]' backends/<each lane B crate>/src crates/*/src infrastructure/*/src   # only jsonschema, if accepted
```
Test counts per crate before and after in the PR body. CI green.

## Done when

- [x] No `#[cfg(test)]` remains in the lane B `src` trees that this feature
      names. Test counts kept. CI green.

## Result

Moved 129 inline tests out of 11 lane B crates. Each crate keeps the same
test count and assertions:

| Crate | Tests moved | Public item added |
|---|---|---|
| foundation_shell | 1 | none |
| foundation_repl | 4 | none |
| foundation_logging | 12 | none (macros are `#[macro_export]`) |
| foundation_codegen | 9 | none |
| ewe_watchers | 5 | none; crate made a standalone workspace root |
| foundation_html | 47 | `Fragment::get_name`; `Stack` fields public |
| foundation_testing | 35 | `SharedBuffer::into_parts`, `compute_accept_key` |
| foundation_platform | 36 | `queue_arc`, `ota::chrono_now`, `mode_badge_for`, `build_mode_page`, `parse_ipc_route` |
| foundation_packager | 10 | `files` items and `BoxedError` re-exported |
| foundation_wasmtime | 3 | `WasmtimeBuilder::name` |
| foundation_connectrpc_codegen | 8 | `to_snake_case`, `extract_idempotency` |

Verified:
`rg -l '#\[cfg\(test\)\]' backends/*/src crates/*/src infrastructure/*/src`
now lists only these:
- `foundation_jsonschema` (accepted vendored-copy exception, backlog §29).
- `foundation_boringtun` (upstream fork; not in this feature's named list;
  backlog §16 keeps its move).
- `foundation_macros` and `crates/{html,template}-macro` (proc-macro crates;
  the helpers cannot be reached from `tests/` without a companion library
  crate; backlog §30).
- Lane A crates (feature 50 owns them) and
  `foundation_deployment_cloudflare` (another agent owns it).

`infrastructure_llama_cpp` moved 7 tests but could not compile here:
`tools/llama.cpp` is empty, so the bindings build script stops on a missing
`llama.h`. Its version stays 0.0.4 because `foundation_ai` pins it and is
owned by PR #176 (backlog §28).
