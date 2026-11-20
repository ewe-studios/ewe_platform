---
feature: "signed-state"
status: "complete"
model: "sonnet"
depends_on: ["01-wire-schema-and-codegen"]
crates: ["foundation_ui_runtime", "foundation_compact", "foundation_ui_wire"]
---

# Feature 04. Get a state signer from the context

A user taps a counter on the web, iOS or Android. Each renderer returns the state bytes attached to that counter.
The host that issued those bytes verifies them before the action reads the count.

A host is the runtime that owns the action and its signing key. A renderer displays components and collects input.
For a server action, the server signs and verifies. For a local action, the local runtime can use its own signer and key.

The component author supplies state when rendering the component. The root receives a signed copy.
A bare `primal::state` marker requests the same bytes on an additional element.
The host signs once. It appends one `AttachState` operation per distinct target before it finishes the frame.
The renderer applies each operation to its target node.

| Renderer | Where an action reads its state |
|---|---|
| Web | The reserved attribute on the element with the event binding. |
| iOS and Android | The attachment on the bound control’s native node record. |
| Desktop | The attachment on the bound control in the desktop renderer. |

For example, root 40 and marked input 42 receive the same bytes. A submit on 42 returns its own copy.
A control without an attachment sends empty state.

### What a signed todo row proves

Suppose the host signs row 42 for Alice when the row is open. Bob later completes it.
Alice’s old attachment can still verify under her binding before it expires. The signature covers the bytes the host issued.
For a consequential change, the handler checks Alice’s current permission and reads the current row.
An application can use the signed value directly for a simple counter.

State and view updates are independent. An action can attach new state to the input and leave the old label visible.
The next submit uses the new attachment on that input. Other controls keep their own earlier attachments until updated.

Cross-site scripting (XSS) means untrusted script runs in the application’s page.
Such a script could trigger Alice’s existing submit control. The normal handler would send the genuine attachment under her active session.
This requires no signing key or change to the signed bytes. A per-user binding can reject Alice’s bytes in Bob’s context.
The application chooses how it handles and renders untrusted content.

## The contract

The host installs a signer in its context (`Ctx`). It can also supply a **signing binding**: stable bytes that restrict where state verifies.
The component’s action binding selects a handler. The signing binding selects which host context can verify its bytes.

The runtime exposes these Rust interfaces.

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

### At host startup

The login worker already creates a host context with `foundation_context::Ctx::try_host()`.
It gives that context to `SessionManager::new`. Its dashboard handler reads the session cookie and calls `get_session`.
A successful lookup gives the handler a trusted `session.user_id`.
See the [worker setup and dashboard handler](../../../../examples/cf-login-app/src/lib.rs) and [session lookup](../../../../backends/foundation_auth/src/shared/session.rs).

Extend that setup as shown below. `App` is the application's long-lived state. The signer is available now. Features 05 and 07 supply the registry and dispatcher.
The host configuration supplies the state signing key. Restarts and replicas that serve the same state need compatible keys.

```rust
let base = foundation_context::Ctx::try_host()?;
let sessions = SessionManager::new(storage, session_config, session_key, base.clone())?;
let signer = HmacStateSigner::new(state_key, state_config)?;
install_state_signer(&base, Arc::new(signer));

let mut registry = ActionRegistry::new();
registry.register(add_todo)?;
let app = App { base, sessions, dispatcher: Dispatcher::new(registry) };
```

The `install_state_signer` helper stores a `SignerEntry` with the supplied signer and an absent binding.

### At the protected route

The application uses the same request helper on the todo page and its action endpoint.
This example uses the worker’s cookie helpers. `RouteError` is the application's error type.

```rust
struct CurrentUser { id: String }

async fn todo_request_ctx(app: &App, req: &web_sys::Request) -> Result<Ctx, RouteError> {
    let cookie = get_cookie_header(req).ok_or(RouteError::LoginRequired)?;
    let token = extract_session_token_from_cookie(&[cookie])
        .ok_or(RouteError::LoginRequired)?;
    let session = app.sessions.get_session(&token).await?
        .ok_or(RouteError::LoginRequired)?;

    let ctx = with_state_binding(&app.base, session.user_id.as_bytes())?;
    ctx.store(CurrentUser { id: session.user_id });
    Ok(ctx)
}
```

`get_session` checks the stored token, revocation and expiry. A missing or invalid session stops the protected route.
The page can redirect to login. The action endpoint can return an unauthenticated error. A storage failure takes the service error path.
The cookie proves the session through lookup. The verified user identifier supplies the signing binding.
The application stores `CurrentUser` on the same fork so its data access code can authorize the list read and later updates.

```rust
// Runtime helper. The fork shares the host services.
fn with_state_binding(base: &Ctx, value: &[u8]) -> Result<Ctx, StateError> {
    let child = base.fork();
    let entry = child.get::<SignerEntry>().ok_or(StateError::MissingSigner)?;
    child.store(SignerEntry {
        signer: Arc::clone(&entry.signer),
        binding: Some(Arc::from(value)),
    });
    Ok(child)
}
```

`Ctx::fork()` copies the typed entry map and shares the existing clock and other services.
A plain clone shares the map. Use the fork before installing anything specific to a request.

### On the first render and the next submit

```rust
// Page handler. The application loads this user's list.
let ctx = todo_request_ctx(app, req).await?;
let user = ctx.get::<CurrentUser>().ok_or(RouteError::LoginRequired)?;
let list = todos.load_for(&user.id).await?;
render_todo_page(&ctx, &list)?;

// Action handler, on a separate request. Authenticate the request again.
let ctx = todo_request_ctx(app, req).await?;
let reply = app.dispatcher.dispatch(&ctx, &connection, &request_frame).await;
```

The render helper uses the signer entry from its argument to sign the list before attaching its bytes.
The action handler supplies its own request context, the negotiated connection and the bounded incoming frame.
The two handlers create separate contexts. Their trusted user identifiers produce equal binding bytes.
A public route can use the base signer directly. A native host can derive a fork from its own authenticated account.
Feature 05 follows the [context through the registry call](../05-action-runtime/feature.md#stage-3-pass-the-request-context-to-the-action).

**Law and test.** Install a small test signer. Send its JSON wrapper through a frame, then verify the returned bytes.
Share that input with the web, Swift and Kotlin renderer tests in features 10, 14 and 15.
Give a parent, child and sibling different attachments. Trigger each node and compare its request with its own bytes.
Mark a sibling to receive the same bytes as the root. Expect those bytes and the sibling’s trigger identity.
Repeat with an unattached child. Expect empty state. With no signer, assert a typed error before calling the action.

Test a missing cookie, revoked session and session-store error. Assert zero list reads, renders and action calls.
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
   The wire writer enforces presence and size. The selected signer validates its own format.
6. Keep the existing `EWS1` fixtures as examples of that format. Test a second signer with a different format.
7. Update the runtime README with one readable counter and one private counter. Run both examples.

**Laws and tests.** Each signer restores the original body. Reject a changed body, wrong binding, expired state or wrong key.
Advance the injected test clock to the expiry time. Check that repeated private signatures use fresh nonces and decrypt to the same value.

## Scope and verification

Request throttling belongs at the host request boundary. A host can reject repeated requests before dispatch.
The signer has no caller-address table. The host owns request throttling.

The built-in signers replace the prototype codec. The old wire fixtures remain readable. Hosts moving from the prototype must issue fresh state or supply a compatible signer.
See the [runtime examples](../../../../backends/foundation_ui_runtime/README.md), [protected route tests](../../../../backends/foundation_ui_runtime/tests/protected_route_tests.rs) and [custom signer fixture](../../../../backends/foundation_ui_wire/fixtures/state_counter_custom.json).
Features 05, 07, 10, 14 and 15 own actual action invocation and renderer event handling.

```sh
cargo test -p foundation_compact --features crypto-chacha --profile uat
cargo test -p foundation_ui_runtime -p foundation_ui_wire --profile uat
cargo check -p foundation_ui_runtime --target wasm32-unknown-unknown
node --test backends/foundation_compact/tests/wasm_hosts/crypto.test.mjs
```
