# How the Todo Server application starts

## Terms

| Term | Meaning |
|---|---|
| Entry point | The part of the application that the system starts first. |
| Activity | An Android component that contains a screen. |
| Host | The part of the application that connects a screen to Rust. |
| Action | A named function that handles an event, such as a button press. |

## Part 1. What opens when I tap Todo Server?

### Android

The manifest names `.MainActivity`. The launcher uses the `MAIN` action and the `LAUNCHER` category to find this activity.

```xml
<activity android:name=".MainActivity" android:exported="true">
    <intent-filter>
        <action android:name="android.intent.action.MAIN"/>
        <category android:name="android.intent.category.LAUNCHER"/>
    </intent-filter>
</activity>
```

Android starts the activity and calls `MainActivity.onCreate`. The activity uses `setContent` to select the first screen.
Inside `MaterialTheme`, the application makes this call.

```kotlin
RemoteEweScreen(endpoint, "native-initial", { createTodoCore(NativePlatform.ANDROID, it) })
```

The first argument gives the server address. The second names the route for the first view.
The last argument gives the screen a function that creates its Rust core. [Android manifest](../../../examples/ui_todo/android/todo_server/app/src/main/AndroidManifest.xml) · [Kotlin activity](../../../examples/ui_todo/android/todo_server/app/src/main/kotlin/ewe/todo/MainActivity.kt)

### iOS

Swift marks the application entry point with `@main`. The declaration begins with this line.

```swift
@main struct TodoServerApp: App {
```

The application describes its window with `WindowGroup`. The window contains the following screen when the server address is valid.

```swift
ServerScreen(base: url, initial: "native-initial") { transport, store in
  let core = try createTodoCore(platform: .ios, transport: transport)
  return NativeActionCore(core, store: store)
}
```

Both applications request the first view from `native-initial`. Each gives its screen a function that creates the Rust core. [Swift application](../../../examples/ui_todo/ios/todo_server/App/App.swift)

## Earlier questions

**Can two views in one application use different hosts?**

Yes. One view can use on-device Rust actions. A second view can use its own server connection.
Each screen owns its node store. The examples demonstrate the two setups in separate applications. [Android hosts](../../../mobile/android/ui/src/main/kotlin/ewe/ui/compose/EweHost.kt) · [Swift local host](../../../mobile/ios/Sources/EweUI/LocalScreen.swift) · [Swift server host](../../../mobile/ios/Sources/EweUI/ServerScreen.swift)

**Can one view use both local and remote actions?**

Yes. In Todo Server, `host:open_add_dialog` opens the form on the device.
Submitting its title calls `server:add_todo` on the server. [Todo actions](../../../examples/ui_todo/src/lib.rs)

**What does “service” mean here?**

A service is a remote destination with its own connection settings.
For example, two accounts can use different credentials on the same server.
The native core takes one optional action transport. A transport carries a request to its destination. [Core construction](../../../examples/ui_todo/native/src/lib.rs)

**How do register and declare differ?**

The device registers a local handler to run the function.
It declares a server action to identify a function that the server handles.
The server registers the handler that runs there. [Core setup](../../../examples/ui_todo/native/src/lib.rs) · [Action registries](../../../examples/ui_todo/src/lib.rs)

**Why does the example take a `web: bool` flag?**

The example uses separate control trees for the browser and mobile screens. The flag selects a tree.
The browser tree includes the live count and browser form submission. The mobile tree uses checklist and form presentation roles.
The shared presentation model can describe one row. Removing the flag also requires preserving the browser form and live count behavior. [Todo views](../../../examples/ui_todo/src/lib.rs)

## Reading order after Part 1

Continue when the reader asks for the next part.

1. Follow the screen’s call into Rust. Locate the local handler registrations and the remote declarations.
2. Trace the native bindings. Show the build commands and the library location for each mobile application.
3. Map the Rust crates and their source directories. Explain the roles of `native/`, `module/`, and `src/`.
4. Follow one action through `src/`, then through the browser and native code.
5. Return to Android and iOS. Trace how each screen receives a reply and displays the change.

### Can one Android activity show multiple screen types?

Yes. The activity calls `setContent` once. Inside `MaterialTheme`, the application can select a screen when the user selects a tab.
For example, the Drafts tab can use `LocalEweScreen`. The Team tasks tab can use `RemoteEweScreen`.
Both inherit the same theme. Two different pages can also use the same screen function with different initial routes.

### What differs between createTodoCore and createLocalTodoCore?

Both functions create the same Rust type, `NativeCore`, on the device. The todo example defines both functions.
With `createTodoCore`, opening the form runs locally. Submitting the title sends `server:add_todo` through the supplied connection.
With `createLocalTodoCore`, opening the form also runs locally. Submitting the title runs `host:add_local_todo` against the on-device task store.
The local constructor supplies no network transport.

### How does Swift exchange data with local Rust?

Swift and Rust run in the same application process. Generated Swift bindings call the linked Rust library.
On a tap, the Swift event bridge sends the action name and event data to the Rust core.
Rust runs the local handler. Its return value can contain a frame of screen updates.
The Swift bridge applies that frame to the node store. SwiftUI displays the updated controls.
The optional transport supplies a network connection for server actions.

### Does Android use the same local call?

Yes. Kotlin uses generated bindings to call Rust inside the Android application process.
For example, the event bridge sends `host:add_local_todo` and the entered title to the core.
Rust returns an optional frame as a Kotlin `ByteArray`. The bridge applies the frame to the store, and Compose displays the updated list.

### Which transports do the mobile packages provide?

A local action calls Rust inside the application. A remote action uses a Hypertext Transfer Protocol (HTTP) POST request by default.
For live updates, both mobile packages can receive server-sent events (SSE), binary WebSocket messages, or an HTTP stream with length-prefixed frames.
A local live mount opens no network connection. The Rust core accepts a custom action transport through `HostTransport`.
The supplied WebSocket channel receives updates. The supplied remote action transport uses HTTP.

### Does each read from a live HTTP response contain one frame?

The client opens one request. The server can keep the response open and send several frames over time.
Each frame starts with a four-byte length in little-endian order. The length counts the following frame bytes.
A network read can contain part of a frame or bytes from several frames.
The client delivers each complete frame as soon as it has the declared number of bytes.

### Does the browser support the same live transports?

Yes. A browser live mount can receive server-sent events, WebSocket messages, or a binary HTTP response stream.
For a binary HTTP live mount, select `transport="chunked"` and `protocol="frame"`. The browser delivers complete length-prefixed frames during the response.

### Can a WebSocket send an action in an existing frame?

Yes. A FrameRequest carries an ActionRequest. The browser WebSocket action transport sends the encoded frame as one binary message.
The trigger completes when the socket accepts the send. A later FrameMessage supplies updates through the receive path.
The mobile libraries already read WebSocket updates. A built-in mobile sender can use the same action frame and return no immediate reply.
The server must accept binary requests on that socket and pass them to the action dispatcher. Reconnection must not replay a submitted action.

[Feature 25](../features/25-mobile-websocket-actions/feature.md) proposes built-in mobile WebSocket action delivery.

### Can actions reuse the existing WebSocket implementation?

Yes. The current live channel opens a socket and delivers incoming frames. Its public handle exposes a close operation.
The Rust core sends actions through a different interface called `HostTransport`.
A screen can own one socket session that implements both operations. One reader delivers incoming frames once.
A live mount can share that session when its endpoint and lifetime agree. Removing the mount must preserve the screen’s action connection.

### Connection sharing and resume

The connection manager owns connection lifetime. Each subscriber owns its applied cursor and requests replay through the application protocol. WebSocket can carry that request directly. SSE needs a separate upstream request because its event response flows from server to client.

### How does a live mount indicate its position today?

A frame mount sends its stream identifier and sequence position in the request query.
On web, set `data-channel-id="1"` and `data-resume-from="42"` to request stream 1 after frame 42.
The native wire declaration carries `channel_id` and `resume_from`. Both mobile adapters send the same query parameters.
The browser event reader also sends `Last-Event-ID` on reconnect when a server event supplies an identifier.

### Proposed connection sharing defaults

A default server-sent events (SSE) mount opens its own response. Set `data-connection-shared="true"` to join a live feed without requesting replay. WebSocket users share by default. Set `data-connection-shared="false"` to give one mount its own socket.
