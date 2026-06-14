# Extending the embedded runtime

Three things you'll typically add: a **Web global** the harness JS needs, a new
**Rust ↔ JS op**, or **Web Crypto** (`crypto.subtle`) — the one capability we
deliberately left out. All of it lives in `src/wasm/embedded_js.rs` behind the
`wasm-embedded-js` feature.

First read the [architecture overview](./embedded-js.md) so the three parts
(composition, bootstrap, ops) and the `loadExtScript` mechanism are familiar.

---

## Adding a Web global from `deno_web`

`deno_web` already *implements* a lot (encoding, timers, URL, streams, blobs,
structured clone, `performance`, compression, broadcast channel) — it just ships
them as `lazy_loaded_js` IIFE modules that aren't auto-installed as globals. To
expose one, load its module in the bootstrap and assign the export.

1. Find the module + export name. The modules live in the `deno_web` crate source
   (`~/.cargo/registry/src/*/deno_web-*/`). Each ends with `return { … }` listing
   its exports. Examples already wired: `08_text_encoding.js`, `02_timers.js`,
   `00_url.js`.

2. Add to `BOOTSTRAP_GLOBALS` in `embedded_js.rs`:

   ```js
   const perf = Deno.core.loadExtScript("ext:deno_web/15_performance.js");
   Object.assign(globalThis, { performance: perf.performance });
   ```

`loadExtScript` returns the module's export object and pulls the module's own
transitive deps (e.g. `deno_webidl`) automatically; `__bootstrap` (core +
primordials) is provided by `deno_core`. That's it — no Rust change, because the
backing ops are already registered by `deno_web::deno_web::init(...)`.

> If a global needs an op `deno_web` doesn't register in our `init` call, or needs
> extension state we didn't `put`, you've crossed into "new extension" territory —
> see the next section.

---

## Adding a new op (Rust ↔ JS)

Ops are how JS calls into Rust. The harness uses two (`op_fwt_read_file`,
`op_fwt_report`); add yours the same way.

1. **Write the op** with `#[op2]`. Note the marshalling annotations:

   ```rust
   use deno_core::{op2, OpState};
   use deno_core::convert::Uint8Array;
   use deno_error::JsErrorBox;

   // Sync, fallible, returns bytes as a Uint8Array:
   #[op2]
   fn op_fwt_read_file(#[string] path: String) -> Result<Uint8Array, JsErrorBox> {
       std::fs::read(&path)
           .map(Uint8Array::from)
           .map_err(|e| JsErrorBox::generic(format!("op_fwt_read_file({path}): {e}")))
   }

   // Fast op with mutable engine state (capture into OpState):
   #[op2(fast)]
   fn op_fwt_report(state: &mut OpState, #[string] json: String) {
       *state.borrow::<ReportSink>().0.borrow_mut() = Some(json);
   }
   ```

   Annotation cheatsheet (`#[op2]`): `#[string]` for `String`/`&str`, `#[buffer]`
   for `&[u8]`/`&mut [u8]`, `#[serde]` for serde types, `#[smi]` for small ints.
   `OpState` **must** be a bare identifier (`state: &mut OpState`) — `#[op2]`
   pattern-matches the token, so `deno_core::OpState` is rejected. Mark an op
   `#[op2(fast)]` only if the compiler tells you it's fast-compatible.

2. **Register it** on an extension. For harness ops, add to the existing
   `foundation_fwt` extension's `ops = [ … ]`:

   ```rust
   deno_core::extension!(
       foundation_fwt,
       ops = [op_fwt_read_file, op_fwt_report, /* your_op */],
       options = { sink: ReportSink },
       state = |state, options| { state.put(options.sink); },
   );
   ```

   If your op needs shared Rust state, model it like `ReportSink`
   (`#[derive(Clone)] struct Holder(Rc<RefCell<T>>)`): keep a clone in the driver,
   pass one into `init(...)`, read it back after the event loop drains.

3. **Call it from JS.** Ops live at `globalThis.Deno.core.ops.<name>`. Keep the
   runner host-agnostic by feature-detecting:

   ```js
   const op = globalThis.Deno?.core?.ops?.op_fwt_read_file;
   const bytes = op ? new Uint8Array(op(path)) : await fetch(url).then(r => r.arrayBuffer());
   ```

Errors: return `Result<T, deno_error::JsErrorBox>` and build messages with
`JsErrorBox::generic("…")` (a custom `#[derive(deno_error::JsError)]` enum works
too). `std::io::Error` is **not** accepted directly — map it.

---

## Adding Web Crypto (`crypto.subtle`)

We omitted `deno_crypto` on purpose: pulling it (via the full `deno_runtime`) forces
`aes = "=0.8.3"`, which can't coexist with the workspace's `aes 0.8.4`
([why](./embedded-js-internals.md#the-aes--deno_crypto-conflict)). The harness
doesn't need crypto today, but when you want to test wasm/JS that *uses*
`crypto.subtle.digest`, `crypto.getRandomValues`, key derivation, etc., here are the
two paths — pick based on whether the `aes` pin has been resolved.

Gate whichever you choose behind a further sub-feature so a plain in-process JS
build stays free of the crypto tree:

```toml
# Cargo.toml
[features]
wasm-embedded-crypto = ["wasm-embedded-js", /* the crypto deps below */]
```

### Path A (preferred, once the conflict is gone) — the real `deno_crypto`

Viable only when a `deno_crypto` release exists whose `aes` requirement unifies with
turso's `aes 0.8.4` (i.e. it's no longer the exact `=0.8.3` pin). Check first:

```bash
# inspect the aes requirement per published version
cat ~/.cargo/registry/index/*/.cache/de/no/deno_crypto | tr '\0' '\n' \
  | grep -o '"vers":"[^"]*"\|{"name":"aes"[^}]*}'
```

If a compatible version exists, wire it exactly like `deno_web`:

1. `deno_crypto = { version = "<compatible>", optional = true }`, add `dep:deno_crypto`
   to `wasm-embedded-crypto`.
2. Add its extension to the `extensions` vec in `build_runtime_with`
   (`deno_crypto::deno_crypto::init(<seed/options>)` — check its `extension!` block
   for the exact init signature, the same way we did for `deno_web`).
3. Globalize `crypto` in `BOOTSTRAP_GLOBALS` via `loadExtScript("ext:deno_crypto/<file>.js")`
   (find the module + export name in the crate source).

This gets the real, spec-complete Web Crypto. The cost is the heavier dep tree and
the version coupling — bump `deno_crypto` together with the rest of the deno set
(see below).

### Path B (works today, conflict-free) — a small op-backed shim

Provide just the primitives you test, using RustCrypto crates already resolvable in
the workspace (no new `aes` pin). Concrete, copy-pasteable:

```rust
// deps (wasm-embedded-crypto): getrandom = "0.2", sha2 = "0.10"
use deno_core::convert::Uint8Array;
use deno_core::op2;

#[op2(fast)]
fn op_fwt_random_fill(#[buffer] buf: &mut [u8]) {
    getrandom::getrandom(buf).expect("getrandom");
}

#[op2]
fn op_fwt_sha256(#[buffer] data: &[u8]) -> Uint8Array {
    use sha2::{Digest, Sha256};
    Uint8Array::from(Sha256::digest(data).to_vec())
}
```

Register them on `foundation_fwt` (or a dedicated `foundation_crypto` extension),
then install a `crypto` global in the bootstrap:

```js
globalThis.crypto = {
  getRandomValues(arr) {
    Deno.core.ops.op_fwt_random_fill(arr);   // fills the typed array in place
    return arr;
  },
  subtle: {
    async digest(algorithm, data) {
      const name = typeof algorithm === "string" ? algorithm : algorithm?.name;
      const bytes = new Uint8Array(data.buffer ?? data);
      if (name === "SHA-256") return Deno.core.ops.op_fwt_sha256(bytes).buffer;
      throw new Error("unsupported digest: " + name);
    },
  },
};
```

Extend the `subtle` surface (other hashes, HMAC, AES) by adding one op per primitive
backed by the matching RustCrypto crate. This keeps the dependency footprint tiny
and dodges the `aes` conflict entirely — at the cost of implementing only what you
actually exercise.

### Recommendation

Use **Path B** until you genuinely need broad Web Crypto coverage; it's
conflict-free and ships only what your tests use. Move to **Path A** when the
`deno_crypto`/`aes` situation resolves and you want spec-complete behaviour. Either
way, record the change in `specifications/44-embedded-deno-runtime/spec.md` §4.4
(which tracks this deferral).

---

## Bumping the deno crates

The deno ecosystem releases in lockstep — `deno_core`, `deno_web`, `deno_webidl`
(and `deno_crypto`, if added) are tightly version-coupled to one V8. When bumping:

1. Bump `deno_core` first; find the matching extension versions (the set that
   resolves to a **single** `deno_core` in the lock — verify with
   `cargo tree -i deno_core`).
2. Watch for API drift — past breaks we hit: `JsRuntime::handle_scope` was removed
   (use `deno_core::scope!(scope, rt)`); the `extension!` init fn is `init()` (not
   `init_ops_and_esm`); `lazy_loaded_js` module filenames/exports can change (the
   bootstrap's `ext:deno_web/NN_*.js` paths).
3. Re-run the suite on the `uat` profile:
   `cargo test -p foundation_testbed --no-default-features --features wasm-embedded-js --profile uat`.
   The W1 spike tests are the fastest "does V8 still link + run" signal.
