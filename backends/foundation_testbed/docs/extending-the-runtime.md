# Extending the embedded runtime

Three things you'll typically add: a **Web global** the harness JS needs, a new
**Rust ↔ JS op**, or **Web Crypto** (`crypto.subtle`) — the one capability we
deliberately left out. All of it lives in `src/wasm/embedded_js.rs` behind the
`wasm-embedded-js` feature.

First read the [architecture overview](./embedded-js.md) so the three parts
(composition, bootstrap, ops) and the `loadExtScript` mechanism are familiar.

---

## How the globals bootstrap works

This is the single most-extended seam, so it's worth understanding fully.

When you create a `JsRuntime` from `deno_core` + extensions, you get the engine,
`console`, the module loader, and all the extensions' **ops** (the Rust functions) —
but **not** the Web API *globals* (`TextEncoder`, `setTimeout`, `URL`, `fetch`, …).
Those classes are defined in JavaScript that each extension ships as
**`lazy_loaded_js`**: included in the binary but *not evaluated at startup*, and
written as IIFEs (not ES modules) that return their exports. In the full
`deno_runtime` a large bootstrap (`99_main.js`) evaluates them and assigns the
classes to `globalThis`. We don't pull that runtime, so we run our own tiny
bootstrap — `BOOTSTRAP_GLOBALS` in `embedded_js.rs`, executed once right after the
runtime is built:

```rust
const BOOTSTRAP_GLOBALS: &str = r#"
((globalThis) => {
  const load = globalThis.Deno.core.loadExtScript;
  const enc = load("ext:deno_web/08_text_encoding.js");
  const timers = load("ext:deno_web/02_timers.js");
  const url = load("ext:deno_web/00_url.js");
  const fetchMod = load("ext:deno_fetch/26_fetch.js");
  const headers = load("ext:deno_fetch/20_headers.js");
  const request = load("ext:deno_fetch/23_request.js");
  const response = load("ext:deno_fetch/23_response.js");
  Object.assign(globalThis, {
    TextEncoder: enc.TextEncoder,
    TextDecoder: enc.TextDecoder,
    TextEncoderStream: enc.TextEncoderStream,
    TextDecoderStream: enc.TextDecoderStream,
    setTimeout: timers.setTimeout,
    setInterval: timers.setInterval,
    clearTimeout: timers.clearTimeout,
    clearInterval: timers.clearInterval,
    URL: url.URL,
    URLSearchParams: url.URLSearchParams,
    fetch: fetchMod.fetch,
    Headers: headers.Headers,
    Request: request.Request,
    Response: response.Response,
  });
})(globalThis);
"#;
```

Line by line:

- **`Deno.core.loadExtScript(specifier)`** is the deno_core runtime API that
  evaluates a `lazy_loaded_js` file *on demand* and returns its export object (the
  thing the IIFE `return {…}`s). The Rust side is `op_load_ext_script`.
- The **`ext:<crate>/<file>.js`** specifier names a file the extension registered.
  Find the available files + their export names in the crate source (each IIFE ends
  with `return { … }`): `~/.cargo/registry/src/*/deno_web-*/` and `…/deno_fetch-*/`.
- These IIFEs read **`__bootstrap`** (which holds `core` + `primordials`). `deno_core`
  provides it, and because we never run `99_main.js` (which would delete it), it's
  still present when `loadExtScript` evaluates them. Modules also pull their own
  transitive deps via `loadExtScript` internally (e.g. `deno_fetch` modules load
  `deno_web`/`deno_webidl` ones), so you only list the *top-level* globals you want.
- **`Object.assign(globalThis, …)`** publishes the classes as real globals, so test
  code and `foundation-wasm.js` can use `new TextEncoder()`, `fetch(...)`, etc.

Three invariants when extending it:

1. The backing **ops must be registered** — i.e. the owning extension is in the
   `extensions` vec in `build_runtime_with` (`deno_web`, `deno_fetch`, …). If a
   global's ops aren't registered, it loads but throws when *used*.
2. Some extensions need **runtime state in `OpState`** (e.g. `deno_fetch` needs a
   `PermissionsContainer` — see the fetch wiring in `build_runtime_with`). Add it
   before the global is *called*, not necessarily before it's defined.
3. Keep it a **plain script** (`execute_script`), not a module — it runs before any
   `runner.mjs`/entry module loads, so globals exist by the time tests run.

## Adding a Web global

To expose another API an extension already implements (e.g. `performance`,
`structuredClone`, `Blob` from `deno_web`):

1. Find the module + export name in the crate source (the IIFE's `return {…}`).
   `deno_web` ships, among others, `15_performance.js` (`performance`),
   `02_structured_clone.js` (`structuredClone`), `09_file.js` (`Blob`/`File`).
2. Add two lines to `BOOTSTRAP_GLOBALS`:

   ```js
   const perf = load("ext:deno_web/15_performance.js");
   Object.assign(globalThis, { performance: perf.performance });
   ```

No Rust change is needed if the backing ops are already registered by the
extension's `init(...)` (they are, for everything `deno_web`/`deno_fetch` ship). If
the global needs an op from an extension we *don't* yet include, or needs extension
state we don't `put`, that's "new extension" territory — see the next section, and
the [fetch wiring](#a-worked-example-how-fetch-was-added) for a full example of
both (new extension + `OpState` state).

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

## A worked example: how `fetch` was added

`fetch` is *not* in `deno_web` — it's a separate, heavier extension (`deno_fetch`)
that also needs runtime state (`OpState`). It's the complete pattern for "add a whole
extension," so it's worth walking through. All of this is live in `embedded_js.rs`.

1. **Add the crates** to the `wasm-embedded-js` feature + `[dependencies]` — versions
   coordinated with `deno_core` 0.404 (verify a single `deno_core` with
   `cargo tree -i deno_core`):

   ```toml
   deno_fetch = { version = "0.275", optional = true }        # fetch/Request/Response/Headers
   deno_net   = { version = "0.243", optional = true }        # net/TLS JS deno_fetch lazy-loads
   deno_permissions = { version = "0.110", optional = true }   # op_fetch's PermissionsContainer
   sys_traits = { version = "0.1", features = ["real", "libc"], optional = true }  # RealSys
   rustls = { version = "0.23", default-features = false, features = ["aws_lc_rs"], optional = true }
   ```

   > `deno_net` is needed even for `data:` fetches: deno_fetch's `22_http_client.js`
   > lazy-loads `ext:deno_net/02_tls.js`, so that extension must be registered or the
   > whole `fetch` module fails to load. `sys_traits` needs `real` + `libc` (unix) so
   > `RealSys` implements `EnvHomeDir` for the descriptor parser.

2. **Register the extensions** in `build_runtime_with`'s `extensions` vec —
   `deno_net` before `deno_fetch`; both have an `init`. `deno_net::init` takes
   `(root_cert_store_provider, unsafely_ignore_certificate_errors)` (both `None`),
   `deno_fetch::init` takes an `Options` (it implements `Default`):

   ```rust
   deno_net::deno_net::init(None, None),
   deno_fetch::deno_fetch::init(deno_fetch::Options::default()),
   ```

3. **Install the TLS CryptoProvider.** `deno_tls` (under deno_fetch's HTTP client)
   uses rustls, which panics without a process-default `CryptoProvider` — in the full
   runtime deno installs it; we must. Idempotent, once per process, before building:

   ```rust
   static ONCE: std::sync::Once = std::sync::Once::new();
   ONCE.call_once(|| { let _ = rustls::crypto::aws_lc_rs::default_provider().install_default(); });
   ```

4. **Provide the state the ops need.** `op_fetch`/`op_net` read a
   `deno_permissions::PermissionsContainer` from `OpState` (`check_net_url`). Local
   test runtime → grant everything; `allow_all` still needs a descriptor parser
   structurally (`RealSys` is the standard host one). Inject after `JsRuntime::new`,
   before anything calls `fetch`:

   ```rust
   let parser = Arc::new(deno_permissions::RuntimePermissionDescriptorParser::new(
       sys_traits::impls::RealSys,
   ));
   runtime
       .op_state()
       .borrow_mut()
       .put(deno_permissions::PermissionsContainer::allow_all(parser));
   ```

   (`runtime.op_state().borrow_mut().put(…)` is the alternative to an extension
   `state =` closure — handy when the value isn't an extension option.)

5. **Shim the telemetry bootstrap.** deno_fetch's `26_fetch.js` destructures
   `__bootstrap.internals.__telemetry` / `.__telemetryUtil` (the full runtime's
   OpenTelemetry bootstrap). We don't run OTel, so install a no-op shim *before*
   loading the fetch JS — `TRACING_ENABLED: false` gates the real span code, so the
   no-ops are never called, they just have to exist:

   ```js
   const internals = globalThis.__bootstrap.internals;
   internals.__telemetry ??= { TRACING_ENABLED: false, PROPAGATORS: [],
     builtinTracer: () => ({ startSpan: () => ({ end(){}, setAttribute(){}, recordException(){}, setStatus(){} }) }),
     ContextManager: undefined, enterSpan: () => undefined, restoreSnapshot: () => undefined };
   internals.__telemetryUtil ??= { updateSpanFromClientResponse(){}, updateSpanFromError(){}, updateSpanFromRequest(){} };
   ```

6. **Globalize the JS** in `BOOTSTRAP_GLOBALS` (deno_fetch's `lazy_loaded_js`):

   ```js
   const fetchMod = load("ext:deno_fetch/26_fetch.js");   // fetch
   const headers  = load("ext:deno_fetch/20_headers.js"); // Headers
   const request  = load("ext:deno_fetch/23_request.js"); // Request
   const response = load("ext:deno_fetch/23_response.js");// Response
   Object.assign(globalThis, {
     fetch: fetchMod.fetch, Headers: headers.Headers,
     Request: request.Request, Response: response.Response,
   });
   ```

7. **Test it hermetically** with a `data:` URL (no network):

   ```rust
   // tests/embedded_js_tests.rs
   run_module(entry_with(r#"
       const res = await fetch("data:text/plain,hello-embedded");
       if ((await res.text()) !== "hello-embedded") throw new Error("bad fetch");
   "#))?;
   ```

**Lesson:** a "whole extension" can drag in the full runtime's assumptions — a
companion extension (`deno_net`), a host resource (rustls provider), `OpState` state
(permissions), and even a bootstrap shim (telemetry). When adding one, expect to
chase a short chain of "X cannot be lazy-loaded" / "Y is undefined" / "Z provider
not installed" errors; each names exactly the next piece. The general shape stays:
**crates → `init`s in the vec → host resources/state → bootstrap shims → globalize →
hermetic test.** `crypto` (below) follows the same shape.

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
