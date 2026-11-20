---
feature: "signed-state"
status: "revised plan for review"
model: "sonnet"
depends_on: ["01-wire-schema-and-codegen"]
crates: ["foundation_ui_runtime", "foundation_compact", "foundation_ui_wire"]
---

# Feature 04. Get a state signer from the context

A user taps a counter on the web, iOS or Android. Each renderer returns the state bytes attached to that counter.
The host that issued those bytes verifies them before the action reads the count.

A host is the runtime that owns the action and its signing key. A renderer displays components and collects input.
For a server action, the server signs and verifies. For a local action, the local runtime can use its own signer and key.

The component author chooses which element stores the state. We call that element the **state owner**.
The action binding selects the owner for that trigger. Without an explicit reference, the triggering element is the owner.

| Renderer | Default lookup | Binding with an explicit owner |
|---|---|---|
| Web | Read the bound element’s state attribute. | Resolve the owner in this view’s node registry. Read that element’s state attribute. |
| iOS (SwiftUI) | Read the control’s Swift node record. | Read the named owner’s record in the same node store. |
| Android (Compose) | Read the control’s Kotlin node record. | Read the named owner’s record in the same node store. |

For example, the author attaches counter state to node 40. Node 40 can trigger its own action.
The author can also bind a separate button, node 42, to use node 40 as its state owner.
The button can be a child or a sibling. Its request identifies trigger 42 and carries the exact bytes stored on node 40.
A state update for that counter goes back to node 40.

With no explicit reference and no attachment on the trigger, send empty state. Never search the tree for a stateful ancestor.
An explicit owner must resolve inside the current user’s view. If it is missing, report a binding error before sending the action.

## The contract

The host installs a signer in its context (`Ctx`). It can also supply a **signing binding**: stable bytes that restrict where state verifies.
The component’s action binding selects a node. The signing binding selects which host context can verify its bytes.

These are proposed Rust interfaces.

```rust
pub trait StateSigner: Send + Sync {
    fn sign(&self, ctx: &Ctx, binding: Option<&[u8]>, body: &[u8])
        -> Result<Vec<u8>, StateError>;

    fn verify(&self, ctx: &Ctx, binding: Option<&[u8]>, signed: &[u8])
        -> Result<Vec<u8>, StateError>;
}

pub struct SignerEntry {
    signer: Arc<dyn StateSigner>,
    binding: Option<Arc<[u8]>>,
}
```

| Host policy | Binding supplied by the host | Effect |
|---|---|---|
| Public counter | `None` | The issuing signer can verify the public state without a caller identity. |
| Per-user state | Authenticated user identifier | The same user can return the state across sign-ins while the key and state remain valid. |
| Per-session state | Current trusted session identifier | A new session cannot use the old session’s state. |

The host can use another application value. It must obtain the expected value independently of the submitted state.
A binding can be public. The signing key remains secret.

| Host choice | What each renderer receives | What verification does |
|---|---|---|
| `HmacStateSigner` | Readable state and a tag. HMAC means hash-based message authentication code. | Check the tag and binding. Return the original state bytes. |
| `ChaChaStateSigner` | State encrypted with ChaCha20-Poly1305, a nonce and an authentication tag. A nonce is a value used once per key. | Authenticate the binding and encrypted state. Return the decrypted bytes. |
| Application signer | Bytes in the application's format, such as a JavaScript Object Notation (JSON) wrapper. | Apply the same contract using that format. |

A readable signer can put `{"count":4}` in a wrapper with its tag.
It must define the exact bytes covered by the tag. A change to the count must fail verification.
Each renderer stores and returns the original wrapper bytes.

## Stage 1. Install the signer and use it

At startup, install one signer with an absent binding. For public state, use this context directly.
For scoped state, fork the context and replace its signer entry. `Ctx::fork()` copies the entry map and shares the existing services.

```rust
// Proposed helper in foundation_ui_runtime. The key stays with the signer.
fn with_state_binding(base: &Ctx, value: &[u8]) -> Result<Ctx, StateError> {
    let entry = base.get::<SignerEntry>().ok_or(StateError::MissingSigner)?;
    let child = base.fork();
    child.store(SignerEntry {
        signer: Arc::clone(&entry.signer),
        binding: Some(Arc::from(value)),
    });
    Ok(child)
}
```

The host runs the same policy before rendering a component and before dispatching a later action.
For example, a signed-in route requires an authenticated user and derives its context from that user’s identifier:

```rust
// Initial render
let render_ctx = with_state_binding(&base, authenticated_user.id_bytes())?;
render_counter(&render_ctx, counter)?;

// A later request. Authentication resolves the user again.
let action_ctx = with_state_binding(&base, request_user.id_bytes())?;
dispatcher.dispatch(&action_ctx, request).await?;
```

Inside the runtime, rendering calls `entry.signer.sign(ctx, entry.binding.as_deref(), body)`.
Dispatch calls `entry.signer.verify(ctx, entry.binding.as_deref(), signed)` before decoding state.
It passes the same context to the action and uses it to sign the next state when it drains the receiver.
The component sees its ordinary typed state on web, iOS and Android.

**Law and test.** Install a small test signer. Send its JSON wrapper through a frame, then verify the returned bytes.
Share that input with the web, Swift and Kotlin renderer tests in features 10, 14 and 15.
Give a parent, child and sibling different attachments. Trigger each node and compare its request with its own bytes.
Bind a sibling explicitly to the parent. Expect the parent’s bytes and the sibling’s trigger identity.
Repeat with an unattached child and no explicit owner. Expect empty state. With no signer, assert a typed error before calling the action.

Derive two contexts from one base for users A and B. Render for A, dispatch as A and verify A’s next state.
Verification as B or with the public context must fail. Public state must round-trip with the base context.
Run both users concurrently. Confirm that neither request changes the base or the other user’s binding.

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
| Native server or on-device Rust host (iOS and Android) | The host seeds the context from operating system entropy. |
| Browser WebAssembly (Wasm) with `wasm-bindgen` | The host uses Web Crypto through the JavaScript bindings. |
| Wasm with `host-abi` | The embedding application supplies the entropy import through its application binary interface (ABI). |
| Host without secure entropy | Forward signing to the server. Propagate a local initialization error if local encryption is required. |

Use a fixed seed only in tests. A renderer that only returns state bytes needs no randomness for this task.

**Law and tests.** The same test key, nonce and body give the same encrypted bytes. Decryption restores the body.
A changed tag fails. Exercise the native helper and each supported Wasm host path. A target build alone cannot test an entropy import.

## Stage 3. Implement and connect the two signers

1. Put the signers in `foundation_ui_runtime::state`. Reuse the existing HMAC verification and compact encryption helpers.
2. Authenticate the body, binding and expiry. Both built-in signers use the injected clock and configurable lifetime. The default is 24 hours.
3. Encode binding presence and length explicitly. Distinguish an absent binding from a supplied empty value. For ChaCha, authenticate the binding inside the encrypted payload or as associated data.
4. Check configured size limits before allocating or decoding. Return the body only after all checks pass.
5. Treat `AttachState.state` and `ActionRequest.state` as byte vectors in the wire writer, schema comments and package guide.
   The current writer requires an `EWS1` table with a tag. Move format validation into the selected signer.
6. Keep the existing `EWS1` fixtures as examples of that format. Test a second signer with a different format.
7. Update the runtime README with one readable counter and one private counter. Run both examples.

**Laws and tests.** Each signer restores the original body. Reject a changed body, wrong binding, expired state or wrong key.
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
