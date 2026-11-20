# Todo actions on the web

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
The add response carries the MessagePack string `host:notify` as a data message.
The web page invokes its registered notification function when this message arrives.

The Rust wire crate describes generated tables, size checks, fixtures and hand-written requests.
Use its fixture generator for byte-level compatibility tests. The browser test submits a real form and clicks a stale delete control.
See the [wire format and examples](../../backends/foundation_ui_wire/README.md) and the [browser test](tests/todo_browser_tests.rs).

```sh
examples/ui_todo/build-module.sh
cargo test --profile uat -p ui_todo --features browser-tests --test todo_browser_tests
```

## Glossary

- EWF1 identifies a version-one Ewe wire frame.
- HTTP means Hypertext Transfer Protocol.
- WebAssembly runs compiled Rust in the browser.
