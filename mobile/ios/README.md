# Ewe UI for Swift

Generate a standalone application in the current directory.

```sh
ewe_platform generate -t ios-app -p FieldNotes -l plain --app-id com.example.fieldnotes --host server
```

Use `--host on-device` to include a Rust counter. The generated guide gives the build and launch commands.

Ewe UI draws portable wire frames with SwiftUI. A frame contains the complete changes for one render. The Rust native core verifies local actions and sends remote actions through the application transport.

## Glossary

| Term | Meaning |
|---|---|
| UI | User interface. |
| EWF1 | The four-byte file identifier for a foundation UI frame. |
| FlatBuffers | The binary format used by the wire schema. |
| SwiftPM | Swift Package Manager. |
| HTTP | Hypertext Transfer Protocol. |
| SSE | Server-sent events. A connection delivers a sequence of events over HTTP. |
| FFI | Foreign function interface. It connects the Swift bindings to Rust. |
| Fixture | Checked-in input bytes with a known expected result. |

## Add a screen

Add this dependency to a Swift package. Xcode also accepts the same package address and version.

```swift
.package(url: "https://github.com/ewe-studios/ewe_platform.git", exact: "0.1.1")
```

Add the `EweUI` product to your iOS application. SwiftPM checks the archive checksum and links the native library. The example uses a local release candidate until the public tag is available.

```swift
import EweUI
import SwiftUI

struct Home: View {
    let server: URL

    var body: some View {
        ServerScreen(base: server, initial: "open-count", actions: ["refresh_count"])
    }
}
```

The starter uses the server registry for all business actions. Open `examples/mobile_starter/ios/Starter.xcodeproj` for a complete application. Its package reference points to version 0.1.1.

The server accepts the native hello at `POST /action` and returns its capabilities. The initial route returns one EWF1 frame. A tap sends the registered action name, control identifier, input value and attached state. The server returns its render instructions. Each HTTP action response starts with a four-byte little-endian frame length.

Use `URLActionTransport` with an application request factory to add credentials and select an endpoint. It sends one request per action and applies the request timeout.

## Wire messages and state

A `FrameMessage` contains operations, data and control messages. A `FrameRequest` contains a hello or an action. The screen root uses node identifier 1. The wire reserves identifier 2. Application nodes start above 2.

| Operation | Result |
|---|---|
| Create, append, insert, remove | Creates, orders or deletes records. Removing a record also removes its descendants. |
| Set text or property | Updates the corresponding record. SwiftUI observes each record separately. |
| Replace subtree or place | Removes descendants, swaps a target or inserts a complete render at the requested position. |
| Bind or unbind action | Registers or removes a reference for a named control event. |
| Attach state | Stores opaque bytes on the specified record. A control returns its own bytes with its next event. |
| Live mount | Starts its channel after attachment and closes the channel on removal. |
| Native command | Calls the command host for haptic feedback, sharing or opening a URL. |
| Mobile or shared behaviour | Applies a registered SwiftUI modifier. The store reports unsupported behaviours. |
| Web view mount | Records the URL and mounting parent for a platform host. |

Controls support tap, text change, submit and toggle events. Prefix an action with `server:` for business logic or `host:` for an action compiled into the native core. A packaged application that needs local domain actions supplies a core factory. The Todo Server example provides one.

A signer selects the state format. Swift keeps its bytes unchanged. A Rust component can attach readable signed JSON or sealed bytes. The renderer never searches a parent for state. The application decides whether a business action accepts signed component state.

Live mounts support SSE, length-prefixed HTTP streams and binary WebSocket messages. The stream identifier and last sequence form the resume cursor. The store drops duplicates, waits for a missing sequence, and asks the channel to resume after its gap timeout. Acknowledged action responses apply immediately.

## Write a frame by hand

Use the generated Swift wire tables in a package test. This example creates a complete frame that changes text on node 42. Create that node before applying this operation.

```swift
import EweWire
import FlatBuffers
import Foundation

var b = FlatBufferBuilder()
let string = b.create(string: "Ready")
let text = ewe_ui_SetText.createSetText(&b, nodeId: 42, textOffset: string)
let op = ewe_ui_Operation.createOperation(&b, uiType: .settext, uiOffset: text)
let row = ewe_ui_Message.createMessage(&b, operationOffset: op)
let rows = b.createVector(ofOffsets: [row])
let body = ewe_ui_FrameMessage.createFrameMessage(&b, messagesVectorOffset: rows)
let frame = ewe_ui_Frame.createFrame(
    &b, schemaVersion: 1, kindType: .framemessage, kindOffset: body)
ewe_ui_Frame.finish(&b, end: frame)
let bytes = Data(b.sizedByteArray)
```

Call `store.receive(bytes)` on the main actor. The call validates and applies the frame before it returns. Read `store.reports` for unsupported operations and invalid targets. Use `store.onData`, `store.onError`, and `store.commands` to connect application services.

## Components and motion

The screen supports stacks, text, images, buttons, text fields, toggles, lists, scroll views, spacers and overlays. Supply color and font token tables to `EweScreen`. A dialog uses a native sheet. The dismiss behaviour handles escape and a downward swipe. On iOS it also handles a tap outside the sheet. Scroll lock disables scrolling inside the marked subtree. A transition uses native animation and respects Reduce Motion.

Install a custom modifier before the first frame arrives.

```swift
store.behaviours.register("pull_to_refresh") { view, _ in
    AnyView(view.refreshable { await refresh() })
}
```

## Fixtures and checks

The Rust reference model creates binary fixtures and their expected JSON snapshots. They cover row and column encoding, replacement, mixed platform operations, behaviours, explicit controls and an unknown node kind. Some fixtures extend the initial tree. The Swift tests apply each fixture in the same order and compare the complete snapshots.

Change the Rust fixture builder first. Run the reference tests with `UPDATE_FIXTURES=1`. Run the synchronization script from the platform checkout to refresh generated inputs. The script updates the package's generated sources and fixtures.

```sh
cargo test -p foundation_ui_wire --profile uat --features reference
native-task task run native:bindings:sync-ios --check
swift test
```

`mobile/ios/Tests/EweUITests/RendererTests.swift` contains the hand-written frame builder. The tests check signed action bytes, ordering, local state ownership, reconnection and removal. The Swift package requires iOS 17 or macOS 14. Simulator builds require Xcode.

## Load a local frame

Use `LocalScreen` to build a screen with frames and actions that the application supplies.
The loader receives a node store. It returns an `ActionCore` and the first frame.
The action core receives the event and may return another frame.
The local screen does not open a channel or make a request.

```swift
LocalScreen { store in
    let session = try await startLocalSession()
    return (session.actions(store: store), session.firstFrame)
}
```

The application provides `startLocalSession`. Its action core can use Swift or an application-specific Rust bridge.
Use `EweScreen(store:bridge:)` when the application already owns the node store and action bridge.

## Present native controls

The Rust view declares each control by tag. SwiftUI maps each tag to its native control.
Use `NavigationTitle` and `ToolbarButton` in a screen header.
Use `SegmentedControl` for a picker. Its selected button has `checked={true}`.
An `InsetList` can contain a `ChecklistRow` with a `CheckmarkButton` and a `DestructiveButton`.
Use `ListFooter` for supporting text.

### Explicit containers

The Rust author supplies children to each container. The Swift package renders them in their declared order.
The wire carries separate node kinds for all three tags.

| Tag | Behavior |
|---|---|
| `Overlay` | Draw the children above the current screen or modal. Keep the underlying controls available outside those children. |
| `Modal` | Present one modal surface. Keep focus and input inside that surface. Dismissal uses the declared cancel action. |
| `BottomSheet` | Measure the child panel with the available size. Keep its bottom edge aligned in an overlay or modal. |

```rust
html! {
    <Overlay><BottomSheet><Text>Saved</Text></BottomSheet></Overlay>
}
```

This message appears over the screen. A user can still tap controls outside the panel.

```rust
html! {
    <Modal><BottomSheet>
        <SheetTitle>New todo</SheetTitle>
        <TextInput placeholder="Title" on:submit={add_todo}/>
        <CancelButton on:tap={close_dialog}>Cancel</CancelButton>
        <SubmitButton>Add</SubmitButton>
    </BottomSheet></Modal>
}
```

The input owns the submit action. The keyboard and Add submit its current text.
Cancel calls its own action. An explicit tap binding on Add takes priority.

```rust
html! {
    <ConfirmationDialog>
        <SheetTitle>Delete this todo?</SheetTitle>
        <CancelButton on:tap={close_dialog}>Cancel</CancelButton>
        <DestructiveButton on:tap={confirm_delete}>Delete</DestructiveButton>
    </ConfirmationDialog>
}
```

## Run the todo apps

Install Xcode with an iOS simulator runtime. Install the Rust simulator target for the Mac.

```sh
rustup target add aarch64-apple-ios      # Physical iPhone
rustup target add aarch64-apple-ios-sim  # Apple silicon simulator
rustup target add x86_64-apple-ios      # Intel
```

From the repository root, run `native-task task run native:todo:prepare-ios`.
The script builds a device and simulator package with the reusable renderer and the todo bridge.
The bridge links to the same native core in one archive.

- [Todo Server](../../examples/ui_todo/ios/todo_server/README.md) uses a loopback server.
- [Todo On Device](../../examples/ui_todo/ios/todo_on_device/README.md) keeps the Rust task store in the app.
- [Starter](../../examples/mobile_starter/ios/README.md) imports only `EweUI`.

Build and test the library at the repository root.

```sh
native-task task run native:build-ios
mkdir -p .build-inputs
unzip -o target/native-ui/apple/EweCoreFFI.xcframework.zip -d .build-inputs
swift test
```

The local archive supplies the native core before the first public package release.

## Style the public hosts

Build one theme in Rust. Use its stylesheet on the web. Enable the `native-manifest` feature of `foundation_theme`.
Pass the text from `theme.native_manifest()` to Swift through your application binding.

```swift
let theme = try EweTheme(manifest: Data(todoThemeManifest().utf8))
LocalScreen(theme: theme, load: load)
ServerScreen(base: url, initial: "initial", theme: theme, create: makeCore)
```

Omit `theme` for the system appearance. Changing this value redraws the active store.
Use `colors` and `fonts` in the initializer for an explicit numeric property.
The todo applications read the optional launch environment value `EWE_TODO_THEME=branded`.

```rust
html! { <Text class="text-primary p-md ios-p-lg android-p-sm">Buy milk</Text> }
```

The iPhone uses the primary color and large padding. Android selects small padding. The browser selects medium padding.
Each host reads the same named tokens. See the [utility mapping](../../backends/foundation_theme/README.md#shared-control-utilities).


A bottom sheet groups adjacent cancel and submit buttons into an action row.
Cancel comes first. Both controls retain their original node, state, and action.
When the labels need more space, the controls wrap onto separate rows.
The software keyboard leaves the action row visible.

## Call a typed Connect service

The Swift application can pass its own URLSession to a generated application client.

```swift
let client = NativeHttpClient(client: URLSessionRpcClient(session: session))
let item = try await getTodo(client: client, baseUrl: base.absoluteString, id: "00000000-0000-4000-8000-000000000042")
print(item.title)
```

The example method belongs to EweTodoCore. A missing identifier throws `TodoServiceError.NotFound`.
Pass `transport: .connect(base)` to `ServerScreen` to send actions through Connect.
[Service setup and stream limits](../../backends/foundation_ui_connectrpc/README.md)

### Submit typed input

A text control sends its own text through `bridge.send(.submit, node: fieldID, value: "Buy milk")`.
For a custom form, define the payload and pass it to the same event bridge.

```swift
struct AddItem: Encodable { let title: String; let list_id: UInt64 }
try bridge.sendStructured(.submit, node: fieldID, value: AddItem(title: "Buy milk", list_id: 7))
```

The bridge encodes the payload once and supplies the signed state attached to the field.
Rust receives `Interaction<S, AddItem>`. A remote host must negotiate vocabulary 9 or later.

## Send actions through a WebSocket

Supply the socket address to `ServerScreen`. The screen loads its initial view over HTTP.

```swift
ServerScreen(base: base, initial: "initial", transport: .webSocket(base.appendingPathComponent("socket"))) { transport, store in
    NativeActionCore(try createTodoCore(platform: .ios, transport: transport), store: store)
}
```

The example supplies `createTodoCore` from its application library. Its local action opens the form.
The socket sends one binary request for a remote action. A later binary reply updates the originating screen.
The screen negotiates capabilities again after a disconnect. The user can submit another action after negotiation.

Pass the same `ConnectionManager` through `connections:` to share connections between screens with the same credentials.
The last handle closes the connection. A retained connection closes when its owner releases retention or closes the manager.
Each event stream has its own connection by default. Set `data-connection-shared="true"` on a live mount to share it.
WebSocket mounts share by default. Set the attribute to `"false"` for a dedicated socket.
Use `data-connection-keep-alive="true"` to keep a connection open between listeners. A listener can request replay from its own cursor.

Run the package tests and the server example on an iPhone simulator.
Set `TEST_RUNNER_EWE_TODO_SOCKET=1` when running `native-task task run native:todo:test-ios <simulator-id>`.

Streamed HTTP also accepts a separate resume request. The original response delivers length-prefixed frames.
The native integration tests request an expired cursor over SSE, streamed HTTP, and WebSocket. Each checks the subscription identifier.

## Register a named view

```swift
let editor = try store.registerView("editor")
let accepted = editor.receive(frame)
try store.unregisterView("editor")
```

An append with no target adds a child to the receiving view. A full view replaces its content.
Use `view#editor` to select that root. Use `sheet#details` to check the shared element type before replacement.
A custom control can call `try editor.registerElement("details", tag: "InvoiceEditor", node: id)`.
Render the named view with its own store. Each store owns its node identifiers and stream positions.
Closing one view releases its listeners and children. Other views keep their content and positions.

## Recover a live view

`ServerScreen` installs the remote loader. A custom host can install its own loader.

```swift
let recovery = store.registerLoader(maxAttempts: 3) {
  try await service.initialView(using: account.currentCredentials)
}
try await recovery.open()
```

A full snapshot includes a cursor for each replayable stream. The store checks them before replacing visible content.
A failed load preserves the displayed view. Call `try await recovery.retry()` when the user requests another attempt.
Call `recovery.close()` when the view closes or the account changes.

## Local graphics

Call `try core.graphics()` to get the graphics service that local Rust actions can read.
Use the same call on a local screen or a server-backed screen. Compute jobs can run before a view exists.

```swift
let graphics = try core.graphics()
let values = try await graphics.compute(shader: shader, words: [0, 2, 7], groups: 3)
let target = try await graphics.attach(name: "preview", layer: metalLayer, width: 128, height: 64)
try await target.clear(red: 0, green: 0, blue: 1, alpha: 1)
try await target.remove()
```

A DrawingSurface control supplies its own Metal layer. The package retains each layer while it presents to it.
Updates use physical pixels. A zero dimension suspends presentation. Removing the control releases the target.
The host can call `try await graphics.reinitialize()` after device loss. Reattach active drawing targets afterward.
The package test runs the shared shader and samples two presented Metal textures. [Graphics test](Tests/EweUITests/GraphicsTests.swift)

## Lend a host-owned Metal device

Install a provider on the core with the local Rust action that accepts its results.
A core created by `NativeCore.remote` includes `host:foundation_gpu_ready`.
For a custom Rust core, register `GraphicsReady` or a local action that accepts the graphics offer.
The readiness action runs on the requesting core, including when the core has a server transport.

```swift
try core.installDeviceProvider(readyAction: "host:foundation_gpu_ready", provider: provider)
```

The Rust action starts a device request with a device name and an optional surface name.
The provider checks both names in `request(request:)` and schedules work on the host's resource owner.
Return from `request(request:)` before waiting for graphics work or calling the asynchronous reply.

```swift
// Run this on the resource owner after it reserves the selected device, queue, and texture.
let grant = try NativeDeviceGrant.metal(device: device, queue: queue, texture: preview)
try await request.ready(grant: grant, generation: generation, completion: completion)
```

Use a queue from the selected device. A compute-only request uses a `nil` texture.
A surface request uses an initialized, shader-readable, single-sample two-dimensional RGBA8 Metal texture with one mip level.
Keep the generation for the resource alive across leases. Increase it when the host replaces the resource.
Send `try await request.fail(reason: .denied)` if the host cannot reserve the requested resource.
Other refusal reasons are `.cancelled`, `.incompatible`, and `.deviceLost`.

The host finishes any prior writes from other queues before the grant.
The import waits for prior work on the loaned queue.
Reserve the queue and texture until `HostDeviceCompletion.released(deviceLost:)` runs.
If the lease is active and the surface closes, call `grant.withdraw()` and wait for the same completion callback.
Return the lease from Rust after the last job. Route the completion callback to the host's resource owner before reusing the texture.
The host presents the result after completion.
For a Metal layer, the host acquires the drawable, renders the returned preview into it, and presents it on the host queue.
If the callback reports device loss, replace the affected resources before submitting another grant.

The graphics tests check texture readback and presentation after the release callback. [Metal handoff test](Tests/EweUITests/HostGraphicsTests.swift)
