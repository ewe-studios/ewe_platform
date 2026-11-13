---
feature: "action-runtime"
description: "The action model in foundation_ui_runtime: Ctx and Context, Interaction, Response, ComponentState, the Action trait, the user-built registry, action references, and the action manager that checks origin and scope."
status: "pending"
priority: "high"
model: "sonnet"
depends_on: ["01-wire-schema-and-codegen", "04-signed-state"]
estimated_effort: "large"
created: 2026-11-13
last_updated: 2026-11-13
author: "Main Agent"
crates: ["foundation_ui_runtime"]
decisions: ["D16", "D17", "D18", "D22", "D23", "D31", "D37", "D38", "A6", "A8", "R6", "R7"]
tasks:
  completed: 0
  uncompleted: 10
  total: 10
  completion_percentage: 0
---

# Feature 05: The action runtime

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** user interface (UI); WebAssembly (WASM); JavaScript (JS);
remote procedure call (RPC); Secure Hash Algorithm 256 (SHA-256); identifier (id); AeroSpace
and Defence Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

`foundation_ui_runtime::action` holds the one action model. An action is a plain async
function behind a small trait. The app builds a registry and tells it the origin of each
action. An action manager reads a reference such as `server:increment_counter`, checks the
origin and the declared platform scope, and routes the call. The same types run on the
server, in WASM and on a device.

## 2. Story

- **S1**: "An app registers an action under an origin."
- **S2**: "An event names an action, and the manager routes the call to the right place."
- **S3**: "An action reads the clock and the session from its context."
- **S4**: "An action returns a view update, a data update or an error."
- **S5**: "An event names an action that the registry does not have."
- **S6**: "An event names a `js:` action on a mobile surface."
- **S7**: "An event names a web-only action on a mobile surface."
- **S8**: "An app registers two actions with one name and one scope."
- **S9**: "A client tells the server which actions it has."

| # | Use case | Actor | Trigger | Result |
|---|---|---|---|---|
| U1 | Register | App start code | `registry.register(Origin::Server, action)` | The entry exists, or `RegistryError` |
| U2 | Look up | Dispatcher, manager | `registry.get(origin, name, surface)` | The action, or `LookupError` |
| U3 | Trigger | Device or WASM host | `manager.trigger(reference, event)` | Frame bytes from the route, or `ManagerError` |
| U4 | Run | Dispatcher | `ErasedAction::call_raw` | A `Response` |
| U5 | Hash | Client, server | `registry.hash_for(surface)` | 32 bytes that name the action set |

## 3. Bounded context

**Inside this feature:** `Ctx`, `Context`, `Surface`, `Origin`, `Scope`, `ActionRef`,
`Interaction`, `RawInteraction`, `Response`, `ComponentState`, `Action`, `ErasedAction`,
`ActionRegistry`, `ActionManager`, `ActionTransport`, the route rule and its shared test
vectors.

**Outside:**

| Thing | Owner |
|---|---|
| The `#[action]` macro that writes an `Action` impl | Feature 06 |
| Verify state, sign state, encode the response (the dispatch story) | Feature 07 |
| `Session`, `Expect`, `StateCodec` | Feature 04 |
| `OpT`, `ActionErrorT`, `ActionInputT`, `ErrorCode`, `Platform`, `MountSurface`, `finish_action_request` | Feature 01 |
| `foundation_context::Ctx` | `foundation_context` |
| The JS action manager and the JS registry | Feature 10 |
| The network transport | The host (D25) |

| Entity | Meaning | Owner (the one writer) |
|---|---|---|
| `ActionRegistry` | Entries by origin and name | The app, through `register` |
| `Context` | The per-call values an action can read | The dispatcher (feature 07) |
| Client sequence counter | The `sequence_id` of the next `ActionRequest` | `ActionManager` |

| Verb | Kind | Trigger | Effect |
|---|---|---|---|
| `register` | entity | App start | Adds one entry |
| `get` | entity | Dispatcher, manager | Reads |
| `trigger` | story | An event | Parse, look up, check, build the request, send, return frame bytes |

**Words.** "Origin" is where an action runs. "Scope" is where an action exists. "Reference"
is `origin:name`. "Surface" is the platform and the mount surface of the caller. "Declared"
means that the registry has an entry; a `server:` entry in a client registry is a
declaration that the client never runs.

## 4. Rules

### 4.1 Law worksheets

```
Entity: ActionRegistry
Conserved value:         an entry never changes after register succeeds
Bound:                   for one (origin, name): at most one All entry, or at most one Web and one Mobile entry
Inverse pairs:           none (no unregister in version 1)
Idempotent verbs:        get, hash_for
Ordering pairs:          register comes before get
Monotone values:         the entry count only grows
Freeze points:           none
Links to other entities: one component_type number belongs to one state type
```

```
Entity: ActionRef
Conserved value:         parse(format(r)) == r
Bound:                   origin is one of three; name is 1 to 128 bytes of [A-Za-z0-9_]
Inverse pairs:           parse / to_string
Idempotent verbs:        parse
Ordering pairs:          none
Monotone values:         none
Freeze points:           none
Links to other entities: none
```

```
Entity: Client sequence counter
Conserved value:         none
Bound:                   >= 1
Inverse pairs:           none
Idempotent verbs:        none
Ordering pairs:          none
Monotone values:         strictly increases with each request that is built
Freeze points:           none
Links to other entities: ActionRequest.sequence_id
```

### 4.2 Invariants

| # | Invariant |
|---|---|
| I1 | `get(origin, name, surface)` returns an action only if an entry for `(origin, name)` exists and its scope includes `surface`. |
| I2 | `get` returns `UnknownAction` when no entry has that origin and name, and `UnsupportedAction` when entries exist and no scope includes the surface. |
| I3 | `register` fails, and changes nothing, when it would break the bound of the registry worksheet or give one `component_type` to two state types. |
| I4 | `Scope::All` includes every surface. `Scope::Web` includes exactly the surfaces with `mount == Web`. `Scope::Mobile` includes exactly the surfaces with `mount == Native` and `platform` `Ios` or `Android`. |
| I5 | `trigger` never sends a request to a transport before the reference parsed, the entry was found and the origin is available. |
| I6 | The manager sends only `server:` references to the server transport and only `native:` references to the local transport. |
| I7 | A `js:` reference on a surface with `mount == Native` gives `UnavailableOrigin`. |
| I8 | `hash_for(surface)` is the same for two registries that hold the same set of references for that surface, in any order of registration. |
| I9 | `call_raw` never calls the action function when the state body does not decode. It returns `Response::Error` with code `BadRequest`. |
| I10 | Each request that one manager builds has a `sequence_id` greater than the one before. |
| I11 | The route decision of the Rust manager and of the JS manager (feature 10) is the same for every row of `fixtures/action_routes.json`. (This feature proves Rust.) |

### 4.3 Must have

- `Response` with exactly three cases: `UIUpdate`, `DataUpdate`, `Error` (A6).
- `Interaction<T>` with exactly the four fields of the source document.
- A registry that is a normal value. No global registry and no generated list (D23).

### 4.4 Must do

- Give a typed error for every reference that cannot run. Never a silent no-op (D37).
- Keep `Ctx` a thin wrapper: every clock and RNG read goes to `foundation_context::Ctx` (D17).

### 4.5 Must not

- Do not derive a numeric id from an action name (D31).
- Do not put an encoder or a wire format choice in an action (D16).
- Do not build on `#[service]` or a router (D18).
- Do not add `ctx.authorize`, `ActionCtx`, `ActionResult` or a `Result` around `Response`
  (the owner rejected them; see `learnings.md`).
- Do not guess an origin from a name with no prefix.

## 5. Acceptance scenarios

Surfaces: `WEB = Surface { platform: Web, mount: Web }`,
`IOS = Surface { platform: Ios, mount: Native }`,
`IOS_WEBVIEW = Surface { platform: Ios, mount: Web }`.

```gherkin
Feature: Action runtime

  Scenario: A registered action is found for its origin
    Given a registry with Increment (name "increment_counter", scope All) under Origin::Server
    When the caller calls get(Server, "increment_counter", IOS)
    Then the call returns the action

  Scenario: An unknown action is a typed error
    Given an empty registry
    When the caller calls get(Server, "increment_counter", WEB)
    Then the call fails with LookupError::UnknownAction

  Scenario: The same name under another origin is unknown
    Given a registry with Increment under Origin::Server only
    When the caller calls get(Native, "increment_counter", WEB)
    Then the call fails with LookupError::UnknownAction

  Scenario: A web-only action is refused on a native mount
    Given a registry with OpenPickerWeb (name "open_picker", scope Web) under Origin::Server
    When the caller calls get(Server, "open_picker", IOS)
    Then the call fails with LookupError::UnsupportedAction

  Scenario: A web-only action exists in a WebView on iOS
    Given the same registry
    When the caller calls get(Server, "open_picker", IOS_WEBVIEW)
    Then the call returns the action

  Scenario: One name has a web form and a mobile form
    Given a registry with OpenPickerWeb (scope Web) and OpenPickerMobile (scope Mobile),
      both named "open_picker", under Origin::Server
    When the caller calls get(Server, "open_picker", WEB) and get(Server, "open_picker", IOS)
    Then the first call returns OpenPickerWeb and the second returns OpenPickerMobile

  Scenario: A second action with the same name and scope is rejected
    Given a registry with Increment under Origin::Server
    When the app registers another action named "increment_counter" with scope All under Origin::Server
    Then register fails with RegistryError::DuplicateAction
    And get still returns the first action

  Scenario: A scoped action beside an unscoped action is rejected
    Given a registry with Increment (scope All) under Origin::Server
    When the app registers an action named "increment_counter" with scope Web under Origin::Server
    Then register fails with RegistryError::DuplicateAction

  Scenario: A reference with no origin does not parse
    When the caller parses "increment_counter"
    Then the parse fails with RefError::MissingOrigin

  Scenario: A js reference on a native mount is unavailable
    Given a manager for surface IOS with a server transport and a local transport
    And a registry that declares "open_picker" under Origin::Js
    When the host triggers "js:open_picker"
    Then trigger fails with ManagerError::UnavailableOrigin { origin: Js }
    And no transport received a request

  Scenario: A server reference with no transport is unavailable
    Given a manager for surface IOS with no server transport
    And a registry that declares "increment_counter" under Origin::Server
    When the host triggers "server:increment_counter"
    Then trigger fails with ManagerError::UnavailableOrigin { origin: Server }

  Scenario: A native reference goes to the local transport
    Given a manager for surface IOS with a recording server transport and a recording local transport
    And a registry with Toggle (name "toggle") under Origin::Native
    When the host triggers "native:toggle" for node 42
    Then the local transport received one ActionRequest with action "native:toggle", node_id 42 and sequence_id 1
    And the server transport received nothing

  Scenario: A state that does not decode never reaches the action
    Given Increment, whose state type decodes 4 bytes
    When the dispatcher calls call_raw with a state body of 3 bytes
    Then the response is Response::Error with code BadRequest
    And the action function did not run

  Scenario: The registry hash does not depend on the order
    Given registry A with actions registered in the order x, y
    And registry B with the same actions registered in the order y, x
    Then A.hash_for(WEB) equals B.hash_for(WEB)
```

## 6. Technical breakdown

### 6.1 Measured facts

| Fact | Source |
|---|---|
| `foundation_context::Ctx` is a clonable handle: `clock()`, `rng()`, `get::<T>()`, `config()`, `fork()`, `Ctx::host()`, `Ctx::fixed(clock)`, `Ctx::with_seed(clock, seed)` | `backends/foundation_context/src/lib.rs:211-246`, `:366`, `:437-484` |
| The source document gives `Interaction<T> { node_id: u32, event_name: String, input_value: String, state: T }` and `Response { UIUpdate(Vec<DomOp>), DataUpdate(Vec<u8>) }` | `research/10-action-model.md` section 1 |
| No action macro or registry exists today | `research/10-action-model.md` section 2; `grep -n "pub fn action" backends/foundation_macros/src/lib.rs` gives nothing |

### 6.2 Files

```
backends/foundation_ui_runtime/src/action/
├── mod.rs
├── ctx.rs          Ctx, Context, Surface
├── reference.rs    Origin, Scope, ActionRef, RefError
├── interaction.rs  Interaction, RawInteraction, Response, ComponentState, StateDecodeError
├── action.rs       Action, ErasedAction, ActionFuture
├── registry.rs     ActionRegistry, RegistryError, LookupError
├── manager.rs      ActionManager, ActionTransport, TransportError, ManagerError, route
backends/foundation_ui_runtime/fixtures/action_routes.json
backends/foundation_ui_runtime/tests/{action_ref_tests.rs,action_registry_tests.rs,action_call_tests.rs,action_manager_tests.rs,action_routes_tests.rs,action_ctx_tests.rs}
```

Add `serde_json` (workspace) as a dev-dependency for the route vectors.

### 6.3 Public API

```rust
// ctx.rs ---------------------------------------------------------------
/// Our wrapper over `foundation_context::Ctx` (D17).
#[derive(Clone)]
pub struct Ctx(foundation_context::Ctx);
impl Ctx {
    #[must_use] pub fn new(inner: foundation_context::Ctx) -> Self;
    #[must_use] pub fn clock(&self) -> Arc<dyn foundation_context::Clock>;
    #[must_use] pub fn rng(&self) -> /* the same type that foundation_context::Ctx::rng returns */;
    #[must_use] pub fn get<T: Any + Send + Sync>(&self) -> Option<Arc<T>>;
    #[must_use] pub fn inner(&self) -> &foundation_context::Ctx;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Surface { pub platform: Platform, pub mount: MountSurface }

/// What an action can read for one call. It can gain fields; `Ctx` does not change.
pub struct Context { /* ctx: Ctx, session: Session, surface: Surface, input: ActionInputT, state_root: Option<u32> */ }
impl Context {
    #[must_use] pub fn new(ctx: Ctx, session: Session, surface: Surface, input: ActionInputT) -> Self;
    /// Sets the root node of the component whose state the request carried.
    #[must_use] pub fn with_state_root(self, root_node: u32) -> Self;
    /// The root node from the verified state. `None` when the request had no state.
    #[must_use] pub fn state_root(&self) -> Option<u32>;
    #[must_use] pub fn ctx(&self) -> &Ctx;
    #[must_use] pub fn session(&self) -> &Session;
    #[must_use] pub fn surface(&self) -> Surface;
    /// The full input of the event (checked, key code, modifiers, parent node).
    #[must_use] pub fn input(&self) -> &ActionInputT;
}

// reference.rs -----------------------------------------------------------
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Origin { Server, Native, Js }
impl Origin { #[must_use] pub fn prefix(self) -> &'static str; }  // "server", "native", "js"

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Scope { All, Web, Mobile }
impl Scope { #[must_use] pub fn includes(self, surface: Surface) -> bool; }   // I4

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActionRef { pub origin: Origin, pub name: String }
impl ActionRef {
    pub fn parse(text: &str) -> Result<Self, RefError>;
    #[must_use] pub fn new(origin: Origin, name: &str) -> Self;   // panics never; name is not checked here
}
impl core::fmt::Display for ActionRef {}   // "origin:name"

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RefError {
    MissingOrigin,                 // no ':' in the text
    UnknownOrigin { found: String },
    BadName { found: String },     // empty, longer than 128 bytes, or a byte outside [A-Za-z0-9_]
}

// interaction.rs ---------------------------------------------------------
/// The event that an action receives. The fields are those of the source document.
#[derive(Clone, Debug, PartialEq)]
pub struct Interaction<T> {
    pub node_id: u32,
    pub event_name: String,
    pub input_value: String,   // raw text from the user; untrusted
    pub state: T,
}

/// The event before the state is decoded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawInteraction {
    pub node_id: u32,
    pub event_name: String,
    pub input_value: String,
    /// The plain state body, after the codec opened it. `None` when the request had no state.
    pub state_body: Option<Vec<u8>>,
}

/// The logical result of an action. The delivery handler encodes it (D16).
#[derive(Debug, PartialEq)]
pub enum Response {
    /// A view description: ops in the one vocabulary. May hold `AttachState` ops.
    UIUpdate(Vec<OpT>),
    /// Data, not a view. The handler picks the body type.
    DataUpdate(Vec<u8>),
    /// The action failed. The client gets an `Error` frame.
    Error(ActionErrorT),
}
impl Response {
    /// `Error` with `ErrorCode::ActionFailed`.
    #[must_use] pub fn failed(message: impl Into<String>) -> Self;
}

/// The state type of a component. The body is opaque to the client.
pub trait ComponentState: Sized + Send + 'static {
    /// A number that the author chooses. One number belongs to one type in a registry.
    const COMPONENT_TYPE: u32;
    /// Increase it when the shape of the body changes.
    const FORMAT_VERSION: u16;
    /// True makes the dispatcher seal the state as private (encrypted).
    const PRIVATE: bool = false;
    fn decode(body: &[u8]) -> Result<Self, StateDecodeError>;
    fn encode(&self) -> Vec<u8>;
}
/// The state of an action that has no state. COMPONENT_TYPE 0, FORMAT_VERSION 0.
/// `decode` accepts only an empty body; `encode` returns an empty Vec.
impl ComponentState for () {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateDecodeError { pub detail: String }

// action.rs --------------------------------------------------------------
#[cfg(not(target_arch = "wasm32"))]
pub type ActionFuture<'a> = Pin<Box<dyn Future<Output = Response> + Send + 'a>>;
#[cfg(target_arch = "wasm32")]
pub type ActionFuture<'a> = Pin<Box<dyn Future<Output = Response> + 'a>>;

pub trait Action: Send + Sync + 'static {
    /// The action tag. It is the function name (D31).
    const NAME: &'static str;
    const SCOPE: Scope;
    type State: ComponentState;
    fn call<'a>(&'a self, ctx: &'a Context, event: Interaction<Self::State>) -> ActionFuture<'a>;
}

/// The object-safe form that a registry stores. Every `Action` has it (blanket impl).
pub trait ErasedAction: Send + Sync + 'static {
    fn name(&self) -> &'static str;
    fn scope(&self) -> Scope;
    /// `None` when the state type is `()`.
    fn expect(&self) -> Option<Expect>;
    fn private_state(&self) -> bool;
    fn call_raw<'a>(&'a self, ctx: &'a Context, raw: RawInteraction) -> ActionFuture<'a>;
}
impl<A: Action> ErasedAction for A { /* section 6.4 */ }

// registry.rs ------------------------------------------------------------
#[derive(Default)]
pub struct ActionRegistry { /* BTreeMap<(Origin, String), Vec<Arc<dyn ErasedAction>>>,
                               BTreeMap<u32, core::any::TypeId> */ }
impl ActionRegistry {
    #[must_use] pub fn new() -> Self;
    /// Tells the registry where `action` lives (A8).
    pub fn register<A: Action>(&mut self, origin: Origin, action: A) -> Result<(), RegistryError>;
    /// Declares a page JavaScript function. It has no Rust body. Scope is `Web`.
    pub fn declare_js(&mut self, name: &str) -> Result<(), RegistryError>;
    pub fn get(&self, origin: Origin, name: &str, surface: Surface)
        -> Result<Arc<dyn ErasedAction>, LookupError>;
    /// Every reference that exists on `surface`, sorted.
    #[must_use] pub fn refs_for(&self, surface: Surface) -> BTreeSet<String>;
    /// SHA-256 over each reference of `refs_for`, in order, each followed by one b'\n'.
    #[must_use] pub fn hash_for(&self, surface: Surface) -> [u8; 32];
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryError {
    DuplicateAction { origin: Origin, name: String, scope: Scope },
    DuplicateComponentType { component_type: u32 },
    BadName { found: String },
    /// `Origin::Js` with a scope other than `Web`, or `register` called with `Origin::Js`
    /// (a JS action has no Rust body; use `declare_js`).
    JsScope { name: String },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LookupError { UnknownAction, UnsupportedAction }

// manager.rs -------------------------------------------------------------
#[cfg(not(target_arch = "wasm32"))]
pub type TransportFuture<'a> = Pin<Box<dyn Future<Output = Result<Vec<u8>, TransportError>> + Send + 'a>>;
#[cfg(target_arch = "wasm32")]
pub type TransportFuture<'a> = Pin<Box<dyn Future<Output = Result<Vec<u8>, TransportError>> + 'a>>;

/// Carries one `ActionRequest` buffer and returns one `Frame` buffer. The host implements
/// it for the network (D25). Feature 07 implements it for the local dispatcher.
pub trait ActionTransport: Send + Sync {
    fn send<'a>(&'a self, request: Vec<u8>) -> TransportFuture<'a>;
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportError { pub detail: String }

/// The event that a host hands to the manager.
#[derive(Clone, Debug, PartialEq)]
pub struct TriggerEvent {
    pub node_id: u32,
    pub input: ActionInputT,
    /// The sealed state bytes from the component root, exactly as stored. Empty for none.
    pub state: Vec<u8>,
}

pub struct ActionManager { /* registry: Arc<ActionRegistry>, surface, stream_id: u64,
    server: Option<Arc<dyn ActionTransport>>, local: Option<Arc<dyn ActionTransport>>,
    next_sequence: AtomicU64 (starts at 1) */ }
impl ActionManager {
    #[must_use] pub fn new(registry: Arc<ActionRegistry>, surface: Surface, stream_id: u64) -> Self;
    #[must_use] pub fn with_server(self, transport: Arc<dyn ActionTransport>) -> Self;
    #[must_use] pub fn with_local(self, transport: Arc<dyn ActionTransport>) -> Self;
    /// Returns the bytes of the answer `Frame`. The host applies them.
    pub async fn trigger(&self, reference: &str, event: TriggerEvent)
        -> Result<Vec<u8>, ManagerError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManagerError {
    BadReference(RefError),
    UnknownAction { reference: String },
    UnsupportedAction { reference: String },
    UnavailableOrigin { origin: Origin },
    Transport(TransportError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route { Server, Local, Js }
/// The pure route rule (section 6.5). `lookup` is the result of `registry.get`.
pub fn route(reference: &ActionRef, surface: Surface, lookup: Result<(), LookupError>,
    has_server: bool, has_local: bool, has_js: bool) -> Result<Route, ManagerError>;
```

### 6.4 `call_raw` (the blanket impl)

1. If `A::State` is `()`: the state is `()`; ignore `raw.state_body`.
2. Else: `raw.state_body` of `None` gives
   `Response::Error(ActionErrorT { code: BadRequest, message: "state required", row_index: 0 })`.
   `A::State::decode(body)` of `Err(e)` gives the same with `message = e.detail`.
3. Build `Interaction { node_id, event_name, input_value, state }` and return
   `self.call(ctx, interaction)`.

Steps 1 and 2 run before the future of the action is made, so the action function does not
run on a bad state (I9).

### 6.5 The route rule

`route` applies these rows in order. The first row that matches decides.

| # | Condition | Result |
|---|---|---|
| 1 | `lookup == Err(UnknownAction)` | `Err(UnknownAction)` |
| 2 | `lookup == Err(UnsupportedAction)` | `Err(UnsupportedAction)` |
| 3 | origin `Js` and `surface.mount == Native` | `Err(UnavailableOrigin { Js })` |
| 4 | origin `Js` and `!has_js` | `Err(UnavailableOrigin { Js })` |
| 5 | origin `Js` | `Ok(Route::Js)` |
| 6 | origin `Server` and `!has_server` | `Err(UnavailableOrigin { Server })` |
| 7 | origin `Server` | `Ok(Route::Server)` |
| 8 | origin `Native` and `!has_local` | `Err(UnavailableOrigin { Native })` |
| 9 | origin `Native` | `Ok(Route::Local)` |

The scope check is inside `registry.get` (rows 1 and 2). The Rust `ActionManager` always
passes `has_js = false`: Rust has no page JavaScript. The JS manager of feature 10 passes
`true` on the web.

`fixtures/action_routes.json` is an array of objects
`{ "reference", "platform", "mount", "declared": [{"origin","name","scope"}], "has_server",
"has_local", "has_js", "expect" }`, where `expect` is `"server"`, `"local"`, `"js"`,
`"bad_reference"`, `"unknown_action"`, `"unsupported_action"` or `"unavailable_origin"`.
Write one row or more for each of the 9 rows of the table, for each `RefError` case, and
for each cell of the scope and origin table in `research/10-action-model.md` section 3.8.
`action_routes_tests.rs` builds a registry from `declared` (a test action type with a
runtime name is not possible with `const NAME`; so the test calls `route` with the lookup
that it computes from `declared` with `Scope::includes`, and a second test checks that
`registry.get` gives the same lookup for a fixed set of real test actions).

### 6.6 `trigger`, in order

```mermaid
sequenceDiagram
    participant H as Host (event bridge)
    participant M as ActionManager
    participant R as ActionRegistry
    participant T as ActionTransport (server or local)
    H->>M: trigger("native:toggle", event)
    M->>M: ActionRef::parse
    M->>R: get(origin, name, surface)
    R-->>M: entry or LookupError
    M->>M: route(...)
    M->>M: build ActionRequest (next sequence id)
    M->>T: send(request bytes)
    T-->>M: frame bytes
    M-->>H: frame bytes
```

_The diagram shows one trigger. The manager sends nothing before the checks pass._

1. `ActionRef::parse(reference)`; an error gives `BadReference`.
2. `registry.get(origin, name, surface)`, mapped to `Ok(())` or the `LookupError`.
3. `route(...)`; an error returns.
4. `Route::Js` cannot happen in Rust (`has_js` is false). Treat it as
   `UnavailableOrigin { Js }`.
5. Take the next sequence id (`fetch_add(1)`).
6. Build `ActionRequestT { schema_version: SCHEMA_VERSION, stream_id, sequence_id,
   action: reference text, node_id, input, state }` and call `finish_action_request`.
7. `send` on the transport of the route. An error gives `Transport`.
8. Return the bytes. The manager does not read them.

**Fault table: `trigger`**

| Step that fails | What the system does | What the caller sees | What is left behind |
|---|---|---|---|
| 1 to 4 | Returns at once | `BadReference`, `UnknownAction`, `UnsupportedAction` or `UnavailableOrigin` | nothing; the sequence counter did not move |
| 7 (transport error) | Returns | `Transport` | The sequence counter moved by one. The server sees a gap in client sequence ids, which it accepts (feature 07: the client counter is for ordering and duplicate detection, not for gaps) |

### 6.7 `register`

1. `Origin::Js` gives `JsScope`.
2. Check the name as `ActionRef` does (`BadName`).
3. Among the entries of `(origin, name)`: an entry with the same scope gives
   `DuplicateAction`; an existing `All` entry or a new `All` scope beside any entry gives
   `DuplicateAction`.
4. If `A::State` is not `()`: if `COMPONENT_TYPE` is in the type map with another `TypeId`,
   `DuplicateComponentType`. `COMPONENT_TYPE == 0` for a type other than `()` also gives
   `DuplicateComponentType { component_type: 0 }`.
5. Insert. Steps 1 to 4 change nothing.

`declare_js(name)`: name check; duplicate check; store a private `JsDeclared` entry whose
`scope()` is `Web`, `expect()` is `None`, and whose `call_raw` returns
`Response::Error` with code `UnavailableOrigin` (a JS action never runs in Rust).

## 7. Tests

All in `backends/foundation_ui_runtime/tests/`. Test actions are hand-written `impl Action`
types in `tests/support.rs` (the macro is feature 06): `Increment` (state `Counter`, 4-byte
little-endian `u32`, `COMPONENT_TYPE` 7), `Toggle` (state `()`), `OpenPickerWeb`,
`OpenPickerMobile`, and `Probe`, which sets an `AtomicBool` when its function runs.

| Law or scenario | Test | Oracle |
|---|---|---|
| I1, I2; Scenarios: found, unknown, other origin, web-only refused, WebView, two forms | `action_registry_tests.rs::…` one test per scenario, named after it | example |
| I3; Scenarios: duplicate, scoped beside unscoped | `action_registry_tests.rs::duplicate_is_rejected_and_first_stays`, `::scoped_beside_unscoped_is_rejected`, `::one_component_type_for_two_types_is_rejected` | expected outcome |
| I4 | `action_ref_tests.rs::scope_includes_table` (all 3 scopes × 5 platforms × 2 mounts against the rule) | property |
| `ActionRef` law; Scenario: no origin | `action_ref_tests.rs::parse_format_round_trip`, `::missing_origin`, `::unknown_origin`, `::bad_name_cases` | property, expected outcome |
| I5, I6; Scenarios: native goes local, server with no transport | `action_manager_tests.rs::native_reference_goes_to_local_transport`, `::server_reference_with_no_transport_is_unavailable`, `::no_request_before_checks_pass` | example, fault |
| I7; Scenario: js on native | `action_manager_tests.rs::js_reference_on_native_is_unavailable` | expected outcome |
| I8; Scenario: hash order | `action_registry_tests.rs::hash_does_not_depend_on_order`, `::hash_changes_with_the_set`, `::hash_differs_by_surface` | metamorphic |
| I9; Scenario: bad state | `action_call_tests.rs::bad_state_never_reaches_the_action`, `::missing_state_is_bad_request`, `::unit_state_ignores_body` | expected outcome |
| I10 | `action_manager_tests.rs::sequence_ids_increase` (5 triggers; ids 1 to 5) | property |
| I11 | `action_routes_tests.rs::route_matches_every_vector`, `::registry_lookup_matches_scope_rule` | example |
| `Ctx` passes the clock and RNG through | `action_ctx_tests.rs::ctx_reads_inner_clock_and_rng` | example |
| Fault: transport error | `action_manager_tests.rs::transport_error_is_returned` | fault |

## 8. Tasks

- [ ] 1. `ctx.rs` and its test.
- [ ] 2. `reference.rs` and its tests.
- [ ] 3. `interaction.rs`.
- [ ] 4. `action.rs` with the blanket impl; `action_call_tests.rs`.
- [ ] 5. `registry.rs` and its tests.
- [ ] 6. `route` and `fixtures/action_routes.json`; `action_routes_tests.rs`.
- [ ] 7. `ActionManager::trigger` and its tests.
- [ ] 8. Re-export the public types from `lib.rs`.
- [ ] 9. Bump `foundation_ui_runtime` to `0.0.2`.
- [ ] 10. Update `progress.md` and `learnings.md`.

## 9. Verify

```bash
cargo fmt --all -- --check
cargo check  -p foundation_ui_runtime --profile uat
cargo clippy -p foundation_ui_runtime --profile uat --no-deps -- -D warnings
cargo test   -p foundation_ui_runtime --profile uat
cargo check  -p foundation_ui_runtime --profile uat --target wasm32-unknown-unknown
python3 -c "import json;json.load(open('backends/foundation_ui_runtime/fixtures/action_routes.json'))"
```

## 10. Done when

- [ ] All tasks are complete and every command of section 9 passes with no warning.
- [ ] Each invariant and each scenario has a passing test (section 7).
- [ ] `Interaction` has exactly four public fields and `Response` exactly three cases.
- [ ] No name from the "Must not" list is in the crate.
- [ ] No `TODO`, `FIXME`, `todo!()` or `unimplemented!()` is in the module.
- [ ] The check list of the `story-driven-specification` skill passes.

---

_Created: 2026-11-13_
