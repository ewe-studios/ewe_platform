---
feature: "signed-state"
status: "revised plan for review"
model: "sonnet"
depends_on: ["01-wire-schema-and-codegen"]
crates: ["foundation_ui_runtime", "foundation_compact", "foundation_ui_wire"]
---

# Feature 04. Get a state signer from the context

A counter page sends its state to the browser. On the next click, the browser sends those bytes back.
The server must know that it issued the state before it uses the count.

## The contract

The host installs one `StateSigner` in its context (`Ctx`). The dispatcher uses two methods.
These are proposed Rust interfaces.

```rust
pub trait StateSigner: Send + Sync {
    fn sign(&self, ctx: &Ctx, binding: &StateBinding, body: &[u8])
        -> Result<Vec<u8>, StateError>;

    fn verify(&self, ctx: &Ctx, binding: &StateBinding, signed: &[u8])
        -> Result<Vec<u8>, StateError>;
}

pub struct StateBinding {
    pub session: String,
    pub purpose: String,
}

pub struct SignerEntry(pub Arc<dyn StateSigner>);
```

A binding identifies where the state belongs. The server supplies the session from the authenticated request.
The application supplies a stable purpose, such as `"counter"`, for both rendering and handling that component.
The server uses these expected values when it verifies the bytes.

| Host choice | What the browser receives | What verification does |
|---|---|---|
| `HmacStateSigner` | Readable state and a tag. HMAC means hash-based message authentication code. | Check the tag and binding. Return the original state bytes. |
| `ChaChaStateSigner` | State encrypted with ChaCha20-Poly1305, a nonce and an authentication tag. A nonce is a value used once per key. | Authenticate the binding and encrypted state. Return the decrypted bytes. |
| Application signer | Bytes in the application's format, such as a JavaScript Object Notation (JSON) wrapper. | Apply the same contract using that format. |

A readable signer can put `{"count":4}` in a wrapper with its tag.
It must define the exact bytes covered by the tag. A change to the count must fail verification.
The browser stores and returns the original wrapper bytes.

## Stage 1. Install the signer and use it

At startup, the host creates the signer with its key. The existing typed map in `foundation_context::Ctx` stores the entry.
The runtime adds a lookup helper. A missing entry returns `StateError::MissingSigner`.

```rust
fn state_signer(ctx: &Ctx) -> Result<Arc<dyn StateSigner>, StateError> {
    let entry = ctx.get::<SignerEntry>().ok_or(StateError::MissingSigner)?;
    Ok(Arc::clone(&entry.0))
}
```

```rust
// Host startup. This host installs its signer once.
let ctx = Ctx::try_host()?;
let signer = HmacStateSigner::new(key, config)?;
ctx.store(SignerEntry(Arc::new(signer)));

// Initial render. The dispatcher puts these bytes in AttachState.state.
let binding = StateBinding { session: session.id().to_owned(), purpose: "counter".into() };
let signer = state_signer(&ctx)?;
let signed = signer.sign(&ctx, &binding, br#"{"count":4}"#)?;

// Later request. The dispatcher obtains the same binding from trusted inputs.
let body = signer.verify(&ctx, &binding, &request_state)?;
let counter: Counter = serde_json::from_slice(&body)?;
```

`request_state` contains the bytes the client returned in `ActionRequest.state`. The action receives `counter`.
The template and the browser need no server key.

**Law and test.** Install a small test signer. Send its JSON wrapper through a frame, then verify the returned bytes.
Assert byte equality at each boundary. With no signer, assert a typed error before calling the action.

## Stage 2. Isolate the compact cryptography

`foundation_compact::crypto::chacha` already supplies `encrypt_with_nonce` and `decrypt` under the `crypto-chacha` feature.
The explicit nonce lets the host control randomness. The compact library encrypts bytes. It knows nothing about components, sessions or frames.

```rust
let encrypted = encrypt_with_nonce(&key, nonce, body)?;
let recovered = decrypt(&key, &encrypted)?;
assert_eq!(recovered.as_slice(), body);
```

| Execution environment | Source of fresh randomness |
|---|---|
| Native server | The host seeds the context from operating system entropy. |
| Browser WebAssembly (Wasm) with `wasm-bindgen` | The host uses Web Crypto through the JavaScript bindings. |
| Wasm with `host-abi` | The embedding application supplies the entropy import through its application binary interface (ABI). |
| Host without secure entropy | Forward signing to the server. Propagate a local initialization error if local encryption is required. |

Use a fixed seed only in tests. A browser that only returns signed state needs neither key access nor an entropy import for this task.

**Law and tests.** The same test key, nonce and body give the same encrypted bytes. Decryption restores the body.
A changed tag fails. Exercise the native helper and each supported Wasm host path. A target build alone cannot test an entropy import.

## Stage 3. Implement and connect the two signers

1. Put the signers in `foundation_ui_runtime::state`. Reuse the existing HMAC verification and compact encryption helpers.
2. Authenticate the body, binding and expiry. Both built-in signers use the injected clock and configurable lifetime. The default is 24 hours.
3. Use an unambiguous encoding for variable-length fields. For ChaCha, authenticate the binding inside the encrypted payload or as associated data.
4. Check configured size limits before allocating or decoding. Return the body only after all checks pass.
5. Treat `AttachState.state` and `ActionRequest.state` as byte vectors in the wire writer, schema comments and package guide.
   The current writer requires an `EWS1` table with a tag. Move format validation into the selected signer.
6. Keep the existing `EWS1` fixtures as examples of that format. Test a second signer with a different format.
7. Update the runtime README with one readable counter and one private counter. Run both examples.

**Laws and tests.** Each signer restores the original body. Reject a changed body, wrong session, wrong purpose, expired state or wrong key.
Advance the injected test clock to the expiry time. Check that repeated private signatures use fresh nonces and decrypt to the same value.

## Scope and verification

Request throttling belongs at the host request boundary. A host can reject repeated requests before dispatch.
The signer has no caller-address table. Remove the prototype limiter dependency from the dispatcher plan when adopting this revision.

The existing implementation uses `StateCodec` and a fixed envelope. Its passing tests describe that prototype.
Approval of this plan starts the migration. Update features 03 and 07 and their tests in the same implementation.

```sh
cargo test -p foundation_compact --features crypto-chacha --profile uat
cargo test -p foundation_ui_runtime -p foundation_ui_wire --profile uat
cargo check -p foundation_ui_runtime --target wasm32-unknown-unknown
```
