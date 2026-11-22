# Ewe UI for Kotlin and Android

Generate a standalone application in the current directory.

```sh
ewe_platform generate -t android-app -p FieldNotes -l plain --app-id com.example.fieldnotes --host server
```

Use `--host on-device` to include a Rust counter. The generated guide gives the build and launch commands.

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
            initialPath = "open-count",
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
Use NavigationTitle, ToolbarButton, SegmentedControl, InsetList, ChecklistRow, CheckmarkButton and ListFooter for shared list controls.

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

## Style the public hosts

Both hosts inherit Material colors and typography. Supply a theme to use Rust utility values and explicit node tokens.

```kotlin
val theme = EweTheme.fromManifest(todoThemeManifest())
LocalEweScreen(::makeLocalCore, ::firstFrame, theme = theme)
RemoteEweScreen(endpoint, "initial", ::makeServerCore, theme = theme)
```

Enable the `native-manifest` feature of `foundation_theme`. The application binding above returns `theme.native_manifest()`.
The same Rust theme creates the browser stylesheet. Omit `theme` for Material defaults.
Use `EweTheme(colors = colors, fonts = fonts)` for existing numeric node properties.
Changing the theme redraws the active store. The host keeps its connection and input.
Set the boolean activity extra `branded` to test the todo theme.

On Android 12 and later, pass `dynamicLightColorScheme(context)` or `dynamicDarkColorScheme(context)` to `MaterialTheme`.
Choose the scheme for the current appearance. Use your ordinary light or dark scheme on earlier versions.
See the [utility mapping](../../backends/foundation_theme/README.md#shared-control-utilities).


A bottom sheet groups adjacent cancel and submit buttons into an action row.
Cancel comes first. Both controls retain their original node, state, and action.
When the labels need more space, the controls wrap onto separate rows.
The software keyboard leaves the action row visible.

## Call a typed Connect service

The Kotlin application can pass its own OkHttp client to an application procedure.

```kotlin
NativeHttpClient(OkHttpRpcClient(okHttpClient)).use { network ->
    val item = getTodo(network, baseUrl, 42uL)
    println(item.title)
}
```

The example method belongs to the todo application binding. A missing identifier throws `TodoServiceException.NotFound`.
To send UI actions through Connect, pass `rpcBase` to `RemoteEweScreen`.
[Service setup and stream limits](../../backends/foundation_ui_connectrpc/README.md)

### Submit typed input

A text control calls `bridge.send(EventId.SUBMIT, fieldId, value = "Buy milk")`.
For a custom form, pass the value and its serializer to the event bridge.

```kotlin
data class AddItem(val title: String, val listId: Long)
bridge.sendStructured(EventId.SUBMIT, fieldId, AddItem("Buy milk", 7)) { item ->
    org.json.JSONObject().put("title", item.title).put("list_id", item.listId).toString()
}
```

The bridge keeps the state attached to this field. Rust decodes the payload as the declared action input.
A remote host must negotiate vocabulary 9 or later.

## Send actions through a WebSocket

Set the socket endpoint on the remote screen. The first view still loads over HTTP.

```kotlin
RemoteEweScreen(
    endpoint, "initial",
    createCore = { transport -> createTodoCore(NativePlatform.ANDROID, transport) },
    socketEndpoint = "/socket",
)
```

The application library supplies `createTodoCore`. Its local action opens the form. Submit sends one binary remote action.
A later socket message carries the reply for this screen. Reconnection negotiates capabilities again.
Pass the same `ConnectionManager` through `connections` to reuse a connection across screens with the same credentials.
Close the manager when its account signs out.

Event streams use dedicated connections by default. Set `data-connection-shared="true"` on a live mount to share its endpoint.
WebSocket mounts share by default. Use `"false"` for a dedicated socket.
A sender keeps the connection open when readers leave. The last handle closes it unless its owner selected retention.
Rust can request retention with `data-connection-keep-alive="true"`. Each listener tracks its cursor separately.

Run `mobile/android/gradlew -p mobile/android :core:test :ui:testDebugUnitTest` for the library checks.
Set `EWE_TODO_SOCKET=true` when running the Android todo verification script.

Streamed HTTP also accepts a separate resume request. The original response delivers length-prefixed frames.
The native integration tests request an expired cursor over SSE, streamed HTTP, and WebSocket. Each checks the subscription identifier.

## Register a named view

```kotlin
store.registerView("editor", 7u)
val editor = store.viewStore("editor")
val accepted = store.receive(frame, 7u)
store.unregisterView("editor")
```

An append with no target adds a child to the receiving view. A full view replaces its content.
Use `view#editor` to select that root. Use `sheet#details` to check the shared element type before replacement.
A custom control can call `editor?.registerElement("details", "InvoiceEditor", id)`.
Render the named view with its own store. Each store owns its node identifiers and stream positions.
Closing one view releases its listeners and children. Other views keep their content and positions.

## Recover a live view

`RemoteEweScreen` installs the remote loader. A custom host can install its own loader.

```kotlin
val recovery = store.registerLoader(screenScope, maxAttempts = 3) {
    service.initialView(account.currentCredentials())
}
recovery.open()
```

A full snapshot includes a cursor for each replayable stream. The store checks them before replacing visible content.
A failed load preserves the displayed view. Call `recovery.retry()` when the user requests another attempt.
Close the store when the screen closes or the account changes.

## Local graphics

Call `core.graphics()` on a local or server-backed screen. Local Rust actions read the same service.
The service runs compute jobs before a view exists. The DrawingSurface control manages its own Android SurfaceView.
An application can also attach a Surface that it already owns.

```kotlin
import ewe.ui.android.attach

val graphics = core.graphics()
val values = graphics.compute(shader, listOf(0u, 2u, 7u), 3u)
val target = graphics.attach("preview", surface, 128u, 64u)
target.clear(0.0, 0.0, 1.0, 1.0)
target.remove()
target.close()
```

Call `resize` with physical pixels. A zero dimension suspends drawing. Call `remove` when the native window is destroyed.
After device loss, call `graphics.reinitialize()` and attach the active windows again.
The device test uses PixelCopy to verify both presented targets. [Graphics test](ui/src/androidTest/kotlin/ewe/ui/compose/GraphicsTest.kt)

### Lend a Vulkan device

Use the [Vulkan bridge header](binding/src/main/assets/include/ewe_graphics_vulkan.h) in your native development kit (NDK) code.
Pass your Vulkan instance, physical device, application programming interface (API) version, and enabled instance extensions to `ewe_ui_gpu_vulkan_plan`.
The instance API must cover the adapter API up to Vulkan 1.3. Use the version from the instance creation record.
Use `ewe_ui_gpu_vulkan_requirements` to read the required logical device features and extensions before you create a device.
For an existing device, retain its actual creation records to check that it meets these requirements.
Supply the separate Vulkan feature structures returned by the plan in the device creation chain.
The bridge rejects aggregate Vulkan 1.1, 1.2, or 1.3 structures when they replace a required separate structure.
See [issue 691](https://github.com/ewe-studios/ewe_platform/issues/691) for aggregate feature support.

Register a local Rust action on the core that accepts `GraphicsReady`.
Install the Kotlin provider with that action reference.
The provider schedules work off the caller thread and returns immediately.
Make each grant on the native worker that owns the queue.

```kotlin
core.installDeviceProvider("host:graphics_ready", object : HostDeviceProvider {
    override fun request(request: NativeDeviceRequest) {
        graphicsScope.launch {
            request.use {
                val grant = host.acquireVulkanGrant(it.device(), it.surface())
                if (grant == null) {
                    it.fail(HostDeviceFailure.DENIED)
                    return@use
                }
                FfiConverterTypeNativeDeviceGrant.lift(grant.handle).use { nativeGrant ->
                    it.ready(nativeGrant, grant.generation, object : HostDeviceCompletion {
                        override fun released(deviceLost: Boolean) {
                            host.scheduleReturn(grant, deviceLost)
                        }
                    })
                }
            }
        }
    }
})
```

`acquireVulkanGrant` and `scheduleReturn` are application functions. Keep the provider scope active while work is pending.
Supply the actual queue family and index from the logical device creation record.
The bridge resolves that queue on the host device. The optional image must belong to that device.
Pass a null surface name for compute only. For drawing, lend a two-dimensional `VK_FORMAT_R8G8B8A8_UNORM` image.
The image needs one mip level, one layer, one sample, and `SAMPLED` and `TRANSFER_DST` usage.
Initialize the image and transition it to `SHADER_READ_ONLY_OPTIMAL` on the selected queue family.
Stop host submissions on the queue before making a grant. The bridge waits for prior queue work.
Keep the instance, device, queue, and image alive until the completion callback.
The callback can run on a Rust worker. Resume queue use on the native worker after the callback.
The host owns presentation. The lease returns the image in `SHADER_READ_ONLY_OPTIMAL`.

Use a larger generation number after you replace a device or surface.
Keep the grant reference through the completion callback if the host can withdraw the image.
Call `nativeGrant.withdraw()` to stop new work. Wait for the completion callback before you remove the image.
Report an incompatible native record with `request.fail(HostDeviceFailure.INCOMPATIBLE)`.
After a device loss, finish cleanup and create a device with a new generation.
See the [Android Vulkan handoff test](ui/src/androidTest/kotlin/ewe/ui/compose/HostGraphicsTest.kt) for a complete provider and native host.
