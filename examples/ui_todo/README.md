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
The add form copies its named title field into its action value during submit.

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
Use its fixture generator for byte-level compatibility tests. The browser test submits a real form and clicks a stale delete control.
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
