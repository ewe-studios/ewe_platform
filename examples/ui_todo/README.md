# Todo actions on web and native screens

The page uses shared Rust templates, a native Hypertext Transfer Protocol (HTTP) server and a browser WebAssembly module.
The server owns one in-memory list. Each browser owns its dialog and filter settings.
The example binds to a loopback address. Restarting the server clears the list.

## Run the example

Install the WebAssembly target for the repository toolchain. Build the module and start the server.

```sh
rustup target add wasm32-unknown-unknown
examples/ui_todo/build-module.sh
cargo run --profile uat -p ui_todo
```

Open `http://127.0.0.1:8180`. Pass another loopback address after `--` to use a different port.

## Actions and state

| Action | Owner | Effect |
|---|---|---|
| `open_add_dialog` | Host | Shows the form and focuses the title field. |
| `open_delete_dialog` | Host | Opens a confirmation dialog for the selected identifier. |
| `close_dialog` | Host | Closes the dialog. |
| `filter_todos` | Host | Renders the selected rows from the input's signed snapshot. |
| `add_todo` | Server | Checks the submitted title, appends one item and sends a notification. |
| `toggle_todo` | Server | Finds the item in the current list and changes its completion status. |
| `confirm_delete` | Server | Removes the item. Repeating a delete returns an action error. |
| `notify` | Web page or mobile host | Announces a successful add. |
| `scroll_to_top` | Web page | Scrolls to the list header. |

The browser boots its local dispatcher before it requests the initial frame.
The server advertises its three actions during the hello exchange.
An action carries the triggering element's value and its own optional signed bytes.
All three platforms render the same `<Modal><BottomSheet>…</BottomSheet></Modal>` tree. The input owns the submit action and supplies its current value.

Only presentation actions consume signed state. The demo signer protects snapshots used by the local dispatcher.
Its demo key is present in the module. The server rejects any attached presentation snapshot at its business endpoint.
The server looks up each item in its own store. The initial page and every successful mutation use the same Rust render function.
Server and browser allocate node identifiers in separate ranges. Each producer keeps both page roots stable.

## Wire format and tests

The action endpoint accepts an Ewe wire frame (EWF1) and a base64url client hello in the `x-ewe-hello` header.
It responds with `application/primal-frame`. A four-byte little-endian size precedes each response frame. The frame contains ordered messages and an acknowledgement.
The count stream at `/live` pushes a complete replacement to the count element after each successful mutation.
The count stream has identifier 1. To resume after frame 12, connect to `/live?stream_id=1&resume_from=12`.
The server replays later count frames before it sends a new update.
The add response carries the MessagePack string `host:notify` as a data message.
The web page invokes its registered notification function when this message arrives.

The Rust wire crate describes generated tables, size checks, fixtures and hand-written requests.
Use its fixture generator for byte-level compatibility tests. The browser test submits a modal editor and clicks a stale delete control.
See the [wire format and examples](../../backends/foundation_ui_wire/README.md) and the [browser test](tests/todo_browser_tests.rs).

```sh
examples/ui_todo/build-module.sh
cargo test --profile uat -p ui_todo --features browser-tests --test todo_browser_tests
```

## Native screens

Both native examples render the Rust view with the platform library. Each example has its own application identifier.

| App | iOS | Android | Where the first screen and task changes come from |
|---|---|---|---|
| Todo Server | [Swift app](ios/todo_server/README.md) | [Kotlin app](android/todo_server/README.md) | The Rust server sends the first frame and handles task changes. The device handles filters and dialogs. |
| Todo On Device | [Swift app](ios/todo_on_device/README.md) | [Kotlin app](android/todo_on_device/README.md) | The app calls Rust in its own process. Its store, actions and count updates work with the server stopped. |

The [Swift library](../../mobile/ios/README.md) and [Kotlin library](../../mobile/android/README.md) own the native controls and event delivery.
This example owns the task store, the Rust action handlers, the view and its application bindings.
The package-only starters exercise a remote Refresh action. See the [iOS starter](../mobile_starter/ios/README.md) and [Android starter](../mobile_starter/android/README.md).

To run Todo Server, start the example Rust host and open either server app.
The app reads `/native-initial` and sends mutations to `/action`.
The count subscribes to `/native-live?stream_id=2&resume_from=0`. A reconnect uses the last applied sequence.
Follow the linked app guide for the address used by each simulator.

To run Todo On Device, launch its own target on the device. Rust supplies the initial frame before the first tap.

## Glossary

- EWF1 identifies a version-one Ewe wire frame.
- HTTP means Hypertext Transfer Protocol.
- WebAssembly runs compiled Rust in the browser.

The initial page routes `/initial`, `/native-initial` and `/native-starter` return one bare EWF1 frame with content type `application/vnd.ewe.frame`.

### Server setup

The binary binds the listener. The application builder installs its services and registers the routes on the router.
The initial routes return the page tree and its state. The response adapter signs state and writes the frame when the body is requested.
A successful mutation publishes a count to each live channel. The action endpoint validates and dispatches the request.
The test can supply its own web channel to disconnect a subscriber and check replay.

## Native todo registry

This crate builds the example's Rust action registry. Its `create_todo_core` function accepts a native platform and a transport. Swift and Kotlin use the generated bindings to call it.

The host handles `open_add_dialog`, `close_dialog`, `open_delete_dialog` and `filter_todos`. The server handles `add_todo`, `toggle_todo` and `confirm_delete`. The example signs presentation state. Business actions read the server's store and reject attached presentation state.

On-device apps call `create_local_todo_core` with a platform. Call `initial_todo_frame` once to mount its first screen.
The core owns an in-memory store for the app session. Its action registry runs add, complete, reopen and delete on the device.
Each reply updates the list and count. The on-device core requires no transport.

The compiler links the domain actions and native core into one library. Swift and Kotlin refer to that same library for callback handles and Rust buffers. The independent starter uses the package's remote factory.

The transport sends one EWF1 frame per request. Strip the four-byte HTTP length before returning a response to the core. The renderer returns each trigger's signed bytes unchanged. See the wire crate README for hand-written frames and fixture generation.

## Build outputs from one package

The shared directory `src/shared` contains the store, actions, and views.
The package selects one entry point for each host.

| Host | Source | Build command from the repository root | Artifact |
|---|---|---|---|
| Server | `src/bin/server.rs` | `cargo build --profile uat -p ui_todo --bin ui_todo` | `target/uat/ui_todo` |
| Browser | `src/web.rs` | `examples/ui_todo/build-module.sh` | `examples/ui_todo/target/web/ui_todo.wasm` |
| iOS | `src/mobile.rs` | `scripts/ui_native/prepare-ios-todo.sh` | `target/native-ui/ios-todo/.build-inputs/EweCoreFFI.xcframework` |
| Android | `src/mobile.rs` | `scripts/ui_native/build-android.sh --with-todo` | Each example gets `app/src/main/jniLibs/<abi>/libui_todo.so`. |

The server is the default Cargo feature. The browser script selects `web`. The mobile scripts select `mobile`.
Swift imports `EweTodoCore`. Kotlin imports the actions from `io.github.ewe_studios.ewe.todo`.

## Ordinary data and UI actions over Connect

Start the data-only example with `cargo run --profile uat -p ui_todo -- 127.0.0.1:8180 --data-only`.
The generated Todos client reads task 42 and returns its identifier and title. Task 999 returns NotFound.
The native integration suites run this procedure in a separate process from the screen test.

The normal server also exposes the generated Actions service. Open `/?rpc` to use it from the browser.
Run the native tests with `TEST_RUNNER_EWE_TODO_RPC=1` on iOS or `EWE_TODO_RPC=true` on Android to use it for UI actions.
[Typed Swift and Kotlin examples](../../backends/foundation_ui_connectrpc/README.md)
