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

### Can the host supply the network connection for ConnectRPC?

Rust can already accept a transport for a typed remote procedure call. [Feature 26](../features/26-connectrpc-transport/feature.md) proposes adapters for mobile hosts and WebAssembly.

### Can one Rust package serve all three targets?

The main package owns the todo state, views, and action registries. Both adapter packages import it with the server feature disabled.
The mobile adapter exports constructors to Swift and Kotlin. The browser adapter exports the WebAssembly startup function.
One package can use target-specific modules for both entry points. That change also requires updates to build scripts and binding generation.

### Can each platform use a binary target?

Cargo treats each binary target as an executable. The mobile apps link a library. The browser loads a WebAssembly module.
One package can keep the mobile and browser exports in library modules. Its server can use a binary target.
The build command enables package features. A binary's `required-features` setting checks that those features are active.
The local mobile constructor can stay in the same library as the remote constructor.
See the [Cargo target reference](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#the-crate-type-field).

### Where does the shared todo code live?

The proposed layout puts todo state, views, and actions in `src/shared/`.
The library root always includes the shared module. The `mobile` and `web` features select their export modules and optional dependencies.
The `server` feature enables the server dependencies and permits the server binary to build.
The build target selects the platform. The library crate types specify its output formats.
Both mobile constructors belong to the mobile module. Each screen chooses its constructor at runtime.

[Feature 27](../features/27-single-todo-package/feature.md) records this layout.

### Which todo actions run on the phone?

The server-connected core registers the dialog functions on the phone. It declares the add, toggle, and delete operations as server actions.
The on-device core also registers local write actions. It installs a todo store in its context.

### Who owns the browser dispatcher?

The proposed LocalApp builder accepts the prepared context and registry. It builds the local dispatcher and installs it in the browser bridge.
The bridge keeps the dispatcher alive after the startup function returns.

### Can Rust also be the network client?

A mobile screen supplies a native sender. A Rust client can supply its own action transport. Both use the same action routing rules.

### Can Rust ask the native app for a transport?

Today, Swift or Kotlin passes an action sender to Rust. The native library adapts that sender for the Rust action manager.
A proposed alternative lets native code supply a provider during setup. Rust asks that provider for a named service.
For a ready action sender, Rust adapts the returned handle. For Connect, Rust can request a network client and build the protocol adapter.
Both paths give the builder a Rust action transport. The provider can keep account credentials in the platform network client.

### Follow the toolbar Add tap

The toolbar button binds its tap to open_add_dialog. Its native event bridge reads the action and signed state from that button.
The bridge calls the Rust core. The local handler sets add_dialog and renders the form. The bridge applies the reply to the screen.

### How does the Add dialog become a native form?

Rust targets the dialog root and renders an overlay with the form-sheet role. The reply carries the controls and their roles.
The iPhone renderer matches that overlay to FormPresentation. SwiftUI presents a sheet with a title, text field, and toolbar buttons.
The submit button sends the field value through the input's submit action. Android maps the same overlay to a Material bottom sheet.

### Why does the Add dialog check the surface?

The web example binds submit to a form. The mobile example binds submit to the input and gives the button a submit role.
Both native renderers implement that role. A shared form needs the browser renderer to send the input value once on click or keyboard submission.
The on-device flag selects the action destination independently of the surface.

### Does a dialog need a reserved node?

Rust creates node 41 in the first todo frame. Both native renderers apply that frame. Opening and closing the dialog replace the same node.
An action can instead append an overlay to an existing parent. Closing it then requires a way to identify and remove that overlay.

### How do presentation and behaviour differ?

A presentation value gives one node a role. A behaviour marker adds a behaviour to its parent and produces no child node.
The iPhone form-sheet role selects a SwiftUI sheet before the ordinary behaviour modifiers run.
Android selects a bottom sheet by role and can apply transition to its contents.
The browser records the role as an attribute. Its dialog and dismiss handlers check separate element capabilities and attributes.
The submit binding selects the Rust action independently of these markers.

The Rust author sets the presentation property. Swift calls its accessor role. Kotlin reads the same property into a variable named presentation.
A behaviour marker uses its place in the tree to identify the node that receives the behaviour.

### Proposed markup for layers, modal interaction, and bottom placement

The author chooses the element. Each renderer implements its contract. The children supply the content and retain their action bindings.

| Tag | iOS | Android | Web |
|---|---|---|---|
| Overlay | Add a layer above the current view or sheet. | Add a layer above the current screen or sheet. | Place a container above the current view. |
| Modal | Present the children and block the background. | Present the children and block the background. | Open a modal dialog and contain focus. |
| BottomSheet | Align the child panel to the bottom safe edge. | Use a bottom-aligned panel or a compatible Material sheet. | Align a section to the bottom of its container. |

Overlay permits background interaction outside its children. Modal owns focus, background input, and optional dimming.
BottomSheet specifies the resting edge. The panel can enter from below. It can contain any supported children.
Inside a modal, present one modal surface. Inside an overlay, keep the background available.
In ordinary layout, the platform measures the sheet from its children within the available space.
The author can insert the sheet when needed. The parent arranges its children again when the sheet appears or disappears.
The author needs no placeholder or fixed height for the sheet.
These are proposed contracts. The present mobile form-sheet role opens a modal on both mobile platforms.

### Follow the Add submission

The shared Add form selects the input action when Rust builds the view. The on-device and server handlers both pass the submitted text to add.
The store assigns the identifier. The action then renders the updated list and dialog from the new snapshot.

### Proposed conditional markup

Inside html!, Either takes a Rust boolean expression in condition. It contains one Left branch and one Right branch.
Evaluate the condition once per render. False selects Left. True selects Right. Evaluate and emit only the selected contents.
The three tags describe control flow and produce no wire nodes. A later render evaluates the condition again.
For the todo row, Left binds toggle_todo on the server. Right binds toggle_local_todo on the device.

In Interaction<T>, T describes the state field. The submitted text travels separately in input_value, a String.
For add_todo, the state is () and input_value holds the title.

### Can a signal hold the todo list across actions?

A long-lived host can own a signal runtime and a signal for the list and its next identifier.
On native targets, SignalHub exposes thread-safe handles that the action context can store. The owner must process queued updates.
An action that needs the committed result must receive it from the owner before rendering its reply.
Restart persistence requires storage. Sending later updates to a screen requires an active delivery path.

### Can an action declare its input type?

Today Interaction has a state parameter and a fixed String input. The action macro accepts one type argument.
A proposed Interaction<S, E = String> lets the handler declare input separately from signed state.
Keep plain text unchanged for existing actions. Give structured input an explicit encoding and decode it before calling the handler.
Typed input still comes from the caller. State verification applies to the separate state field.

### Signal update with a reply

On native targets, the host retains the signal context and drives SignalHub on its owner thread.
The proposed store exposes a RemoteSetter for StoreState. StoreState also needs Clone and PartialEq.
The following sketch uses a one-shot reply channel from futures-channel.

```rust
struct TodoSignals {
    writer: foundation_signals::RemoteSetter<StoreState>,
}

impl TodoSignals {
    async fn add(&self, title: String) -> Result<TodoApp, ErrorTrace<ActionError>> {
        let (reply, result) = futures_channel::oneshot::channel();
        self.writer.update(move |state| {
            let snapshot = add_and_snapshot(state, &title);
            if reply.send(snapshot).is_err() {
                // The caller dropped the receiver. The mutation already finished.
            }
        }).map_err(|_| error("Todo signal owner stopped"))?;
        result.await.map_err(|_| error("Todo signal reply closed"))?
    }
}
```

The application function add_and_snapshot validates the title and identifier before it changes the list.
It returns the updated snapshot from this call. The action can render that snapshot while the hub handles later calls.

### Could native actions share a signal runtime directly?

A native backend can provide thread-safe signal handles. Value storage, graph state, scope ownership, and callback execution need separate rules.
The current graph uses one active effect and one dependency trail. A concurrent reader must not join another thread's effect.
A native update can return the caller's snapshot from the same mutation. Run reactive effects after releasing the write lock.

[Feature 29](../features/29-thread-safe-signals/feature.md) defines the proposed native runtime and the todo update contract.

Feature 29 preserves the owner-thread hub beside the shared runtime. A caller can still receive its own snapshot through a reply channel.

Feature 29 now proposes two named runtimes at the crate root. Call runtime.scope() to create signals in the chosen graph.

### How does the phone show the saved todo?

The todo action renders the list and the closed dialog from the same store snapshot.
The native event bridge gives the reply frame to the screen store. The native renderer displays the updated list and removes the form.

### How can one registry serve several execution modes?

Use one registration function to build the application catalog in each process. Endpoints in that process can share the registry.
Each action declares its name, origin, and surface scope. Compile-time constants can supply these values.
A selector can register several entries under one logical name. Reject overlapping entries for the same origin and surface.
The registry resolves the name against the execution origin and the calling surface. The manager uses the existing transport for a remote destination.
The dispatcher supplies the host-selected mode as a required argument. The registry adapter passes it unchanged to Action::call.

```rust
fn call<'a>(
    &'a self,
    mode: ExecutionMode,
    ctx: &'a Context,
    event: Interaction<Self::State>,
) -> ActionFuture<'a>;
```

Ordinary routing uses the registered entries. Generated adapters for existing two-argument functions can ignore the mode.
For an unqualified name, lookup considers the local and server candidates for the calling surface.
For a phone call, both candidates can support the mobile surface. Local means execution on that phone for this call.
Run the local candidate if it is callable in this context. Otherwise, forward once if the connected server advertises that action.
Report an unavailable action if neither route exists. A local execution error goes back to the caller without a second attempt on the server.
Explicit host and server prefixes select their requested origin. A server handles its requests through its own registered handlers.
The view binds the same logical name in each case.

### Can a struct provide named action handlers?

Keep register for existing actions. Selector registration can expand entries. Named structs are an optional way to organize handlers.
Keep the associated constant `Action::NAME`. With `#[named_action]`, the macro derives the name from the struct in snake case.
For example, `AddTodo` gives `"add_todo"`. An explicit argument such as `#[named_action("create_todo")]` overrides that default.
Both forms set the associated constant at compile time. Markup can read the name from the type without constructing an instance.
The author implements NamedActionHandlers. Its on_ios and on_android methods delegate to on_mobile by default. The author can override either method.
A named handler can use the explicit mode to select its platform method after registry lookup.
Keep the existing registration checks, error propagation, state verification, and async action contract across these paths.

Resolve remote destinations before invoking a local handler. Preserve the existing request, capability checks, and reply path.

### How does the browser discover server actions?

During startup the browser sends a hello. The server uses the requested surface to list callable actions in its reply.
A matching local implementation takes precedence for an unqualified binding. The client can otherwise select an advertised server action.

### Where does a browser action go?

The todo browser configures one action endpoint at /action. The Add form sends the action name and input inside a binary request frame.
The server passes that frame to the dispatcher. The same endpoint serves other registered server actions.

Feature 30 separates frame reading from HTTP dispatch. The same reader can consume a file or an HTTP body through a suitable adapter.

In the proposed HTTP interface, mount reads the route metadata and registers a handler. The request invokes Serve later.
Feature 30 includes the proposed todo route module.

Feature 30 proposes ViewRequest inside foundation_ui_http. The generic HTTP macro forwards the original Serve arguments.

The proposed todo register_routes function accepts a mutable router. It registers actions, views, streams, and assets on that router.

A mounted handler declares its method and path through a trait. The action handler can declare its wildcard alias on the same type.

The proposed initial route returns Response::frame with a complete tree. The handler chooses the response format. The requesting screen owns its mount. Targeted action macros still specify individual placements.

A view response owns the original body source. Tests can inspect it before writing or render it through the production writer into memory.

### How does a request become a Rust function call?

The dispatcher selects the registry entry for the action name, origin, and surface.
The runtime builds the call context from the request context. The registered adapter constructs Interaction using the handler's state type.
For add_todo, input_value contains the submitted title and state is unit. The handler uses the request's services and its own output receiver.

The browser theme generates utility classes. Shared controls currently encode explicit color and font properties for native renderers.

On shared controls, Feature 23 proposes one class list for all three platforms. Matching platform prefixes override the corresponding shared property.

### What does the submitting tab see after Add?

The action stores the new item and takes a snapshot of the list.
The page function uses that snapshot to build the list for the submitting tab.
The dialog function queues an empty dialog for that same tab.
Other tabs that subscribe to the count receive the new count through the server feed.

### Why does the page take a web flag?

The example uses the flag to select one of two page trees.
The browser tree includes a count subscription. The mobile tree supplies a segmented filter and checklist rows.
Both trees use the same render call. The local flag separately identifies the on-device host.

### What does the OnDevice check tell the page?

OnDevice is an empty type in the todo example. The local registry stores it in the application context.
The page checks for this marker to select the local action bindings and show the local count.
Two phones can use different hosts. The marker records how the application starts.

### Why does the todo tree still use presentation?

Feature 23 adds explicit containers for the entry sheet. The sheet still reads child roles for its title and buttons.
The delete confirmation still uses an overlay with a confirmation role. The migration covers only part of the tree.

### Why does an action call ctx.inner().get()?

An action receives a Context. Its inner() method returns a reference to the host Ctx.
The host Ctx provides get() for typed entries. The action Context has no forwarding get() method.

### How does an action return updates when it returns unit?

The action queues output in its call receiver. It returns success when its work completes.
The runtime takes the receiver and drains its items after success. The dispatcher prepares those items for the caller.

### How does the browser apply the Add reply?

The action manager passes each reply to the frame reader.
The todo app registers its page mount and dialog element during startup.
The renderer uses the supplied targets to update the list and clear the dialog in the submitting tab.

### Why does the server keep two live channels for one count?

Each channel owns a stream number, subscribers, and a bounded history for replay.
The browser feed swaps the element named count-value. The native feed renders a footer at node 44.
Both mobile platforms subscribe to the native feed. The on-device example computes its own count.
One shared count target and one portable update would let all three clients use one channel.

### Can every client subscribe to /live?

Yes. Use one count target, one channel, and one route.
The current route chooses the subscriber surface when the server installs its handler.
A shared handler can select the surface for each subscription if the encodings differ.
Each client still owns its connection and resume position.

### Does the platform change the frame format?

Both clients receive the same frame format. The writer can omit operations that apply only to another surface.
A shared text update at a shared node needs no platform choice. Each renderer draws that instruction using its own controls.

### How does a subscriber catch up after a disconnect?

A cursor records the last sequence number the subscriber has applied.
When a subscriber resumes from seven, the channel returns retained frames after seven in order.
The channel registers the subscriber for future updates while it selects this history under the same lock.

### What happens when replay history expires?

The server returns status 410. The browser stream retries. The native stream reports a failure.
The todo app has no automatic view refresh for this response. Issue 668 tracks recovery from a fresh snapshot and cursor.

### Who reloads a view after replay expires?

Proposed flow: the host remembers the view loader when it first mounts the view.
A typed replay error reaches the subscription owner. That owner asks the host to refresh its view.
The loader returns fresh content and a resume position from the same application revision.
The host replaces the affected mount, drops late deliveries from the old subscription, and subscribes after that position.

### Why are there three initial routes?

The browser and mobile todo clients load different versions of the full todo screen.
The separate mobile starter apps load a smaller screen with a count and a Refresh button.
A shared todo view can use one initial route. The count-only demo chooses a separate view because it shows different content.

### What happens when someone taps Refresh on the count screen?

The button invokes the server action refresh_count. The action reads the store, counts open items, and renders the count screen again.

### What does the small phone app register?

The starter gives its host the server address, initial screen path, and remote action name.
The frame binds Refresh to that action. The server registry holds the Rust implementation.

### Does hello advertise the server actions?

Yes. The server lists registered actions for the requesting surface in its capabilities reply.
The action manager accepts an advertised server action even if the client has no declaration for that name.
The starter supplies an explicit name, but a successful handshake already supplies it.

### How does a native button discover a local action?

Rust registers local handlers when the on-device core starts. Each rendered control carries its own action reference.
Swift and Kotlin forward that reference and the event to the core. Local dispatch needs no exchange of action lists.

### Which todo does Complete change?

Each completion button carries the todo identifier in its value. Its action reference selects the handler.
The native event bridge sends the event to the core. The Rust handler reads the value to select the todo.

### How does a label appear without an action?

Rust lowers a Text tag and its content into shared instructions.
The browser creates a text element. SwiftUI and Android Compose draw a label from their local node records.

### How can one app display two hosts?

A SwiftUI TabView can contain a LocalScreen for drafts and a ServerScreen for team todos.
Each tab uses the core factory from its corresponding example. The local tab gets its first frame from Rust.
The server tab supplies its base address and initial route. Its core receives the transport from ServerScreen.

### How does one screen use local and remote actions?

The server-backed todo screen opens its entry sheet through a host action.
Submitting the form calls the server action add_todo. The screen keeps the same core for both events.
