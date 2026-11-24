# IPC Explorations

## Initial Discussions

 Test agent finished — will apply that separately. Let me focus on IPC design.

  All four approaches, each as a separate IPC route in the Android example. Let me think through them:

  ---
  Approach 1: Tauri Command (invoke())

  Mechanism: JS calls window.__TAURI__.invoke('command_name', { args }) → Rust #[tauri::command] handler runs →
  result returned to JS.

  The ewe:// question: In Tauri v2, __TAURI__ injection is tied to the custom protocol. For ewe:// pages, we
  control the protocol handler — we can inject the Tauri IPC shim ourselves during register_ewe_protocol, or
  use dangerousRemoteDomainIpcAccess.

  IPC route idea: /api/command — page with a form, user types a message, clicks "Send", JS calls invoke('echo',
  { message }), Rust echoes it back with a timestamp.

  ---
  Approach 2: Tauri Events (emit/listen)

  Mechanism: Rust app_handle.emit("event-name", payload) → JS listen("event-name", callback). Bidirectional,
  decoupled. Unlike invoke(), events are push-based.

  IPC route idea: /api/events — page subscribes to platform:tick events. Rust emits a counter every second. The
  page displays a live-updating ticker without polling.

  Advantage: Works even if __TAURI__ doesn't load — events go through the Tauri runtime, not the URL scheme.

  ---
  Approach 3: Backend Transport (dispatch_ipc)

  Mechanism: Navigation to ewe://localhost/api/* → RouteSource::IpcShell → BackendTransport::dispatch_ipc() →
  runs real Rust logic → returns response through the RouteResponder.

  IPC route idea: /api/system — the existing IpcEmit responder reads /proc/cpuinfo or runs whoami, returns
  actual system info in the HTML. Proves the backend query path does real I/O.

  Advantage: Uses the existing architecture (already wired through session → handler chain → responder). No new
  JS required.

  ---
  Approach 4: Capability Contract

  Mechanism: CapabilityRequest { id, capability, action, payload } from WASM/JS → session routes to registered
  Capability handler → executes → CapabilityResponse back.

  IPC route idea: /api/capability — register a ClipboardRead capability. Page has a "Read Clipboard" button.
  Click → CapabilityRequest → Rust handler reads clipboard → response displayed.

  Advantage: Proves the full decision-07 architecture. The same contract works for filesystem, camera,
  biometrics later.

  ---
  The ewe:// Tauri IPC problem

  All four approaches face the same question: does __TAURI__ work on ewe:// pages?

  In Tauri v2, the IPC injection happens inside the custom protocol handler. Since we register ewe:// as our
  own protocol, we can inject the Tauri IPC initialization script ourselves during register_ewe_protocol. The
  PlatformBuilder already injects platform-scheme-interceptor.js via window.eval() after the page loads — we
  can add the Tauri IPC bootstrap there too.

  What do you think about this breakdown? Any preferences on which mechanisms to prioritize, or concerns about
  the ewe:// bridge?

    ---
  How Tauri v2 invoke() Works

  Tauri's IPC has three layers of JavaScript init scripts, always injected into every webview during creation
  (regardless of URL scheme):

  Layer 1: core.js — Callback infrastructure

  Sets up window.__TAURI_INTERNALS__ with:
  - transformCallback(cb, once) → numeric callback ID
  - runCallback(id, data) → invokes the stored callback
  - invoke(cmd, payload, opts) → registers callbacks, calls __TAURI_INTERNALS__.ipc()

  Layer 2: ipc.js — Pattern router

  __TAURI_INTERNALS__.ipc(message) routes based on the pattern:
  - brownfield → __TAURI_INTERNALS__.postMessage(message) (direct WRY IPC)
  - isolation → iframe postMessage to isolation origin

  We use brownfield ("pattern": {"use": "brownfield"} in tauri.conf.json).

  Layer 3: ipc-protocol.js — Wire transport (two paths)

  ┌─ fetch("ipc://localhost/{cmd}")  ← Desktop (custom protocol POST)
  │  Headers: Tauri-Callback, Tauri-Error, Tauri-Invoke-Key
  │
  └─ window.ipc.postMessage(data)    ← Android (WRY bridge)
     canUseCustomProtocol = FALSE on Android (line 20)

  On Android, osName === 'android' → canUseCustomProtocol = false, so it ALWAYS uses window.ipc.postMessage().
  This is the WRY-level IPC bridge — it works regardless of what URL scheme the page loaded under. No ipc://
  protocol needed.

  The invoke_key

  A random Z85-encoded 128-bit key generated at app startup. Embedded into ipc-protocol.js as
  __TAURI_INVOKE_KEY__. Sent as a header in ipc:// requests. Prevents unauthorized pages from invoking
  commands.

  manager/webview.rs).

  ---
  Conclusion for our ewe:// pages

  invoke() already works on our Android ewe://localhost/app/ pages. The init scripts are always injected. On
  Android, the wire transport uses WRY's window.ipc.postMessage() — no URL scheme dependency. We just need to:

  1. Register #[tauri::command] functions
  2. Call window.__TAURI_INTERNALS__.invoke('command_name', { args }) from our JS
  3. Done — the existing brownfield WRY bridge handles the rest.


  F23 — IPC Shell: Four IPC Demos

  Tauri v2's invoke() already works on our ewe:// pages because:
  - Init scripts are injected during webview creation (not per-page)
  - On Android, it uses window.ipc.postMessage() (no ipc:// protocol needed)
  - withGlobalTauri: false only hides window.__TAURI__, not __TAURI_INTERNALS__

  Four IPC routes, each proving a different mechanism:

  ┌─────────────────┬─────────────────────────────────────────────────────┬────────────────────────────────┐
  │      Route      │                      Mechanism                      │         What it proves         │
  ├─────────────────┼─────────────────────────────────────────────────────┼────────────────────────────────┤
  │ /api/invoke     │ __TAURI_INTERNALS__.invoke() → #[tauri::command]    │ Tauri bridge works on ewe://   │
  ├─────────────────┼─────────────────────────────────────────────────────┼────────────────────────────────┤
  │ /api/events     │ app_handle.emit() → JS event listener               │ Push-based server-driven       │
  │                 │                                                     │ updates                        │
  ├─────────────────┼─────────────────────────────────────────────────────┼────────────────────────────────┤
  │ /api/system     │ RouteResponder::respond() reads real system info    │ Existing architecture does     │
  │                 │                                                     │ real I/O                       │
  ├─────────────────┼─────────────────────────────────────────────────────┼────────────────────────────────┤
  │ /api/capability │ CapabilityRequest → Capability handler →            │ decision-07 contract           │
  │                 │ CapabilityResponse                                  │ end-to-end                     │
  └─────────────────┴─────────────────────────────────────────────────────┴────────────────────────────────┘

  Each page gets a button + output area. All include the interceptor + nav buttons.


## My Response

I had to go dive deep into how tauri works and the different APIs you mentioend and it provides, all documented here with some additional checks on injecting scripts: /home/darkvoid/Boxxed/@dev/ewe_platform/specifications/52-tauri-foundation-platform/foundations/tauri-ipc-shell.md

So here are my ideas:

### Capabilities based IPC

Capabilities based IPC are interesting but we've initially designed them around providing extra functionalities that are explicitly owened by our API and are specifically reached via the register_capability and invoke_capability APIs.

And while they can be used for general IPC, they are not the primary mechanism we use for IPC in our platform and I feel like we are about to abuse them here for such a need.

Infact I love the Capabilities concept so much I wish to raise this up to the foundation_wasm level, so that every one gets it by default and can use it and the contact simple is:

1. A capability exposes a Trait that has a invoke_capability method that is implemented by the capability provider and they all take a type that is either implementing serde::Serialize and Deserialize and our To/FromArrow traits.

2. A capability responds with a Result whoes type is serde::Serialize and Deserialize and our To/FromArrow traits and whoes errors can also be serialized as such.

The crate provides a flexible API to register a capability and the capability provides a unique name or id which must be used to invoke the capability and will be in the CapabilityRequest object passed in.

Lets add a new feature to add this directly to foundation_wasm, then let others inherit it and they can decide when a capabity should be invoked or not outside of the functionality, this way there is flexibility to it.


We need to be thoughtful as to how the responses (which we actually never) solved will be used. I think in such a case, we should make it possible for the route handlers to be able to get a capability and invoke it with a CapabilityRequest object which they get back the response and can do whatever with it.

So its not a route handler itself but its a feature route handlers can get from the session to do something with.

### Foundation wasm + ui + polyfills as injected

So tauri has actually nicer APIs already that allows us inject specific scripts into the every webview on creation, I think we do well to build a ScriptInjector into our platform that we can create as part of the session and platform builder that adds a series of scripts we just always inject by default.

This allows us disable the single_runtime flag and write a plugin that just automatically takes the ScriptInjector, get all the different javascript text and auto-inject them always.

This way the links polyfills, the foundation_wasm and foundation_wasm_ui runtimes are always auto-injected, simplifying the output of the WasmBundleGenerator to just the script for the app, we probably need to configure it so it can skip generating those when disabled, so the platform generator can set it and just generate the js file for file and wasm alone, whilst still respecting the single=True attribute to combine the wasm and js for the app into a single bundle file and its respective different bundling styles.

Lets add a feature for this and implement it.

### A true IPC mechanism

Inspired by our capabilities and the various ipc mechanisms tauri has: commands, invoke, events and channels, i think we are in  a good position to build a great IPC mechanism over them and even those around them.

Like the capabilities, instead we implement a derivative of the RouteHandler for IPC interactions but also as an independent concepts on their own.

There will be a central IPC registery that registers all IPCs for the foundation_platform and like capabilities they take a request and return a response which must implement serde::Serialize and serde::Deserialize and our To/FromArrow traits, the only difference is these IPCs can also be used like a route handler, if they invoked and their response will be the result delivered as the response to invoke, or a link request that hits the route. 

This allows us also get an IPC in a regular route handler to invoke it for some an operation and use the result in our response. This is import, the session should provide a way for us to get an IPC by a specific name and invoke it.

In essence, we register automatically in Platform builder a command for IPCs which the frontend can trigger to invoke an platform ipc command and the request has the id/name of the IPC different from the name of the tauri command to be invoked, in essence we provide a nice js side that does the needed tauri invoke call target the command specific for the platform IPC requests.


We also enable `withGlobalTauri` as `true` so the __TAURI__ global is always injected.

The benefit of this approach is our ewe:// route can work if its registered as a route and the app can trigger it via calling our APIs.

I am even tempted to also move this up a layer into foundation_wasm, then we add all the tuari specific pieces in  foundation_platform.

This way, the invoke API is also standard and settled and we just add a tauri specific means via the tauri `invoke` api to also be able to call our IPCs, maybe we call it `invoke_ipc` and we just ensure that foundation_wasm provides this as part of its runtime originally and foundation_platform just overrides it to pass it through the tauri invoke call since thats the only way to pass the boundaries. 

The IPC can do whatever it needs to do, routers can call them to do something for them e.g the app running in a IPC process, it does not matter it all works.

Then we can craft different IPCs types around the invoke, events and listen capabilities if we want and extend them further. 

We dont need special routes like you have them, users can register whatever IPCs on whatever routes they want, we dont force them into a specific way.

I think whith this type of structure all the different IPCs examples you should:

1. events (push based)
2. system as RouteResponder::response
3. invoke commands are all possible.


### Streaming

The tauri channels is an area we have yet to explore but really is a powerful API we should look into, develop and add a feature to and see where we can take advantage of it without bringing in tokio or whatever. 

It definitely will be very interesting to see where we can apply it.
