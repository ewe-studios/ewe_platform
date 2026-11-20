---
feature: "dispatcher-and-handshake"
description: "The shared stateless dispatcher: handshake over schema and vocabulary versions, the ServerCaps action list, verify state, run the action through the receiver, seal the new state, encode the messages and answer with one frame."
status: "pending"
priority: "high"
model: "sonnet"
depends_on: ["03-frame-writer", "04-signed-state", "05-action-runtime"]
estimated_effort: "large"
created: 2026-11-13
last_updated: 2026-11-14
author: "Main Agent"
crates: ["foundation_ui_runtime"]
decisions: ["D13", "D14", "D15", "D16", "D25", "D29", "D32", "D37", "D38", "R1", "R6", "D41", "D42", "D43", "D44", "D45", "D47", "D50", "D52", "D54", "D59"]
tasks:
  completed: 0
  uncompleted: 11
  total: 11
  completion_percentage: 0
---

# Feature 07: The dispatcher and the handshake

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** user interface (UI); hypertext transfer protocol (HTTP);
WebAssembly (WASM); random number generator (RNG); identifier (id); AeroSpace and Defence
Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## Context for state signing

The revised [signer contract](../04-signed-state/feature.md) defines the bytes and the host binding.
The [protected route](../04-signed-state/feature.md#at-the-protected-route) passes its request context to the dispatcher.
The [action invocation](../05-action-runtime/feature.md#stage-3-pass-the-request-context-to-the-action) specifies the handoff to the registry entry. Use that context for verification, the action call and signing the collected state.
A session identifier is one optional host binding. The dispatcher obtains the signer and binding from the context.


## 1. Goal

`foundation_ui_runtime::dispatch` is the one implementation of the action lifecycle. It takes
the bytes of one request frame and returns the bytes of one message frame. It holds no
per-user memory, so any server replica can run it. The same code runs in WASM and on a device
with its host-installed state signer. A handshake turns the `hello` field of a request frame into the
capabilities of one connection. The hello checks the schema and the vocabulary versions only
(D54). The answer lists the `server:` actions that the server has for the surface (D54).

## 2. Story

- **S1**: "A client says hello, and the server answers with its capabilities and its action
  list."
- **S2**: "A client sends an action request, and a replica that never saw this user answers
  with a frame."
- **S3**: "An action writes data, and the delivery handler picks the encoding."
- **S4**: "A client says hello with an old schema version."
- **S5**: "A client says hello with a vocabulary version that the server does not have."
- **S6**: "A client sends a request with a forged state."
- **S7**: "A client sends a `host:` reference to a server."
- **S8**: "An action fails."
- **S9**: "A device runs the dispatcher locally for a `host:` action."

| # | Use case | Actor | Trigger | Result |
|---|---|---|---|---|
| U1 | Handshake | Client | A request frame whose `hello` is set | `HelloOutcome::Accepted` with a `Connection` and a caps frame, or `UpdateRequired` |
| U2 | Dispatch | Client | A request frame whose `action` is set | One `Frame`: the messages, or one `Control` message |
| U3 | Deliver | Dispatcher | A receiver and its items | The delivery handler returns frame bytes |
| U4 | Local dispatch | `ActionManager` | `ActionTransport::send` | The same as U2, with no network |

## 3. Bounded context

**Inside this feature:** `Dispatcher`, `DispatcherBuilder`, `Connection`, `HelloOutcome`,
`HelloError`, `DeliveryHandler`, `DefaultDelivery`, `LocalDispatcher`, the mapping from internal errors to `Control` messages.

**Outside:**

| Thing | Owner |
|---|---|
| `read_frame`, `finish_frame`, `expect_kind`, `FrameKindName`, `WireLimits`, `SCHEMA_VERSION` | Feature 01 |
| `VOCABULARY_VERSION`, `vocab_version` | Feature 02 |
| `write_messages`, `write_error`, `write_caps`, `write_messagepack`, `ClientCaps`, `StreamSequencer`, `WriterConfig` | Feature 03 |
| `StateSigner`, `sign_state`, `verify_state`, request context binding | Feature 04 |
| `ActionRegistry`, `ErasedAction`, `Context`, `ActionReceiver`, `ReceiverItem`, `ActionRef`, `ActionTransport`, `invoke_action`, `prepare_messages` | Feature 05 |
| The network: how bytes arrive and leave; how the host finds the session and the address | The host (D25) |
| The domain data, single use, authorization of the user for a row | The action (D13, D14) |
| The live mode (`App`, `Broadcaster`) | `foundation_wasm_ui::server` (feature 11). The dispatcher does not replace it (D15) |

| Entity | Meaning | Owner (the one writer) |
|---|---|---|
| `Connection` | The capabilities and the frame counter of one client connection | `Dispatcher::hello` creates it; `dispatch` advances its counters |
| Answer frame | The bytes that `dispatch` returns | `dispatch` |
| `ServerCaps.actions` | The `server:` actions for one surface, sorted by name | `Dispatcher::hello` builds it from the registry |

| Verb | Kind | Trigger | Effect |
|---|---|---|---|
| `hello` | story | A request frame whose `hello` is set | Checks the two versions; makes a `Connection`; lists the actions |
| `dispatch` | story | A request frame whose `action` is set | The lifecycle of section 6.4 |
| `messages` | entity | `dispatch` | Encode the answer messages |

**Words.** "Answer" is the frame with kind `FrameMessage` whose `ack` equals the request
frame's `sequence_id`. "Accept origin" is the one origin that a dispatcher runs: `Server` on
a server, `Host` in WASM and on a device. "Caps frame" is a frame with one `Control{caps}`
message; it has sequence 0 and ack 0.

## 4. Rules

### 4.1 Law worksheets

```
Entity: Answer frame
Conserved value:         exactly one frame for each dispatch call
Bound:                   its length <= WireLimits::max_frame_bytes
Inverse pairs:           none
Idempotent verbs:        none (the framework does not make an action idempotent; the domain does, D14)
Ordering pairs:          checks before the action; the action before seal; seal before encode
Monotone values:         frame sequence_id of one Connection strictly increases
Freeze points:           none
Links to other entities: ack == request.sequence_id (0 when the frame could not be read)
```

```
Entity: Connection
Conserved value:         caps, surface and stream_id never change after hello
Bound:                   none
Inverse pairs:           none
Idempotent verbs:        none
Ordering pairs:          hello comes before dispatch
Monotone values:         the frame counter; the highest request sequence_id seen
Freeze points:           caps freeze at hello
Links to other entities: caps.actions is the sorted server action list for the surface (D54)
```

### 4.2 Invariants

| # | Invariant |
|---|---|
| I1 | `dispatch` returns exactly one valid `Frame` for every input, also for bytes that are not a request. It never panics. |
| I2 | The action function runs only after: the request frame was read, the schema version matched, the origin was the accept origin, the registry found the action for the surface, and the request signer verified any required state before typed decoding. |
| I3 | A dispatcher never runs a reference whose origin is not its accept origin. |
| I4 | Every `AttachState` in the answer carries the authenticated bytes returned by the signer in the supplied request context. |
| I5 | A failed verification stops dispatch before the action runs. Host request throttling stays outside dispatch. |
| I6 | The answer never holds a field that the mount surface of the connection must not receive (the gate of feature 03 runs in the delivery handler). |
| I7 | Two dispatchers with the same registry, key ring and clock give the same body for the same request and connection. (Any replica can answer.) |
| I8 | Nothing that a dispatch call reads from the request stays reachable after the call returns, except the two counters of the `Connection`. |
| I9 | `hello` accepts a client only if the schema versions are equal and `vocab_version` gives a row for the client vocabulary version. |
| I10 | For a `Connection` from `hello`, a request whose `sequence_id` is not greater than the highest one seen gives `BadRequest` and does not run the action. |
| I11 | `ServerCaps.actions` lists every `server:` action whose scope includes the surface, and every action with no origin limit, sorted by name, each with `action_hash(origin, name, scope)` (D54). |
| I12 | On `Err` from the action, the answer holds one `Control{error}` message and no operation or data from the receiver (D52). |
| I13 | `Control.update_required` and `Control.state_expired` are their own tables. The dispatcher does not send them as an `ActionError` (D45). |

### 4.3 Must have

- `DeliveryHandler` as a trait, with `DefaultDelivery` (D16).
- `LocalDispatcher`, which implements `ActionTransport` (D29, D38).
- Use the receiver finalizer from feature 05 to sign state and preserve message order.

### 4.4 Must do

- Answer every failure with one `Control` message that has a `Control.error`,
  `Control.state_expired` or `Control.update_required` field.
- Use the host-supplied `Ctx` handle for the call. Each call owns its own receiver.
- The registered adapter selects the state type and decodes the verified body.
- Drop the receiver on `Err`, so a failed action leaves no half update (D52).

### 4.5 Must not

- Do not keep a session table, a state cache or a per-user map (D15).
- Do not choose a transport or frame the bytes for one (D25).
- Do not send an action registry hash, a known-registry table or a client action list (D54).
- Do not tell the client which state check failed. The wire has `ActionError` code
  `StateRejected` and the `StateExpired` control table only.
- Do not add a nonce or used-id store (D14).
- Do not remove or change `App`, `Broadcaster` or `BroadcastSink` (D15, D40).

## 5. Acceptance scenarios

Fixtures use a registered server counter, `HmacStateSigner`, and a request context bound to user A.
The clock starts at 1 000 000 seconds. The counter template updates node 42 and queues its next state.

```gherkin
Feature: Dispatcher and handshake

  Scenario: A hello with the current versions is accepted
    Given a request frame with schema_version 1 whose hello holds vocabulary_version 2 for a web mount
    When the host calls hello
    Then the outcome is Accepted
    And the caps frame has one Control{caps} message with status Ok and a stream_id that is not 0
    And caps.actions == [{ hash: action_hash(Server, "increment_counter", All),
      name: "increment_counter" }]

  Scenario: A hello with an unknown vocabulary version needs an update
    Given a request frame with the current schema version whose hello holds vocabulary_version 3
    When the host calls hello
    Then the outcome is UpdateRequired
    And the caps frame has status UpdateRequired
    And caps.actions is empty

  Scenario: A hello with another schema version needs an update
    Given a request frame with schema_version 2 whose hello holds the current vocabulary version
    When the host calls hello
    Then the outcome is UpdateRequired

  Scenario: An action runs on a replica that never saw the user
    Given dispatcher A sealed Counter { current: 1 } for "sess-A" on button 42
    And dispatcher B was built separately with the same registry, key ring and clock
    And a Connection from B.hello
    When the host calls B.dispatch with a request frame with sequence 5 whose action is
      "server:increment_counter" for node 42 with that state
    Then the answer has ack 5
    And messages holds one Message whose operation.ui is SetText(42, "2")
    And messages holds one Message whose operation.ui is AttachState for node 42
    And verifying that attachment with B's signer and the same binding gives Counter { current: 2 }

  Scenario: Two nodes receive one signed render
    Given root 40 and marked input 42 hold the same signed counter
    When input 42 sends its own attachment
    Then the action receives trigger 42 and the verified counter
    And the render targets the trusted component root

  Scenario: A forged state is rejected
    Given a signed value with a changed body
    When the host calls dispatch
    Then the answer reports StateRejected and the action never runs

  Scenario: An expired state asks for a reload
    Given a counter signed at 1000000 s and the clock at 1086400 s
    When the host calls dispatch
    Then the answer has one Control{state_expired} message

  Scenario: A server refuses a host reference
    Given a dispatcher with accept origin Server
    When the host calls dispatch with action "host:increment_counter"
    Then the answer has one Control{error} message with code UnavailableOrigin

  Scenario: An unknown action is a typed error
    When the host calls dispatch with action "server:nope"
    Then the answer has one Control{error} message with code UnknownAction

  Scenario: Bytes that are not a request get an error frame
    When the host calls dispatch with 16 bytes of 0xFF
    Then the answer has ack 0 and one Control{error} message with code BadRequest

  Scenario: A repeated sequence id is refused
    Given a Connection that answered request sequence 5
    When the host calls dispatch with another request with sequence 5
    Then the answer has one Control{error} message with code BadRequest
    And the action did not run

  Scenario: An action error becomes one error control message
    Given an action that returns Err with ActionError code ActionFailed and message "no stock"
    And its template queued a text change before the error
    When the host calls dispatch
    Then the answer has ack equal to the request sequence
    And the answer has one Control{error} message with code ActionFailed and message "no stock"
    And the answer holds no operation from the receiver

  Scenario: Data goes in a data message by default
    Given an action that calls receiver.data(42, [0x91, 0x01])
    When the host calls dispatch with node_id 42
    Then the answer holds one Message whose data.msgpack is target_node 42 with bytes [0x91, 0x01]

  Scenario: A native connection gets no web field in the answer
    Given a Connection for a native mount
    And the renderer fixture supplies an Operation with ui SetText(42, "x") and web HtmlMorph(node 40, "<b>x</b>")
    When the host calls dispatch
    Then the answer holds one Message whose operation.ui is SetText(42, "x")
    And no Message holds a web field

  Scenario: A local dispatcher answers through the transport trait
    Given a LocalDispatcher with accept origin Host, a host-installed signer and Toggle under Origin::Host
    When an ActionManager triggers "host:toggle"
    Then trigger returns a frame with one Message that holds the action result
```

## 6. Technical breakdown

### 6.1 Files

```
backends/foundation_ui_runtime/src/dispatch/
├── mod.rs
├── dispatcher.rs   Dispatcher, DispatcherBuilder, dispatch
├── hello.rs        Connection, HelloOutcome, Dispatcher::hello
├── delivery.rs     DeliveryHandler, DefaultDelivery
├── local.rs        LocalDispatcher
└── errors.rs       error_control helpers, StateError -> Control
backends/foundation_ui_runtime/tests/{dispatch_hello_tests.rs,dispatch_tests.rs,dispatch_errors_tests.rs,dispatch_delivery_tests.rs,dispatch_local_tests.rs,dispatch_fuzz_tests.rs}
```

### 6.2 Public API

```rust
pub trait DeliveryHandler: Send + Sync {
    /// Encodes the answer messages, in order, for one client. It must call the gate
    /// (use `write_messages`).
    fn messages(&self, messages: Vec<MessageT>, caps: &ClientCaps, header: FrameHeader)
        -> Result<Vec<u8>, WriteError>;
}

/// `messages` calls `write_messages`.
#[derive(Clone, Debug, Default)]
pub struct DefaultDelivery { /* config: WriterConfig */ }
impl DefaultDelivery { #[must_use] pub fn new(config: WriterConfig) -> Self; }
impl DeliveryHandler for DefaultDelivery {}

pub struct DispatcherBuilder { /* … */ }
impl DispatcherBuilder {
    /// `accept` is the one origin that this dispatcher runs.
    #[must_use] pub fn new(accept: Origin, registry: Arc<ActionRegistry>) -> Self;
    #[must_use] pub fn delivery(self, delivery: Arc<dyn DeliveryHandler>) -> Self; // default DefaultDelivery
    #[must_use] pub fn writer_config(self, config: WriterConfig) -> Self;          // used for control frames, the caps frame and limits
    #[must_use] pub fn build(self) -> Dispatcher;
}

pub struct Dispatcher { /* private */ }
impl Dispatcher {
    /// Reads one request frame whose `hello` is set. A frame of another kind is
    /// `HelloError::Wire`; a request with `action` and no `hello` is `NotAHello`.
    pub fn hello(&self, ctx: &Ctx, hello: &[u8])
        -> Result<HelloOutcome, HelloError>;
    /// Runs one action. Always returns the bytes of one frame.
    pub async fn dispatch(&self, ctx: &Ctx, connection: &Connection, request: &[u8]) -> Vec<u8>;
    /// The `server:` actions for `surface`, sorted by name (D54).
    #[must_use] pub fn server_actions(&self, surface: Surface) -> Vec<ServerActionT>;
}

/// A hello that the dispatcher cannot read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HelloError {
    /// `read_frame` or `expect_kind` failed.
    Wire(WireError),
    /// The frame is a request, but it holds no `hello` field.
    NotAHello,
}
impl From<WireError> for HelloError { /* gives `Wire` */ }

pub enum HelloOutcome {
    Accepted { connection: Connection, caps_frame: Vec<u8> },
    UpdateRequired { caps_frame: Vec<u8> },
}

pub struct Connection { /* caps: ClientCaps, surface: Surface, sequencer: Mutex<StreamSequencer>, last_request: AtomicU64,
    check_request_order: bool */ }
impl Connection {
    /// For a transport with no handshake state between requests (one HTTP request for each
    /// action). It does not check request order (I10 does not apply).
    #[must_use] pub fn stateless(caps: ClientCaps, surface: Surface, stream_id: u64) -> Self;
    #[must_use] pub fn caps(&self) -> &ClientCaps;
    #[must_use] pub fn surface(&self) -> Surface;
    #[must_use] pub fn stream_id(&self) -> u64;
}

/// A dispatcher behind the transport trait, for `host:` actions.
pub struct LocalDispatcher { /* dispatcher: Dispatcher, ctx: Ctx, connection: Connection */ }
impl LocalDispatcher {
    #[must_use] pub fn new(dispatcher: Dispatcher, ctx: Ctx, connection: Connection) -> Self;
}
impl ActionTransport for LocalDispatcher { /* send awaits dispatch and returns its answer as Some(frame) */ }

```

### 6.3 `hello`, in order

Let `frame` be the read `Frame`, and `client_hello` its `FrameRequest.hello`.

1. `read_frame(hello, limits)` with `?`; then `expect_kind(&frame, FrameKindName::Request)`
   with `?`. A frame whose kind is `Message` or `NONE` is a wire fault (`WrongKind` or
   `NoKind`). The `hello` field of the `FrameRequest` must be set; a request that holds
   `action` and no `hello` gives `HelloError::NotAHello`.
2. `frame.schema_version() != SCHEMA_VERSION` gives `UpdateRequired`.
3. `vocab_version(client_hello.vocabulary_version())` of `None` gives `UpdateRequired`. Else
   `vocab = min(client_hello.vocabulary_version(), VOCABULARY_VERSION)`.
4. Use the requested mount as the registry surface. The connection caps are
   `ClientCaps { platform, mount, vocabulary_version: vocab }`.
5. Add server action enumeration to the feature 05 registry for this handshake.
   Include callable Rust entries whose origin permits the server and whose scope accepts the mount.
   Omit client declarations without an implementation. Sort by name and encode each declaration fingerprint in the generated `ServerActionT` (D54).
6. `stream_id`: if `frame.stream_id() != 0`, keep it and start the sequencer with
   `StreamSequencer::resume(stream_id, client_hello.resume_from())`. Else take a `u64` from
   `ctx.rng()`; if it is 0, use 1.
7. Build `ServerCapsT { status: Ok, schema_version: SCHEMA_VERSION, vocabulary_version: vocab,
   stream_id, resume_from: client_hello.resume_from(), actions }` and `write_caps` with
   `FrameHeader { stream_id, sequence_id: 0, ack: 0 }`. A caps frame has sequence 0:
   it is not part of the ordered stream.
8. Return `Accepted` with `check_request_order = true`.

For `UpdateRequired`, the caps frame has `status: UpdateRequired`, the server versions,
`stream_id: 0`, sequence 0 and the same `actions` list. The list is a declaration of the
server at any status, so a client that needs to show what it supports can read it in both
cases. The client must not send a request after an `UpdateRequired`.

### 6.4 `dispatch`, in order

```mermaid
flowchart TD
    R[request bytes] --> P{read one request frame}
    P -->|fail| EB[Control error BadRequest, ack 0]
    P --> V{schema_version == ours?}
    V -->|no| EU[Control update_required]
    V --> O{sequence_id > last seen?}
    O -->|no| EB2[Control error BadRequest]
    O --> A{ActionRef parses and origin == accept?}
    A -->|bad text| EB3[Control error BadRequest]
    A -->|other origin| EO[Control error UnavailableOrigin]
    A --> G{registry.get}
    G -->|unknown| EK[Control error UnknownAction]
    G -->|scope| ES[Control error UnsupportedAction]
    G --> S{action needs state?}
    S -->|yes| OP{verify_state}
    OP -->|Expired| EX[Control state_expired]
    OP -->|other error| EJ[Control error StateRejected]
    OP --> RUN
    S -->|no| RUN[call_raw]
    RUN -->|Err| EA[drop receiver, Control error]
    RUN -->|Ok| SEAL[seal each state item] --> DEL[delivery.messages]
    DEL --> F[answer frame]
```

_The diagram shows the fixed order of one dispatch. Every exit is one frame._

Details:

1. **Read.** `read_frame(request, limits)`, then `expect_kind(&frame,
   FrameKindName::Request)`. A message-kind frame, a frame with `NONE` kind, or a request
   that holds no `action` gives one `Control{error}` message with code `BadRequest` and
   `ack 0` when the sequence is unknown. Let `action_request` be the `FrameRequest.action`;
   `input`, `state` and `node_id` are its fields.
2. **Version.** `frame.schema_version() != SCHEMA_VERSION` gives a
   `Control{update_required}` message.
3. **Order.** Only when `check_request_order`: compare `frame.sequence_id()` with
   `last_request`; on success store the new value (`fetch_max`).
4. **Reference.** `ActionRef::parse(action_request.action())`.
5. **Look-up.** `registry.get(origin, name, connection.surface())`.
6. **Target.** Resolve the component render target from the host's routing information.
7. **Invoke.** Await `invoke_action(ctx, entry, connection.surface(), root, request)` with the owned request.
   It verifies the attachment before it decodes the state. It returns collected output only when the handler succeeds.
8. **Errors.** Map `InvokeError::State` and `InvokeError::Action` using the table below.
9. **Header.** Take the next header from the connection sequencer. Release its lock before delivery.
10. **Sign.** Pass the collected items and the same context to `prepare_messages(ctx, items)`.
11. **Messages.** Pass the resulting messages in receiver order. A signing failure returns one control error.
12. **Deliver.** `delivery.messages(messages, caps, header)`. A `WriteError` gives a
    `Control{error}` message with code `ActionFailed` and the message of the error.
13. **Control messages.** `write_error`, `write_caps` or a helper for `state_expired` and
    `update_required` with the same header. If the message makes the frame too large, write
    it again with an empty message.

**Control mapping**

| Internal result | `Control` field |
|---|---|
| Read failure; bad reference text; missing state; repeated sequence | `error: ActionError{ code: BadRequest, message: "bad request" }` |
| The frame kind is not a request, or the request holds no action | `error: ActionError{ code: BadRequest, message: "bad request" }` |
| Other schema version or unknown vocabulary version | `update_required: UpdateRequired{ client_schema_version, server_schema_version }` |
| Origin is not the accept origin | `error: ActionError{ code: UnavailableOrigin, message: "origin not available" }` |
| `LookupError::UnknownAction` | `error: ActionError{ code: UnknownAction, message: "unknown action" }` |
| `LookupError::UnsupportedAction` | `error: ActionError{ code: UnsupportedAction, message: "action not supported on this surface" }` |
| Missing signer or host service | `error: ActionError{ code: ActionFailed }` |
| `StateError::Expired` | `state_expired: StateExpired{ token_issued_at: 0 }` |
| Invalid submitted state | `error: ActionError{ code: StateRejected, message: "state rejected" }` |
| `Err(ErrorTrace<ActionError>)` from the action | `error` with the public code and message of the runtime error. Keep the trace in the host log |
| Seal or delivery failure | `error: ActionError{ code: ActionFailed, message: "<short reason>" }` |

### 6.5 Fault tables

**`dispatch`**

| Step that fails | What the system does | What the caller sees | What is left behind |
|---|---|---|---|
| 1 to 7 | Stops before the action | One `Control` message | nothing (step 3 stored the sequence id only on success) |
| 7 (state) | Stops before the action | `StateRejected` or `state_expired` | No action output |
| 8 (the action returns `Err`) | Drops the receiver | One `Control{error}` message with the action's code | What the action changed in the domain data. The action owns its own atomicity ("one action is one unit of work", research 10) |
| 8 (the action panics) | The dispatcher does not catch it | The host's task fails; the client gets no frame and its transport reports an error | What the action changed before the panic. The frame counter did not move |
| 10 (seal) | Drops the messages | One `Control{error}` message with code `ActionFailed` | The domain change of the action stays; the client keeps its old state, which is still valid; the next action reads the truth from the backend (D13) |
| 12 (delivery) | Drops the messages | One `Control{error}` message with code `ActionFailed` | The same as step 10 |

**`hello`**

| Step that fails | What the system does | What the caller sees | What is left behind |
|---|---|---|---|
| 1 | Stops | `Err(HelloError)` (a wire fault or `NotAHello`); the host closes the connection | nothing |
| 2, 3 | Builds the caps frame | `UpdateRequired` | nothing |

### 6.6 How a host uses it (not code of this feature)

```rust
// one HTTP request for each action, any replica
let connection = Connection::stateless(caps, surface, stream_id);
let frame = dispatcher.dispatch(&ctx, &connection, &body).await;
// the host writes `frame` to its response; framing is the transport's business (D25)
```

A sync caller wraps the call with valtron (`block_on_future`), as rule 6 says. The
dispatcher has no sync twin.

## 7. Tests

All in `backends/foundation_ui_runtime/tests/`. Reuse the test actions and `test_ctx` of
features 04 and 05. Add `Probe`-wrapped variants to see if an action ran.

| Law or scenario | Test | Oracle |
|---|---|---|
| I1; Scenario: bytes that are not a request | `dispatch_fuzz_tests.rs::every_input_gives_one_valid_frame` (each prefix of a valid request, each single bit flip of its first 128 bytes, and 200 seeded random buffers; `read_frame` must accept each answer) | property |
| I2 | `dispatch_errors_tests.rs::action_never_runs_when_a_check_fails` (one case for each exit of steps 1 to 7; the `Probe` stays false) | fault |
| I3; Scenario: a server refuses a host reference | `dispatch_errors_tests.rs::server_refuses_host_reference`, `::local_refuses_server_reference` | expected outcome |
| I4, I7; Scenario: a replica that never saw the user | `dispatch_tests.rs::any_replica_answers` | metamorphic |
| I5; Scenarios: forged state, expired state | `dispatch_errors_tests.rs::forged_state_is_rejected`, `::expired_state_skips_action` | expected outcome |
| I6; Scenario: native connection gets no web field | `dispatch_delivery_tests.rs::native_answer_has_no_web_field` | example |
| I8 | Review dispatcher fields and run concurrent user contexts | isolation |
| I9; Scenarios: three hello cases | `dispatch_hello_tests.rs::current_versions_are_accepted`, `::unknown_vocabulary_needs_update`, `::other_schema_version_needs_update`, `::resume_keeps_stream_and_sequence` | example |
| I10; Scenario: repeated sequence id | `dispatch_errors_tests.rs::repeated_sequence_is_refused`, `::stateless_connection_does_not_check_order` | expected outcome |
| I11; Scenario: hello lists the actions | `dispatch_hello_tests.rs::caps_actions_are_sorted_and_hashed` | example |
| I12; Scenario: action error | `dispatch_errors_tests.rs::action_error_drops_the_receiver` | expected outcome |
| I13; Scenario: expired state | `dispatch_errors_tests.rs::expired_state_sends_state_expired` (asserts `control.state_expired.is_some()` and `control.error.is_none()`) | expected outcome |
| Scenarios: unknown action, data message | `dispatch_errors_tests.rs::unknown_action_is_typed`; `dispatch_delivery_tests.rs::data_goes_in_a_data_message`, `::custom_delivery_is_used` | example |
| Scenario: local dispatcher | `dispatch_local_tests.rs::manager_triggers_local_action` | example |
| Fault: seal fails | `dispatch_errors_tests.rs::seal_failure_gives_action_failed` (an action that writes a state of 5000 bytes) | fault |
| Frame sequence increases | `dispatch_tests.rs::answers_have_increasing_sequence` | property |
| The finalizer signs once and emits the same bytes to each selected target | `dispatch_tests.rs::sign_once_for_marked_targets` | example |

## 8. Tasks

- [ ] 1. Test signed attachments through the feature 05 receiver and the injected delivery handler.
- [ ] 2. `delivery.rs` and its tests.
- [ ] 3. `errors.rs`: the control mapping table as one function, with a table test.
- [ ] 4. `hello.rs`, the action list and its tests.
- [ ] 5. `dispatcher.rs`: steps 1 to 7 and the error tests.
- [ ] 6. Steps 8 to 13 and the success tests.
- [ ] 7. `local.rs` and its test.
- [ ] 8. The fuzz-style test of I1.
- [ ] 9. Bump `foundation_ui_runtime` to `0.0.3`.
- [ ] 10. Update `progress.md` and `learnings.md`.
- [ ] 11. Write or update the package or feature README.
      Show a hello handshake and an action request. Run the example against the dispatch tests.

## 9. Verify

```bash
cargo fmt --all -- --check
cargo check  -p foundation_ui_runtime --profile uat
cargo clippy -p foundation_ui_runtime --profile uat --no-deps -- -D warnings
cargo test   -p foundation_ui_runtime --profile uat
cargo check  -p foundation_ui_runtime --profile uat --target wasm32-unknown-unknown
python3 scripts/ci/check_time_sources.py
```

## 10. Done when

- [ ] The package or feature README is current. Its examples and validation commands pass.
- [ ] All tasks are complete and every command of section 9 passes with no warning.
- [ ] Each invariant and each scenario has a passing test (section 7).
- [ ] A reviewer confirms that `Dispatcher` has no field that grows with the number of
      users (I8).
- [ ] No lock is held across an `.await`.
- [ ] `backends/foundation_wasm_ui/src/server/mod.rs` is unchanged.
- [ ] No `TODO`, `FIXME`, `todo!()` or `unimplemented!()` is in the module.
- [ ] The check list of the `story-driven-specification` skill passes.

---

_Created: 2026-11-13_
_Revised: 2026-11-14 (D41 to D59)_
