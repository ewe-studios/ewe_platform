# Ewe UI for Swift

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
.package(url: "https://github.com/ewe-studios/ewe_platform.git", exact: "0.1.0")
```

Add the `EweUI` product to your iOS application. SwiftPM checks the archive checksum and links the native library. The example uses a local release candidate until the public tag is available.

```swift
import EweUI
import SwiftUI

struct Home: View {
    let server: URL

    var body: some View {
        ServerScreen(base: server, initial: "native-starter", actions: ["refresh_count"])
    }
}
```

The starter uses the server registry for all business actions. Open `examples/mobile_starter/ios/Starter.xcodeproj` for a complete application. Its package reference points to version 0.1.0.

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

The Rust reference model creates binary fixtures and their expected JSON snapshots. They cover row and column encoding, replacement, mixed platform operations, behaviours, presentation roles and an unknown node kind. Some fixtures extend the initial tree. The Swift tests apply each fixture in the same order and compare the complete snapshots.

Change the Rust fixture builder first. Run the reference tests with `UPDATE_FIXTURES=1`. Run the synchronization script from the platform checkout to refresh generated inputs. The script updates the package's generated sources and fixtures.

```sh
cargo test -p foundation_ui_wire --profile uat --features reference
scripts/ui_native/sync-ios.sh --check
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

Use the `PRESENTATION` text property to select a native control. Frames without this property use ordinary SwiftUI controls.

| Role | Control |
|---|---|
| `navigation-title`, `toolbar-action` | The navigation title and its toolbar buttons. |
| `segmented-control` | A picker. The selected child button has its checked property set. |
| `inset-list`, `checklist-row`, `checkmark-action` | An inset list with a completion button for each row. |
| `destructive-action`, `confirmation-dialog` | A swipe action and its confirmation alert. |
| `form-sheet`, `sheet-title` | A sheet with a title and a text field. |
| `cancel-action`, `submit-action` | The sheet buttons. A submit button sends its tap action with the entered text. If unbound, it sends the single field's submit action. |
| `list-footer` | The list footer. |

## Run the todo apps

Install Xcode with an iOS simulator runtime. Install the Rust simulator target for the Mac.

```sh
rustup target add aarch64-apple-ios      # Physical iPhone
rustup target add aarch64-apple-ios-sim  # Apple silicon simulator
rustup target add x86_64-apple-ios      # Intel
```

From the repository root, run `scripts/ui_native/prepare-ios-todo.sh`.
The script builds a device and simulator package with the reusable renderer and the todo bridge.
The bridge links to the same native core in one archive.

- [Todo Server](../../examples/ui_todo/ios/todo_server/README.md) uses a loopback server.
- [Todo On Device](../../examples/ui_todo/ios/todo_on_device/README.md) keeps the Rust task store in the app.
- [Starter](../../examples/mobile_starter/ios/README.md) imports only `EweUI`.

Build and test the library at the repository root.

```sh
scripts/ui_native/build-ios.sh
mkdir -p .build-inputs
unzip -o target/native-ui/apple/EweCoreFFI.xcframework.zip -d .build-inputs
swift test
```

The local archive supplies the native core before the first public package release.
