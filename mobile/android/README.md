# Ewe UI for Kotlin and Android

Ewe UI reads a binary frame and displays its nodes with Jetpack Compose. An event on a control can call a local Rust action or a remote action.

| Module | Maven artifact | Function |
|---|---|---|
| `core` | `io.github.ewe-studios:ewe-ui-core` | Kotlin reader, node store, action bridge and live mount lifecycle. |
| `binding` | `io.github.ewe-studios:ewe-ui-binding` | Generated UniFFI bridge to Rust. Its Android archive includes ARM64 and x86-64 libraries. |
| `ui` | `io.github.ewe-studios:ewe-ui` | Compose renderer, Android commands and network channels. It depends on both other modules. |

Use one Gradle dependency after the release is available. Set `mavenCentral()` in the application repositories.

```kotlin
dependencies {
    implementation("io.github.ewe-studios:ewe-ui:0.1.0")
}
```

For a local build, use the staging commands below and add the staging repository to the application repositories.

## Display a screen

`RemoteEweScreen` negotiates with the selected endpoint and requests an initial frame.
Supply the first screen path and an application core factory. This example uses the generic Rust core.

```kotlin
import ewe.ui.compose.RemoteEweScreen
import io.github.ewe_studios.ewe.core.NativeCore
import io.github.ewe_studios.ewe.core.NativePlatform
import okhttp3.HttpUrl.Companion.toHttpUrl

setContent {
    MaterialTheme {
        RemoteEweScreen(
            endpoint = "http://10.0.2.2:8180/".toHttpUrl(),
            initialPath = "native-starter",
            createCore = { transport ->
                NativeCore.remote(NativePlatform.ANDROID, listOf("refresh_count"), transport)
            },
        )
    }
}
```

`LocalEweScreen` takes a factory with no transport and a function that returns the first frame.
The application registers its local actions in the Rust core. The [on-device todo](../../examples/ui_todo/android/todo_on_device/) supplies both functions.
Each host owns its core and store. The host closes the core and subscriptions when the screen leaves the composition.

Applications that manage their own store and bridge can use the lower-level renderer.

```kotlin
import ewe.ui.compose.EweScreen

setContent { MaterialTheme { EweScreen(store, bridge) } }
```

The renderer supports stacks, text, images, buttons, text input, toggles, lists, scroll containers, spacers and overlays.
Supply composable image and web-view renderers with `EweScreen`.
Presentation properties select native styles for titles, toolbar actions, filters, task rows and form sheets.

## Containers

An `Overlay` draws its children above the current screen or modal. Controls outside those children still receive input.
A `Modal` opens a native modal window. Android directs focus and input to its children.
A `BottomSheet` measures its panel from the supplied children. At the end of a vertical stack, it uses the lower edge.
The preceding content receives the remaining height. The application supplies no placeholder or fixed height.

```rust
html! { <Overlay><BottomSheet><Text>Map legend</Text></BottomSheet></Overlay> }
```

This panel covers the bottom of the screen. The user can still interact with the map above it.

```rust
html! { <Modal><BottomSheet><Text>New todo</Text></BottomSheet></Modal> }
```

This composition opens one Material modal bottom sheet. Its controls retain their original action bindings.
Provide a button with the cancel action for system dismissal.

## Wire format and actions

A frame is a FlatBuffers buffer with the identifier `EWF1` at bytes 4 through 7.
Its header contains the schema version, stream identifier and sequence. A message contains operations, data or control information.
HTTP responses use a four-byte length in little-endian order before each frame. A WebSocket binary message carries a bare frame.
Server-sent events carry a base64 frame in their data field. The channel factory passes complete frames to the store.

Shared operations create, remove, move and replace nodes. They set text and typed properties, bind actions, attach signed state and mount live channels.
The Android reader also accepts mobile commands, behaviour names and web-view mount records. Applications supply the web-view control.
The system command host supports haptic feedback, opening a web URL and sharing text.

A tap, change or submit reads the reference from that control's action map.
The bridge copies the signed state attached to that control. Its immediate parent identifier describes the tree.
A local Rust action can return an immediate frame. A remote action can return a frame or send its update through a live channel.

## Write a frame by hand

The generated Kotlin tables and the runtime used by the reader ship in the core artifact.
The following example writes text to screen root 1.

```kotlin
import ewe.ui.*
import ewe.ui.flatbuffers.FlatBufferBuilder

val b = FlatBufferBuilder()
val text = SetText.createSetText(b, 1u, b.createString("Ready"))
val row = Operation.createOperation(b, UiOperation.SetText, text, 0u, 0, 0u, 0, 0)
val message = Message.createMessage(b, row, 0, 0)
val messages = FrameMessage.createMessagesVector(b, intArrayOf(message))
val body = FrameMessage.createFrameMessage(b, 0u, messages)
val frame = Frame.createFrame(b, 1u, 9u, 1u, FrameKind.FrameMessage, body)
Frame.finishFrameBuffer(b, frame)
check(store.receive(b.sizedByteArray()))
check(store.record(1u)?.text == "Ready")
```

The test suite builds frames this way for event ownership, gap recovery, commands, caps and live mounts.
Conformance fixtures pair a Rust-generated `.bin` file with a `.expected.json` snapshot. A mixed, replacement or behaviour frame follows the tree fixture.
Create or change fixtures in the monorepo reference test. Run the sync script to copy the results here.

## Build and verify

Run these commands from the repository root. The toolchain task installs the pinned Android dependencies.

```sh
mise run toolchain:android
scripts/ui_native/sync-android.sh --check
scripts/ui_native/build-android.sh
mise exec -- mobile/android/gradlew -p mobile/android :core:test :ui:testDebugUnitTest
mise exec -- mobile/android/gradlew -p mobile/android publishToMavenLocal \
  -Dmaven.repo.local="$PWD/target/native-ui/maven"
```

The [package-only starter](../../examples/mobile_starter/android/) builds with the staged or released dependency.
The [todo apps](../../examples/ui_todo/android/) use the same renderer with different task stores.

The reader uses FlatBuffers 25.9.23. The published Java runtime ends at 25.2.10 on Maven Central.
The core includes the upstream Java source and license from commit `187240970746d00bbd26b0f5873ed54d2477f9f3`.
The build relocates runtime sources and generated imports into `ewe.ui.flatbuffers`. This permits a consumer to load both runtimes.
The source manifest records the SHA-256 digest of each upstream file.

The pinned tools are Gradle 9.9.1, Kotlin 2.4.21 and Java 25. Compose builds with Android platform 37.2. The minimum runtime API is 26.

[Core API](core/docs/index.html) · [Binding API](binding/docs/index.html) · [Renderer API](ui/docs/index.html)

## Glossary

- An application programming interface (API) defines the calls that a package accepts.
- A node is a record for one component on the screen.
- A frame is a complete binary message for a renderer or action host.
- A fixture is a saved test input and its expected output.
- A cursor is the last sequence number that a store applied to one stream.
