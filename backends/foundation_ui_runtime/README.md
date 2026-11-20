# User interface state signers

A host installs a signer in its context at startup. A component can sign its serialized state with that context.
The renderer stores the resulting bytes on the marked control and returns them on submit.
The host verifies the returned bytes before decoding the component state.

| Signer | What the recipient can read |
|---|---|
| `HmacStateSigner` | The original body. A keyed hash (HMAC-SHA256) authenticates the header, time window, binding and body. |
| `ChaChaStateSigner` | A format identifier, key identifier, nonce, ciphertext and authentication tag. The timestamps and body are encrypted. |
| Application implementation of `StateSigner` | The application chooses the format. The wire copies its bytes unchanged. |

## A readable counter

The host supplies a secret 32-byte key. Hosts that verify each other's state need compatible keys.
This runnable example generates a key for a single run. The request handler obtains Alice's identifier from its authenticated session.

```rust
use std::sync::Arc;
use foundation_compact::rng::Rng;
use foundation_context::Ctx;
use foundation_ui_runtime::state::{
    install_state_signer, sign_state, verify_state, with_state_binding, HmacStateSigner, StateConfig,
};

let base = Ctx::try_host()?;
let mut key = [0; 32];
base.rng().lock().unwrap().fill_bytes(&mut key);
install_state_signer(&base, Arc::new(HmacStateSigner::new(key, StateConfig::default())?));

let render = with_state_binding(&base, b"Alice")?;
let signed = sign_state(&render, br#"{"count":4}"#)?;
let submit = with_state_binding(&base, b"Alice")?;
assert_eq!(verify_state(&submit, &signed)?, br#"{"count":4}"#);
assert!(verify_state(&base, &signed).is_err());
# Ok::<(), Box<dyn std::error::Error>>(())
```

A plain context clone shares its entries. `with_state_binding` forks the entries for the request and shares the signer, clock and random source.
A persistent host can reuse its main context for every call. Use the base context for public state. An absent binding differs from an empty binding.

## A private counter

This example uses the host context for public access to an encrypted counter. Each issue draws a fresh nonce.

```rust
use std::sync::Arc;
use foundation_compact::rng::Rng;
use foundation_context::Ctx;
use foundation_ui_runtime::state::{
    install_state_signer, sign_state, verify_state, ChaChaStateSigner, StateConfig,
};

let ctx = Ctx::try_host()?;
let mut key = [0; 32];
ctx.rng().lock().unwrap().fill_bytes(&mut key);
install_state_signer(&ctx, Arc::new(ChaChaStateSigner::new(key, StateConfig::default())?));
let body = br#"{"count":4}"#;
let first = sign_state(&ctx, body)?;
let second = sign_state(&ctx, body)?;
assert_ne!(first, second);
assert_eq!(verify_state(&ctx, &second)?, body);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Both built-in signers default to a 24-hour lifetime and a 4096-byte body limit. Configure these with `StateConfig::ttl` and `max_body_bytes`.
The expiry instant is excluded. Both signers verify using the clock in the supplied context.
Use `from_key_ring` to issue with the current key and accept a previous key during rotation.
Use separate keys for separate purposes. `Ctx::try_host()` reports an error when it cannot obtain entropy.
Choose the compact WebAssembly host backend that matches the embedding application. Fixed seeds are for tests.

The [protected-route tests](tests/protected_route_tests.rs) show session lookup and signing on separate page and submit requests.

## Format and verification

The readable format starts with `EWH1`. The private format starts with `EWC1`.
The signer authenticates the binding's presence and length as well as its contents.
Renderers carry these bytes in `AttachState.state` and `ActionRequest.state` without parsing their format.
The older `EWS1` fixtures remain readable through the wire crate’s independent `read_signed_state` function.
The built-in signers issue a new format. Render fresh state when moving from the prototype.

A signed row identifier tells the handler which row to load. The application checks current permissions and decides whether stale state is acceptable.
The signer can also protect a complete counter value when the application chooses that policy.

The [application signer test](tests/signer_context_tests.rs) signs a readable counter in a JSON wrapper.
Its [shared fixture](../foundation_ui_wire/fixtures/state_counter_custom.json) contains the exact bytes used in the wire tests.

```sh
cargo test -p foundation_ui_runtime -p foundation_ui_wire --profile uat
cargo test -p foundation_compact --features crypto-chacha --profile uat
node --test backends/foundation_compact/tests/wasm_hosts/crypto.test.mjs
```
