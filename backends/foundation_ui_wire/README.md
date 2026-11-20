# foundation_ui_wire

`foundation_ui_wire` is the binary contract for Foundation user interface (UI) requests and updates.
Use it to send an event to application logic, return view changes, or exchange data with a renderer.
The crate provides generated types, Rust readers that verify received bytes, and writers for frames and signed state.
Rendering, action dispatch, state authentication and network transport belong to their respective runtimes.

For a Rust crate under `backends/`, add the local dependency to its `Cargo.toml`:

```toml
[dependencies]
foundation_ui_wire = { path = "../foundation_ui_wire", version = "0.0.1" }
```

The generated types and read functions are exported directly from `foundation_ui_wire`.
A browser can import the [JavaScript module](../foundation_wasm_ui/runtimes/foundation-ui-wire.js) as an ECMAScript module.

## Read a frame

A **frame** is one complete FlatBuffers message. FlatBuffers stores little-endian values and offsets into the same buffer.
The first four bytes locate the root table. Bytes 4 through 7 hold its four-byte file identifier.
The wire format uses ordinary FlatBuffers buffers without an extra size prefix.
The transport must supply the boundary of each frame. The protocol specifies `application/primal-frame` for transport over HTTP (Hypertext Transfer Protocol).

| Root identifier | Read / write | Contents |
|---|---|---|
| `EWF1` | `read_frame` / `finish_frame` | A request or an ordered reply, with schema version, stream identifier and sequence number. |
| `EWS1` | `read_signed_state` / `finish_signed_state` | A component state envelope, with key identifier, time range, session binding, body and authentication tag. |

`SCHEMA_VERSION` is 1. The vocabulary version in a hello describes the available elements and properties. It has a separate version number.
A reader borrows the received bytes. Keep the buffer alive while using the view. Rust uses the standard library by default and also supports `no_std` with allocation.
Use typed accessors for individual fields. Call `unpack()` when you need an owned value.

| Direction | Payload | How to use it |
|---|---|---|
| Client to logic | `FrameRequest.hello` | Set the platform, mount surface, vocabulary version and last applied sequence. Stream zero asks for a new stream. |
| Client to logic | `FrameRequest.action` | Send an action reference, node identifier, event input and optional signed state bytes. |
| Logic to client | `FrameMessage.messages` | Apply messages in list order. `ack` identifies the request sequence. Zero indicates an unsolicited update. |

Choose one request field per request. Use increasing sequence numbers within a stream.
Each reply message carries an operation, data, or control information. The application enforces those protocol rules after structural verification.

- **Operation.** A shared, web or mobile operation changes the view. A single step can carry both web and mobile forms. Each renderer selects its form.
- **Data.** Send an Arrow interprocess communication payload, a typed data table, or a MessagePack byte block to an application node.
  Table columns support 32-bit integers, 64-bit integers, 64-bit floats, booleans and text.
- **Control.** `ServerCaps` advertises the stream, versions and server actions. `StateExpired` requests a state reload.
  `UpdateRequired` reports incompatible versions. `ActionError` carries an error code, message and row index.

See the [frame schema](schema/frame.fbs), [hello schema](schema/hello.fbs) and [signed state schema](schema/state.fbs) for field types and defaults.

## Actions and operations

An **action** is an application-defined name that a runtime resolves. For example, the counter fixture calls `server:increment_counter`.
The crate encodes the name as text. The application must register an implementation.
The action protocol uses `server:name` for backend logic and `host:name` for a host handler.
The runtime design permits a missing host handler to fall forward as `server:name` when the server advertises that name.

`BindAction` associates a node event with an action. `UnbindAction` removes the association.
The shared event vocabulary contains tap, change, submit, focus and blur.
An `ActionRequest` names the action and triggering node. Its optional input holds the event name, value, checked state, key code, modifiers and parent node.
The modifier bits are Alt (1), Control (2), Shift (4) and Meta (8).
The optional state bytes hold the component root's signed envelope. A successful structural read does not authenticate that envelope.
[Action input](schema/action.fbs) · [Shared vocabulary](schema/common.fbs) · [Runtime routing design](../../specifications/69-foundation-ui-v2/features/10-web-action-manager/feature.md)

An **operation** is a typed instruction in a reply. The schema defines 31 operation forms.

| Group | Forms |
|---|---|
| Shared tree and content | `CreateNode`, `SetText`, `SetProp`, `AppendChild`, `InsertBefore`, `Remove`, `ReplaceSubtree` |
| Shared events and lifecycle | `BindAction`, `UnbindAction`, `AttachState`, `LiveMount`, `UiBehaviour` |
| Web nodes and markup | `WebElement`, `WebText`, `WebReplaceNode`, `HtmlMorph`, `WebRegisterNode`, `WebUnregisterNode`, `ScriptNode` |
| Web attributes and behavior | `WebSetAttribute`, `WebRemoveAttribute`, `WebProperty`, `WebAddListener`, `WebRemoveListener`, `WebStyle`, `WebAddClass`, `WebRemoveClass`, `WebBehaviour` |
| Mobile | `MountWebView`, `NativeCommand`, `MobileBehaviour`. The command identifiers are `HAPTIC`, `OPEN_URL` and `SHARE`. |

A `ColumnarBatch` carries multiple shared operations in parallel vectors. Each row uses the numeric tag from `UiOperation`.
Use `create_aligned_vector` when building numeric columns that require eight-byte alignment. The generated object writer alone cannot guarantee that alignment.
The [operation schema](schema/ops.fbs) gives the exact field layout and union order. The [fixture writer](tests/fixtures_tests.rs) builds a three-row batch.

## Write a wire message by hand

Run the [standalone Rust consumer](../../examples/wire_text_update/src/main.rs) from the repository root:

```bash
cargo run --locked --manifest-path examples/wire_text_update/Cargo.toml
```

The program exits successfully when the decoded node and text match. This Rust example writes a reply that changes node 42 to the text “Hello”. The generated owned types end in `T`.
Build the inner operation first, then the message list and frame. `finish_frame` writes the root offset and identifier.

```rust
use foundation_ui_wire::{
    finish_frame, read_frame, FrameKindT, FrameMessageT, FrameT, MessageT,
    OperationT, SetTextT, UiOperationT, WireError, WireLimits, SCHEMA_VERSION,
};

fn main() -> Result<(), WireError> {
    let mut text = SetTextT::default();
    text.node_id = 42;
    text.text = Some("Hello".into());

    let mut operation = OperationT::default();
    operation.ui = UiOperationT::SetText(Box::new(text));
    let mut message = MessageT::default();
    message.operation = Some(Box::new(operation));

    let mut reply = FrameMessageT::default();
    reply.ack = 5;
    reply.messages = Some(vec![message]);

    let mut frame = FrameT::default();
    frame.schema_version = SCHEMA_VERSION;
    frame.stream_id = 7;
    frame.sequence_id = 20;
    frame.kind = FrameKindT::FrameMessage(Box::new(reply));

    let bytes = finish_frame(&frame)?;
    let view = read_frame(&bytes, &WireLimits::default())?;
    let reply = view.kind_as_frame_message().expect("built a reply");
    let first = reply.messages().expect("built one message").get(0);
    let text = first.operation().unwrap().ui_as_set_text().unwrap();
    assert_eq!((text.node_id(), text.text()), (42, Some("Hello")));
    Ok(())
}
```

For a hand-authored **request**, the FlatBuffers compiler (`flatc`) also accepts JavaScript Object Notation (JSON).
From the repository root, run the following commands. They create an action request and decode it again for inspection.

```bash
schema=backends/foundation_ui_wire/schema/frame.fbs
work=$(mktemp -d)
cat > "$work/tap.json" <<'JSON'
{
  "schema_version": 1,
  "stream_id": 7,
  "sequence_id": 5,
  "kind_type": "FrameRequest",
  "kind": {
    "action": {
      "action": "server:increment_counter",
      "node_id": 42,
      "input": { "event_name": "tap" }
    }
  }
}
JSON
mkdir "$work/decoded"
mise exec -- flatc --binary --strict-json -o "$work" "$schema" "$work/tap.json"
mise exec -- flatc --json --strict-json -o "$work/decoded" "$schema" -- "$work/tap.bin"
cat "$work/decoded/tap.json"
```

Send the contents of `tap.bin` as the frame. Here `kind_type` selects the table stored in `kind`.
The example omits state. A handler that requires state needs a valid envelope from its state issuer.

## Verification and compatibility

Use `WireLimits::default()` or set limits on its builder before reading external bytes.

| Limit | Default | Setter |
|---|---:|---|
| Frame size | 4 mebibytes | `max_frame_bytes` |
| Action frame or separate signed state | 1 mebibyte | `max_action_request_bytes` |
| Nested table depth | 64 | `max_depth` |
| Verified tables | 1,000,000 | `max_tables` |

A zero setter value becomes one. The reader checks the frame size, minimum header length and identifier before running the verifier.
It applies the action-specific size cap after identifying a verified action request.
Handle `TooLarge`, `TooShort`, `WrongIdentifier` and `Invalid` as read failures.
A missing root kind gives `NoKind`. Use `expect_kind` on an owned frame to reject the opposite direction with `WrongKind`.

A borrowed operation preserves an unfamiliar numeric tag. Its known typed accessors return no value for that tag.
Unpacking maps it to the empty union member (`NONE`). Other known fields in the same message remain readable.
Append new fields and union members. Preserve released field positions and numeric tags.

## Write and update fixtures

A **fixture** is a checked-in binary with known inputs and expected fields. Rust writes the bytes. TypeScript reads those same files through the bundled browser module.

| Fixture files in [fixtures/](fixtures/) | What they exercise |
|---|---|
| `frame_ops_counter.bin`, `frame_ops_all_forms.bin` | One text change and all 31 operation forms. |
| `frame_columnar_3rows.bin`, `frame_data_table.bin` | Aligned numeric vectors and typed application data. |
| `frame_error.bin`, `frame_state_expired.bin`, `caps_actions.bin` | Action failure, expired state and advertised server actions. |
| `action_increment.bin`, `hello_web.bin`, `state_counter.bin` | Action with embedded state, stream handshake and the separate state envelope. |
| `frame_unknown_op.bin` | An unknown tag alongside a valid operation payload. |

1. Add a deterministic value and filename to [fixtures_tests.rs](tests/fixtures_tests.rs). Use generated types and explicit identifiers.
2. For a frame, call `finish_frame`. For a separate state, call `finish_signed_state`. Use the aligned builder for columnar data.
3. Add assertions in [fixtures.test.mjs](tests/ts/fixtures.test.mjs) for the decoded fields. Use JavaScript `bigint` for 64-bit integer assertions.
4. Write the fixture files once. Inspect the changed filenames. Run the tests again with updates off.

```bash
UPDATE_FIXTURES=1 cargo test -p foundation_ui_wire --profile uat --test integration fixtures_match_built_values
cargo test -p foundation_ui_wire --profile uat
mise run ui-wire:test-ts
```

Ordinary Rust runs compare the rebuilt bytes with the recorded bytes. Commit the fixtures and both sets of assertions together.

## Generate bindings after a schema change

Edit the six files in [schema/](schema/). The compiler pin comes from the root `mise.toml`.
Run these commands from the repository root.

```bash
pnpm --filter foundation-ui-wire-bindings install --frozen-lockfile
mise run ui-wire:generate
mise run ui-wire:check
```

Rust bindings live in [src/generated/](src/generated/). TypeScript, Swift and Kotlin bindings live in [generated/](generated/).
The browser consumes the [bundled JavaScript module](../foundation_wasm_ui/runtimes/foundation-ui-wire.js).
The Rust verifier checks unknown union table locations before the reader returns a view. The drift check regenerates into a temporary directory and compares outputs.
Change the schema or generator to update generated code. Rust callers can disable the default `std` feature for targets with allocation and no standard library.

## Look up vocabulary and platform forms

The vocabulary gives each property one value type. For example, padding takes a length.
A peer version row gives the last property number that the peer accepts.
The operation union identifies the form group. Shared forms apply on both mount surfaces.
Web forms apply on web mounts. The frame writer enforces the mount rule before it sends a frame.

Run [the vocabulary example](examples/vocabulary.rs) from the repository root.

```sh
cargo run -p foundation_ui_wire --profile uat --example vocabulary
```

The example validates a padding value against vocabulary version 1.
It converts a text update to a shared form and an attribute update to a web form.
It then checks each form against the native and web mount surfaces.

Regenerate the JavaScript vocabulary after a change to its Rust tables. Run all vocabulary tests to check the result.

```sh
UPDATE_FIXTURES=1 cargo test -p foundation_ui_wire --profile uat --test integration vocab_js_tests::js_file_matches_tables
cargo test -p foundation_ui_wire --profile uat --test integration vocab_
node --check backends/foundation_wasm_ui/runtimes/foundation-ui-vocab.js
```
