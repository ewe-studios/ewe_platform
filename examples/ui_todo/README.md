# Todo actions on web and native screens

The page uses shared Rust templates, a native Hypertext Transfer Protocol (HTTP) server and a browser WebAssembly module.
The server saves confirmed items in its database. Each browser owns its dialog and filter settings.
The example binds to a loopback address. Restart it with the same database to recover its list.

## Run the example

Install the WebAssembly target for the repository toolchain. Start the server with Cargo.
The build script sets the public browser binary path with `with_module_path("module.wasm")`.
Change that path in `build.rs` to stage the binary and update its browser address.

```sh
rustup target add wasm32-unknown-unknown
cargo run --profile uat -p ui_todo
```

Open `http://127.0.0.1:8180`. Pass another loopback address after `--` to use a different port.

Cargo builds the browser module, its bindings, and the application script before it builds the server.
Move the development executable and its sibling `public` directory together.
The release executable embeds the same resources.

The build validates each browser module before it stages the resource manifest. The server mounts the packaged resources after the application routes.

## Actions and state

| Action | Owner | Effect |
|---|---|---|
| `open_add_dialog` | Host | Shows the form and focuses the title field. |
| `open_delete_dialog` | Host | Opens a confirmation dialog for the selected identifier. |
| `close_dialog` | Host | Closes the dialog. |
| `search_todos` | Host | Filters cached titles when the query changes. |
| `edit_draft` | Host | Saves the current title while the dialog is open. |
| `filter_todos` | Host | Renders selected rows from the local cache. |
| `add_todo` | Server | Checks the submitted title, commits the item and returns its record change. |
| `toggle_todo` | Server | Finds the item in the current list and changes its completion status. |
| `confirm_delete` | Server | Removes the item. A retry returns the original removal position. |
| `scroll_to_top` | Web page | Scrolls to the list header. |

The browser boots its local dispatcher before it requests the initial frame.
The server advertises its callable actions during the hello exchange.
An action carries the triggering element's value and its own optional signed bytes.
All three platforms render the same `<Modal><BottomSheet>…</BottomSheet></Modal>` tree. The input owns the submit action and supplies its current value.

Only presentation actions consume signed state. The demo signer protects snapshots used by the local dispatcher.
Its demo key is present in the module. The server rejects any attached presentation snapshot at its business endpoint.
The server looks up each item in its own store. Each mutation returns a record change and publishes it at the same stream position.
Each host applies the change once and updates its named list and count.
Server and browser allocate node identifiers in separate ranges. Each producer keeps both page roots stable.

## Wire format and tests

The action endpoint accepts an Ewe wire frame (EWF1) and a base64url client hello in the `x-ewe-hello` header.
It responds with `application/primal-frame`. A four-byte little-endian size precedes each response frame. The frame contains ordered messages and an acknowledgement.
The count stream at `/live` pushes a complete replacement to the count element after each successful mutation.
The count stream has identifier 1. A subscriber sends its last applied sequence through the negotiated resume protocol.
The server replays later count frames before it sends a new update.
All hosts deliver Todo data to the registered cache action. A confirmed local addition updates the notification element.

The Rust wire crate describes generated tables, size checks, fixtures and hand-written requests.
Use its fixture generator for byte-level compatibility tests. The browser test submits a modal editor and clicks a stale delete control.
See the [wire format and examples](../../backends/foundation_ui_wire/README.md) and the [browser test](tests/todo_browser_tests.rs).

```sh
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

The device tests start two local programs. Cargo builds the full `ui_todo` binary and the independent `todo_data_fixture` example.
Set `EWE_TODO_BIN` to reuse a full server binary. Set `EWE_TODO_DATA_BIN` to reuse a compiled data fixture.

To run Todo Server, start the example Rust host and open either server app.
The app reads `/initial` and sends mutations to `/action`.
The count subscribes to `/live?stream_id=2&resume_from=0`. A reconnect uses the last applied sequence.
Follow the linked app guide for the address used by each simulator.

To run Todo On Device, launch its own target on the device. Rust supplies the initial frame before the first tap.

## Glossary

- EWF1 identifies a version-one Ewe wire frame.
- HTTP means Hypertext Transfer Protocol.
- WebAssembly runs compiled Rust in the browser.

The initial page routes `/initial`, `/initial` and `/open-count` return one bare EWF1 frame with content type `application/vnd.ewe.frame`.

### Server setup

The binary binds the listener. The application builder installs its services and registers the routes on the router.
The initial routes return the page tree and its state. The response adapter signs state and writes the frame when the body is requested.
A successful mutation publishes a count to each live channel. The action endpoint validates and dispatches the request.
The test can supply its own web channel to disconnect a subscriber and check replay.

## Native todo registry

This crate builds the example's Rust action registry. Its `create_todo_core` function accepts a native platform and a transport. Swift and Kotlin use the generated bindings to call it.

The host handles `open_add_dialog`, `close_dialog`, `open_delete_dialog` and `filter_todos`. The server handles `add_todo`, `toggle_todo` and `confirm_delete`. The example signs presentation state. Business actions read the server's store and reject attached presentation state.

On-device apps call `create_local_todo_core` with a platform. Call `initial_todo_frame` once to mount its first screen.
The core holds the session cache. Opening a dialog, changing the filter, and cancelling work offline.
Adding, completing, reopening, and deleting a record require a connection to the server.

The compiler links the domain actions and native core into one library. Swift and Kotlin refer to that same library for callback handles and Rust buffers. The independent starter uses the package's remote factory.

The transport sends one EWF1 frame per request. Strip the four-byte HTTP length before returning a response to the core. The renderer returns each trigger's signed bytes unchanged. See the wire crate README for hand-written frames and fixture generation.

## Build outputs from one package

The shared directory `src/shared` contains the store, actions, and views.
The package selects one entry point for each host.

| Host | Source | Build command from the repository root | Artifact |
|---|---|---|---|
| Server | `src/bin/server.rs` | `cargo build --profile uat -p ui_todo --bin ui_todo` | `target/uat/ui_todo` |
| Browser | `src/web.rs` | `cargo build -p ui_todo` | `public/ui_todo/module.wasm` |
| iOS | `src/mobile.rs` | `native-task task run native:todo:prepare-ios` | `target/native-ui/ios-todo/.build-inputs/EweCoreFFI.xcframework` |
| Android | `src/mobile.rs` | `native-task task run native:build-android --with-todo` | Each example gets `app/src/main/jniLibs/<abi>/libui_todo.so`. |

The server is the default Cargo feature. The browser script selects `web`. The mobile scripts select `mobile`.
Swift imports `EweTodoCore`. Kotlin imports the actions from `io.github.ewe_studios.ewe.todo`.

## Ordinary data and UI actions over Connect

Start the independent data fixture with `cargo run --profile uat -p ui_todo --example todo_data_fixture -- 127.0.0.1:0`.
The generated Todos client reads task 42 and returns its identifier and title. Task 999 returns NotFound.
The native integration suites run this procedure in a separate process from the screen test.

The normal server also exposes the generated Actions service. Open `/?rpc` to use it from the browser.
Run the native tests with `TEST_RUNNER_EWE_TODO_RPC=1` on iOS or `EWE_TODO_RPC=true` on Android to use it for UI actions.
[Typed Swift and Kotlin examples](../../backends/foundation_ui_connectrpc/README.md)

## Share a WebSocket

Open `/?socket` to use the binary action endpoint at `/socket`.
The browser loads the first screen over HTTP. Opening the form runs local Rust. Submitting the form sends one binary action request.
The screen receives its reply on the socket. The live count also updates on the other open screen.

The server assigns a delivery session to each screen. A session identifies the destination for an action reply.
Each live subscription has a separate identifier and a cursor. The cursor records the last complete update that the subscriber applied.
On a shared WebSocket, a subscriber sends its resume request on the same socket.
On a shared event stream, it posts the binary resume request to the stream endpoint. The reply arrives on the open event stream.
The server checks the delivery session and its allowed streams. A public application can use the default delivery principal.
An authenticated application installs `DeliveryPrincipal` in each request context.

Run the transport test from the repository root. It opens two screens, sends an action, and checks directed replay over both transports.

```sh
cargo build --profile uat -p ui_todo --features server --bin ui_todo
node --test examples/ui_todo/tests/shared-connections.test.mjs
```

Streamed HTTP also accepts a separate resume request. The original response delivers length-prefixed frames.
The native integration tests request an expired cursor over SSE, streamed HTTP, and WebSocket. Each checks the subscription identifier.


All three platforms request the full todo view at `/initial`. The shared markup names its footer `open-count`.
A mutation publishes one replacement on `/live`. Each subscriber applies it to `#open-count` in its receiving view.
The starter requests `/open-count`. Open `/?count` in the browser to see the same screen and use its Refresh button.

### Recover the count after replay expires

The replay fixture accepts the number of retained updates as an argument.
The store captures the view and the feed cursor under the same lock that protects publication.
The handler releases the lock before it renders the response.
A client with an expired cursor loads `/initial` again and resumes each stream from the returned cursor.

The integration fixture starts the todo server with one retained update.

```sh
cargo build --profile uat -p ui_todo --example todo_replay_fixture
node examples/ui_todo/tests/recovery-proxy.mjs 57634 1
```

Set `EWE_TEST_EXPIRE_HTTP=1` on the proxy to test an HTTP 410 resume response.
Without that setting, the real stream service sends its typed expiry control over the open feed.
Seed one todo through the proxy. Open the full view on all three clients.
Run the race from another terminal.

```sh
node examples/ui_todo/tests/recovery-race.mjs http://127.0.0.1:57634/
```

The fixture disconnects the three feeds and adds three todos. It holds all three recovery snapshots through revision four.
It adds a fifth todo before sending those snapshots. Each client's count reaches five after replay.

### Verify list changes from a browser peer

Build the server, browser module, and iPhone package with the commands above.
Run `node examples/ui_todo/tests/mobile-list-peer.mjs` from the repository root.
Use the two printed addresses as `TEST_RUNNER_EWE_TODO_URL` and `TEST_RUNNER_EWE_TODO_PEER_TEST_URL` for the TodoServer test scheme.
Select `TodoServerUITests/TodoTests/testBrowserPeerUpdatesRowsWithoutReplacingFilterOrDraft`.
The driver starts a separate server. It adds a row, completes it, and deletes it while the iPhone stream is disconnected.

To add the Android peer, install Todo Server on an Android 16 Automated Test Device emulator.
Set `EWE_TODO_PEER_ADB` to the Android Debug Bridge executable before you start the driver.
Set `EWE_TODO_PEER_ANDROID_SERIAL` if the emulator uses a serial other than `emulator-5554`.
Set `TEST_RUNNER_EWE_TODO_PEER_ANDROID=1` for the iPhone test.
Each client adds, completes, reopens, and deletes one todo. The other two clients check the row after each change.

### Store and import Todo records

The current backup contains an array of records with `id`, `text`, and `done` fields.
Each identifier is a unique version 4 universally unique identifier (UUID).
The server opens `todo.sqlite3` in its working directory by default.
A new database contains no items and starts at stream position zero.
Set `EWE_TODO_STORE` or pass `--store PATH` to use another database.
One server owns the file while it publishes changes.
Restarting the server restores confirmed records, deleted identifiers, and the last stream position.
Older subscribers request a new snapshot when their cursor predates the restored history.

Import a backup once into a new or empty database. Start the server with that database.

```sh
todo-maintenance import current.json todo.sqlite3
ui_todo 127.0.0.1:8180 --store todo.sqlite3
```

Convert a numeric backup once before restoring it.
Use new paths for both the converted backup and the private identifier map.

```sh
umask 077
todo-maintenance convert old.json current.json > id-map.tsv
```

Conversion preserves record order, titles, completion states, and valid creation identifiers.
It generates an identifier for each record without a creation identifier.
It refuses duplicate identifiers and existing output files. Both files must fit the server's frame size limit.
The original backup stays available.
The map pairs each old numeric identifier with its current UUID.
Use that map to identify the target of an old pending delete before resubmitting it.
If the map was lost, pair records from these same two files by position.

## Application tasks

Run these commands from this directory. The application manifest declares its Rust task package under `tools/tasks`.

```sh
ewe_ui task list
ewe_ui task run todo:prepare-ios --help
ewe_ui task run todo:sync-android --check
ewe_ui task run todo:test-ios --help
ewe_ui task run todo:test-android --help
```

### Play Mathalley

Select **Play Mathalley** in a Todo screen. Tap the title to start. Select an answer in Train or Gumball.
Select **Sound** to mute the game. Select **Back to Todos** to return to the same list and filter.

The browser, iPhone and Android hosts register a separate game view. The Todo view keeps its live subscription.
The host lends the game a drawing surface. Closing the game releases that surface and its sound session.
The host keeps the graphics device for the next view. Backgrounding the app pauses the game clock and sound.

For Chrome, start `cargo run --profile uat -p ui_todo --bin ui_todo`, and open the address it prints.
For an iPhone simulator, run `ewe_ui task run todo:prepare-ios`. Open either Todo Xcode project and run its app.
For Android, run `ewe_ui task run todo:sync-android`. Build either Todo Android project against the matching local native package.

For a repeatable Android game check, install the on-device app and run the following command.
The test uses ImageMagick to check the presented scene. It saves the title, Train, and Gumball images.

```sh
ewe_ui task run todo:test-android-game --adb "$ANDROID_HOME/platform-tools/adb" --device "$DEVICE" --captures "$CAPTURES"
```

In Xcode, run `TodoTests.testTrainAndGumballSurviveRotation` in the on-device project.
Both checks replay seed 13 and rotate the Gumball scene before returning to Todos.

## Tauri desktop check

Run the same Todo page in an optional desktop window. Follow the
[Tauri build prerequisites](https://v2.tauri.app/start/prerequisites/) for your operating system.
In a clean shell at the repository root, start a Todo service with a temporary database.

```sh
todo_check_dir=$(mktemp -d)
cargo run --locked --profile uat -p ui_todo --bin ui_todo -- \
  127.0.0.1:0 --store "$todo_check_dir/todo.sqlite3"
```

In a second shell at the repository root, run the desktop example. Replace the address
below with the address printed by the server.

```sh
cargo run --locked --manifest-path examples/ui_todo/src-tauri/Cargo.toml -- \
  http://127.0.0.1:57841/
```

The launch command requires the Todo service address.
The Todo service supplies the page, browser scripts, and WebAssembly files. Keep it
running while you use the desktop window. The shell registers no native commands
or plugins for the page.
