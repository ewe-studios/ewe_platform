# 031 — Owned WASM infrastructure; wasm-bindgen only at explicit integration boundaries

**Date:** 2026-06-10
**Status:** Resolved

### Decision

The platform **owns its WASM↔JS infrastructure end-to-end** — the runtime
(`foundation_wasm` ABI + `foundation-wasm.js` / `foundation-wasm-ui.js`), the build
(`cargo` → `wasm32-unknown-unknown`, native codegen), and the test harness
(`foundation_wasm_testbed` running our modules on our runtime). **No wasm-bindgen and
no wasm-pack in the default path.**

`wasm-bindgen` (and, where it helps, `walrus`) is permitted **only** at small, explicit,
opt-in integration boundaries where a third party mandates it — never as the backbone.

---

### Why

Spec-31's testbed was built to "replace wasm-pack/wasm-bindgen-test-runner" yet leaned on
them (CLI glue generation, `#[wasm_bindgen_test]` + `__wbgt_` discovery). That inverts
ownership: a framework that owns its ABI must also own how that ABI is built and tested.
Owning it gives: one interop model (ours), no version-coupling to the wasm-bindgen CLI,
smaller/auditable output, and a build that works with our toolchain choices (e.g. the
LLVM codegen backend on a Cranelift-default workspace).

---

### Policy

**Owned by default (everything in spec-39):**
- Runtime/interop: `foundation_wasm` ABI + the `foundation-wasm*.js` runtime. Instantiate
  with `{ abi: runtime.web_abi }`; the module exports the raw ABI (memory, `host_apply`,
  callbacks, timers). No generated glue.
- Build: `cargo build --target wasm32-unknown-unknown` with the LLVM backend (the repo's
  `dev` profile uses Cranelift, which can't target wasm32 — override via
  `CARGO_PROFILE_DEV_CODEGEN_BACKEND=llvm`, already done in `foundation_wasm_testbed::build`).
  One shared build primitive for `ewe-wasm build` (F10) and the testbed (F12).
- Tests: our `#[wasm_test]` macro + `__fwt_` export discovery + a result protocol over our
  ABI, run on our runtime under node/deno/browser (F13). No `#[wasm_bindgen_test]`.

**wasm-bindgen permitted ONLY here (explicit, isolated, opt-in — F14):**
- **Cloudflare Workers** via `worker-rs`, which is heavily wasm-bindgen-based.
- **`foundation_db`** / Arrow and other crates that already ship wasm-bindgen integrations.
- The testbed's `bindgen-*` modes — retained behind an explicit `--interop=wasm-bindgen`
  opt-in for testing such crates, not for ours.

**`walrus`** is fine for parsing/rewriting wasm binaries — including wasm-bindgen's output
at the boundary above, and our own `__fwt_` export discovery. It is a wasm-format tool, not
a wasm-bindgen dependency.

**`wasmbin`** (Apache-2.0) is ported into `foundation_codegen` (F15) for type-safe,
minimal-diff (`Lazy<T>`) wasm editing — owned, preferred where precise/auditable edits matter;
`walrus` stays for general rewrites and the wasm-bindgen-output boundary. Both coexist; pick per
task. Attribution to the upstream project is mandatory (F15).

### Boundary rules

1. wasm-bindgen usage is **isolated to the integrating crate/mode** and never leaks into the
   shared runtime/build/test path.
2. Such integration points must be **as small as possible** and documented as integration,
   not infrastructure.
3. When choosing between "use wasm-bindgen" and "own it", **own it** unless a third party
   (Cloudflare, an external SDK) genuinely requires wasm-bindgen at that seam.

### Affected features

- F10 (build-pipeline) — owns the production wasm build; shares the LLVM-backend primitive.
- F12 (testbed-native-harness) — owned runners; bindgen-* becomes opt-in.
- F13 (`#[wasm_test]` native discovery) — owned test execution.
- F14 (wasm-bindgen interop boundary) — the only sanctioned wasm-bindgen usage.
- F15 (type-safe WASM/WAT) — `wasmbin` ported into `foundation_codegen` for owned, type-safe edits.
