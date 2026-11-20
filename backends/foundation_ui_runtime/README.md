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

A signed row identifier tells the handler which row to load. The application checks current permissions and decides whether stale state is acceptable.
The signer can also protect a complete counter value when the application chooses that policy.

The [application signer test](tests/signer_context_tests.rs) signs a readable counter in a JSON wrapper.
Its [shared fixture](../foundation_ui_wire/fixtures/state_counter_custom.json) contains the exact bytes used in the wire tests.

```sh
cargo test -p foundation_ui_runtime -p foundation_ui_wire --profile uat
cargo test -p foundation_compact --features crypto-chacha --profile uat
node --test backends/foundation_compact/tests/wasm_hosts/crypto.test.mjs
```

## Register and run an action

An action receives the state attached to the control that triggered it. The registry entry selects the Rust type used to decode that state.
The host authenticates the request and supplies the context. A persistent host can reuse that context for many calls.
Each call has a separate output receiver.

This action saves a new counter value on the triggering node. It sends no view change.

```rust
use foundation_ui_runtime::action;
use foundation_ui_runtime::action::{ActionRegistry, ActionResult, Context, Interaction};

#[action(server)]
async fn increment(ctx: &Context, event: Interaction<u32>) -> ActionResult {
    ctx.receiver().state(event.node_id, &event.state.saturating_add(1))
}

let mut registry = ActionRegistry::new();
registry.register(increment)?;
# Ok::<(), foundation_ui_runtime::action::RegistrationError>(())
```

The attribute turns the function into a value named `increment`. The application registers that value once.
The registry decodes verified state as the type inside `Interaction<T>`. Use `Interaction<()>` for an action without state.

| Attribute | Where the action can run |
|---|---|
| `#[action]` | Prefer the local host. Allow an advertised server implementation as a fallback. |
| `#[action(server)]` | Server |
| `#[action(host)]` | Local host |
| `#[action(host, web)]` | Local host on a web surface |
| `#[action(server, mobile)]` | Server for a mobile surface |

Either explicit origin accepts the optional `web` or `mobile` scope.

The manager takes a reference such as `server:increment`, the triggering node, its input and its signed bytes.
A transport returns `Some(frame)` for an immediate reply or `None` after a socket accepts the message.
A client without the implementation can declare an unqualified action with `registry.declare(None, "add_todo", Scope::All)?`.
The manager can then route it to an advertised server implementation.

The host checks the reference and selects the entry with `registry.get(origin, name, surface)?`.
It passes the context, entry, surface, trusted render target and owned request to `invoke_action`.
The helper verifies the attachment before the registry decodes it. An action with state type `()` accepts an absent attachment.
On success, `encode_updates` signs queued state and writes one frame.

### Render the updated list

A shared page function renders the initial list and the result of each action.

```rust
use foundation_ui_runtime::{action, html};
use foundation_ui_runtime::action::{ActionResult, Context, Interaction};
use foundation_ui_runtime::view::template_types::Html;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct TodoList { items: Vec<String> }

fn todo_row(item: &String) -> Html {
    html! { <Text>{ item.as_str() }</Text> }
}

#[action]
async fn add_todo(ctx: &Context, event: Interaction<TodoList>) -> ActionResult {
    let mut list = event.state;
    list.items.push(event.input_value);
    todo_page(ctx, &list)
}

fn todo_page(ctx: &Context, list: &TodoList) -> ActionResult {
    let out = ctx.receiver();
    html! { ctx, out, Some(list),
        <Stack>
            <TextInput primal::state on:submit={add_todo} />
            <List>{list.items.iter().map(todo_row)}</List>
        </Stack>
    };
    Ok(())
}
```

The page handler and action use the same render function. The root and the marked input each receive the same signed snapshot.
The next submit carries the bytes on the input. Ordinary element attributes remain application data.

If the action queues state and then returns an error, the host receives the error and drops the queued output.
For example, a title check can return `Err(ErrorTrace::new(ActionError::new(ErrorCode::ActionFailed, "Enter a todo title")))`.
The client retains its previous view and attachment. The integration test exercises this case while another caller completes a submit.

### Place a view at a specific target

A pure `html! { <Text>Milk</Text> }` returns an owned template. It writes no output.
The form with a context and receiver writes the complete tree and returns its root identifier in `Html.runtime_id`.
An action render replaces its trusted component root. An initial render creates the nodes.
Use `view::mount` to attach a pure tree below an existing parent.

```rust
use foundation_ui_runtime::{html_add, html_remove};
use foundation_ui_runtime::action::{ActionResult, Context};

fn update_rows(ctx: &Context, list: u32, old_row: u32) -> ActionResult {
    let out = ctx.receiver();
    html_remove! { old_row, ctx, out }?;
    html_add! { list, ctx, out, <Text>Buy milk</Text> }?;
    Ok(())
}
```

The targeted macros queue one placement each. The receiver writes all placements to the same response frame.
`html_swap!` replaces the target. `html_add_before!` and `html_add_after!` take a parent and an optional child.
A missing child argument selects the first or last position. A missing named target skips its placement.
Node identifiers work across platforms. Web renderers also accept selector strings.

Install `view::ViewIds::new(first_free_id)` on the base context before deriving request contexts for one mounted view.
All writers for that view must allocate distinct identifiers. Install a shared allocator with `ViewIds::with_allocator` when multiple hosts write into the view.
A single context without this entry starts its own sequence at one.
Install a `view::TokenTable` in the context when templates use named color or font tokens.

## Negotiate a connection and dispatch an action

Build one dispatcher at startup. Pass the caller’s context on each request.
The hello reply advertises callable server actions for the client’s surface. Keep the accepted connection for that stream.

This example sends a hello followed by a stateless action. The action returns the `MessagePack` value `true` to node 42.

```rust
use std::sync::Arc;
use foundation_context::Ctx;
use foundation_ui_runtime::action;
use foundation_ui_runtime::action::{ActionRegistry, ActionResult, Context, Interaction, Origin};
use foundation_ui_runtime::dispatch::{DispatcherBuilder, HelloOutcome};
use foundation_ui_wire::{
    finish_frame, read_frame, ActionRequestT, ClientHelloT, FrameKindT, FrameRequestT, FrameT,
    MountSurface, Platform, WireLimits, SCHEMA_VERSION,
};
use foundation_ui_wire::vocab::VOCABULARY_VERSION;

#[action(server)]
async fn confirm(ctx: &Context, event: Interaction<()>) -> ActionResult {
    ctx.receiver().data(event.node_id, vec![0xc3]);
    Ok(())
}

fn encode(request: FrameRequestT, stream: u64, sequence: u64) -> Vec<u8> {
    let mut frame = FrameT::default();
    frame.schema_version = SCHEMA_VERSION;
    frame.stream_id = stream;
    frame.sequence_id = sequence;
    frame.kind = FrameKindT::FrameRequest(Box::new(request));
    finish_frame(&frame).unwrap()
}

#[foundation_core::valtron::valtron]
async fn main() {
    let ctx = Ctx::try_host().unwrap();
    let mut registry = ActionRegistry::new();
    registry.register(confirm).unwrap();
    let dispatcher = DispatcherBuilder::new(Origin::Server, Arc::new(registry)).build();

    let mut hello = ClientHelloT::default();
    hello.platform = Platform::Web;
    hello.mount = MountSurface::Web;
    hello.vocabulary_version = VOCABULARY_VERSION;
    let mut request = FrameRequestT::default();
    request.hello = Some(Box::new(hello));
    let (connection, caps_frame) = match dispatcher.hello(&ctx, &encode(request, 0, 0)).unwrap() {
        HelloOutcome::Accepted { connection, caps_frame } => (connection, caps_frame),
        HelloOutcome::UpdateRequired { .. } => return,
    };
    let caps = read_frame(&caps_frame, &WireLimits::default()).unwrap();
    assert_eq!(caps.sequence_id(), 0);

    let mut action = ActionRequestT::default();
    action.action = Some("server:confirm".into());
    action.node_id = 42;
    let mut request = FrameRequestT::default();
    request.action = Some(Box::new(action));
    let input = encode(request, connection.stream_id(), 1);
    let reply = dispatcher.dispatch(&ctx, &connection, &input).await;
    let frame = read_frame(&reply, &WireLimits::default()).unwrap();
    let result = frame.kind_as_frame_message().unwrap();
    assert_eq!(result.ack(), 1);
    assert_eq!(result.messages().unwrap().get(0).data().unwrap().target_node(), 42);
}
```

For an authenticated action, pass the context that holds the verified user and bound signer.
If the action renders a component, the host stores `RenderTarget(component_root)` in that request context.
The action reads it through `ctx.state_root()`. The submitting control supplies its own signed attachment.
A persistent host can keep one context and use its own table of component roots for each call.

```rust
# use std::sync::Arc;
# use foundation_ui_runtime::{action::{ActionRegistry, Origin}, dispatch::DispatcherBuilder};
# fn build(registry: Arc<ActionRegistry>) -> foundation_ui_runtime::dispatch::Dispatcher {
let bindings = [("confirm", 42, 40), ("confirm", 82, 80)];
DispatcherBuilder::new(Origin::Server, registry)
    .render_target(move |_, action, trigger| {
        bindings.iter().find(|&&(name, node, _)| name == action && node == trigger)
            .map(|&(_, _, root)| root)
    })
    .build()
# }
```

Here each tuple contains an action name, a triggering node and the root that the host rendered for it.

For independent requests, use `Connection::stateless(caps, surface, stream_id)`. A negotiated connection also checks request order.
A local host can give its dispatcher and context to `LocalDispatcher` and install that transport on the action manager.
The [replica test](tests/dispatch_tests.rs) sends a signed counter between two independently configured hosts.
