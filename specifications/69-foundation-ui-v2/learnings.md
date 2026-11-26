# Spec 69 learnings

## Todo Server launch on iPhone

A simulator launch argument applies to one process. The installed app needs its server address in its bundle.
SpringBoard builds the static launch image before the Todo process can draw.
The simulator rejected the unsigned storyboard resource with security error -67056.
It also reported that the bundle had no supported orientations.
The second launch then used an empty system surface while the simulator prepared the scene.

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Document Object Model (DOM); HyperText Markup Language (HTML);
AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE standard
ASD-STE100.

- Compile-time targeting already works on the web. `html!` writes `primal-id`, `Part::node_id`
  and `DomOp` carry the number, and the JS `NodeRegistry` maps it to the DOM node. The new
  work is one schema for the ids and a native reader. Do not present the mechanism as new.

- The HTML morph is a reconciler, not a second page tree. It compares one received fragment
  with one live subtree. Its cost is bounded by the fragment. It is worth keeping, because it
  gives form preservation and move detection on the web. Express it as one schema type,
  `HtmlMorph`, and gate it by the `mount` field of `ClientHello`. A native reader must
  never render its string or run its scripts.

- An HTML string cannot be a typed operation for native. The schema can hold it, but only as a
  web-only union member. Every place that lists payload fields must name this exception, or the
  App Store 2.5.2 note is wrong.

- `flatc` generates Rust, TypeScript, Swift and Kotlin from one `.fbs` file. `planus` generates
  Rust only, so it cannot be the generator (D7).

- An HTML attribute holds text only. Binary state cannot ride it. The web state is a
  base64url string (D12). A JavaScript expando property can hold bytes, but the markup cannot
  see it and it does not survive a server render or a reload.

- A second vocabulary for the web is not needed. One core vocabulary plus a web-only subset
  with a string fallback is simpler (D10). Mobile reports a web-only name it cannot render,
  and the web supports every name.

- The web island machinery (`<mount-stream>`, `<mount-data>` and scoped JS) is a feature, not
  legacy. Keep it (D4, D11), and give each behaviour the same compiled form on native
  ([12](./research/12-scoped-behaviour.md)).

- The frame apply is synchronous. Recomposing the views is the platform render pass, and
  SwiftUI and Compose own it. Do not add a scheduler between the two.

- Signed state is not authority. The tag proves who made the bytes; it does not prove the
  values are current or that the user may use them. The server can ignore the client copy, keep
  a key from it and load the truth from the backend (D13). Design state as a handle.

- Single use belongs to the domain data. A payment is created once, and a delete changes a row
  once. A framework nonce store repeats a fact that the domain already owns, and adds a second
  store and a failure mode (D14). Do not add one.

- Do not design out the signal server or the replay backlog. The owner calls them good
  properties, and signals cannot move across threads (D15). A stateless mode is an addition for
  actions that do not need a live graph, never a replacement.

- One response and a stream were always both possible. A change in where state lives between
  requests does not change the wire shape (D15). Say what is new and what is unchanged, or the
  design looks like a removal.

- The framework does not prescribe a store, a cache or a session table. It supplies the
  request, the signed state, the action registry and the frame builder; the server decides the
  rest (D15).

- Do not invent an application programming interface (API) the source document does not have.
  `ctx.authorize`, `ActionCtx`, `ActionResult` and a nested `Result` all came from the research,
  not from the document, and the owner rejected them. Quote the document's signature and put a
  real gap in `questions.md`.

- Encoding sits after the action. The action returns a logical `Response`; the handler that
  delivers it chooses ops, columnar, JSON or MessagePack (D16). Do not give the action an
  encoder, and do not let the action choose a wire format.

- `Ctx` wraps `foundation_context::Ctx`. Clocks and randomness pass down through it, and
  `Context` wraps `Ctx` and adds what the action needs (D17). Do not make a parallel context.

- The `#[action]` macro is not a registry stub. It reads the signature, so the request decode
  and the call glue are generated code (D19). Put only the steps that are the same for every
  action in the library.

- An element id is not state. `primal-id` is built into the code, so it needs no stability rule.
  A `View` or a `Div` has one fixed id; a custom element uses a string or a user-defined id
  (D20, Q1).

- Name the crate. `foundation_ui_wire` (D21, Q2).

- One registry model on every surface. The event names the action; the registry finds it; the
  action gets the node, its parent and the data. The action can be local or remote, and the
  caller does not care (D22, D23). The app builds its own registry from macro-generated structs;
  do not generate a global registry or a list of action kinds.

- The owner did not ask for remote procedure calls (RPC). `#[action]` is a plain function
  attribute; `#[service]` stays for code that wants RPC (D18). Do not build the simple idea on
  a router because a macro exists.

- The transport is a boundary concern (D25). We already serve SSE, HTTP and WebSocket, and
  ConnectRPC is possible. Do not pick one for the user, and do not build a transport decision
  into the schema.

- An action is named, not numbered (D31). The macro-generated struct carries the function
  name, and that name is the lookup tag. A hash-based numeric id is invented work.

- The framework delivers a result. It sends state updates or DOM/mobile UI updates, and it
  does not decide the delivery strategy (D32). Do not give it opinions about local then
  remote, optimistic updates or an update policy.

- Do not reinvent what exists. Live streams already use `<mount-stream>` and header
  negotiation, and a client can connect for SSE or WebSocket (D30). The same rule applies to
  stored `foundation_ai` data, which is out of scope (D26).

- `Interaction<T>` is typed by its `T`. The generic names the payload type, so the macro
  decodes a typed interaction. `input_value` stays a `String` inside that struct; it is the raw
  event value, not an untyped design (Q29 is not a question).

- One consolidated representation serves tier 1 and tier 2 (D27). The hand-written columnar
  code retires when the new form delivers the same functionality on the web, not on a
  benchmark date. The different data formats, Arrow included, stay (D28).

- Spec 52 and `foundation_platform` have valid use (D34). Build on them and decide what to
  keep, remove or leave for later as the work proceeds. Do not freeze them as a reflex.

- Lead with the north star (D35). The research is one contract that carries UI, data and
  actions across every surface. Each idea file serves that contract; an idea that reads like a
  separate redesign makes the set look like over-architecture.

- A change has one form (D36). The cross-platform set is what all three surfaces can do. Only a
  change that the set cannot express gets a form for one platform, and that form has no core
  or other-platform twin. Do not model a core form plus a web form of the same change; that is
  duplication. Do not split the schema per platform and do not force the server to write one
  batch per surface.

- Keep the current web capability for script actions (D37). A `<script>` beside the markup
  defines functions that become callable after mount. They register as `js:` actions and stay
  on the web. Mobile never runs them. The action model is the same on every surface, and the
  registry has one entry per origin.

- The platform scope and the action origin are two markers, not two choices (D37). The
  declaration says where the action exists (every platform, web, mobile). The reference says
  where it runs (`server:`, `native:`, `js:`). One manager checks both and routes. Do not
  remove one marker to simplify the other; they answer different questions and compose.

- The reference names its origin. `server:` goes to the backend, `native:` to a local WASM or
  Rust core, `js:` to page JS on the web. The central action manager never guesses. The server
  accepts only `server:` references, so a client cannot send `js:` or `native:` over the wire.

- Reuse is a layer split, not a copy (D38). The standard and the shared runtime are one. Each
  platform implements a capability behind the same interface. An engineer adds a platform hook
  only where the surfaces differ.

- `mount-stream` and `mount-data` are core ideas, not web features (D39). Mobile uses a live
  mount record with the same four fields and the same frame schema. Only the representation
  and the channel host differ.

- The owner's review is a lens, not a delete request. Keep the agreed parts and make them fit
  the north star (D40). Do not drop the schema, the morph, the islands or the live mode to look
  simpler.

- When the owner rejects a part strongly, delete it and return to the source document. Do not
  defend the invented part with more detail.

- Remove nothing the owner did not ask to remove. MessagePack stays as an opaque body (D9).
  The morph stays as `HtmlMorph` (D8).

- The old `docs/bug-backlog.md` file no longer exists, and one finding in it is fixed.
  `foundation_auth` `TokenSigner::verify` compares tags in constant time with
  `Mac::verify_slice`, and the signer has a key ring with key ids (commits `25cab64fe` and
  `540611b30`). Cite the commits, not the deleted file.

## Design specification (2026-11-13)

- The research left two forms for the state on the wire: a `StateUpdate` body, and ops plus
  state in one frame. A change has one form (D36), so the design has one: the
  cross-platform op `AttachState`. The view and its state then travel in one frame and
  cannot arrive apart.

- The client must return the state bytes exactly. A table field would make each client
  build the table again. A `[ubyte]` field with `nested_flatbuffer` keeps the bytes as the
  server made them, on the root element (base64url) and in the request.

- `html! { ctx, receiver, <…> }` is already the reactive form of spec 39. The source
  document writes `html! { ctx, Some(&state), <…> }` for a view with state. The macro cannot
  tell the two apart, so the view form is `html! { view(ctx, Some(&state)), <…> }`. Check an
  idea from a document against the syntax that exists before you copy it.

- `flatc` does not promise `force_align` on a vector in every language. Only Rust writes
  frames with numeric columns, so one Rust helper (`Aligned8`) makes the 8-byte rule true
  with no dependence on that promise. Readers check the offset and copy when it is wrong.
  When a fact about a tool is not certain, design so that the result does not depend on it,
  and test the result.

- A hash of the action names cannot tell the server which names an old app has. The server
  needs a table from the hash to the name set of each released version, or it must answer
  `UpdateRequired`.

- `foundation-wasm-ui.js` has no `import` line, and hosts serve it as one file. New runtime
  code goes in separate modules, and the old file gets small static seams
  (`ProtocolHandler.frameHook`, `EventDispatcher.actionHook`), like the existing
  `ProtocolHandler.arrowIpcReader`.

- The live `Broadcaster` keeps every frame so that a late joiner can build the page. A
  plain backlog limit breaks a late joiner. The real fix is a snapshot frame (issue #548).
  Do not add a limit with a default that silently breaks the join.

- `WasmEnvelope` is also the frame of protocol byte 0, the general batch-instructions ABI
  of `foundation_wasm`. Only its use for DOM ops retires. Read what a type is used for
  before you list it for removal.

- A browser WASM module has one thread. A `native:` action there is polled one time; a
  pending future gives a typed error. Never spin on it (`CLAUDE.md` rule 6). On a device
  the same dispatcher is awaited on the Rust executor.

- The FlatBuffers Java runtime has no verifier. The Kotlin reader checks the size, the
  length and the identifier, and reads inside a `try`.

- The behaviour modules are eleven, not ten: the research list leaves out `composite`
  (`machinery/mod.rs:27-37`).

- Each native renderer needs the same oracle. A small reference model in Rust writes the
  expected store as JSON for each fixture frame, and the Swift and Kotlin stores must give
  the same JSON. This is cheaper than two hand-written sets of expected values, and it
  keeps the two platforms from drifting apart.

- `flatc` was run on 2026-11-14, after the review asked for the measured facts. The release
  was 25.12.19 first, then 25.9.23. The exact flags of feature 01 section 6.3 work on both:
  `--rust --gen-object-api --rust-module-root-file --filename-suffix ""`, `--ts
  --gen-object-api`, `--swift` and `--kotlin`. `mise install github:google/flatbuffers@<v>`
  downloads the release asset and gives that `flatc`. The install route needs no plugin.

- The pin is 25.9.23, not 25.12.19. The npm `flatbuffers` runtime stops at 25.9.23, and the
  generated TypeScript imports it. The Rust crate has 25.12.19 and 25.9.23. One release for
  the tool, the crate and the runtime is the safer rule, so the newest common release wins.
  This is the rule 17 exception: an upstream package forces the older version.

- `force_align` on a vector is not honoured by every generated writer. Measured with 25.9.23
  and 25.12.19 and `--gen-object-api`: the Rust `pack` writes `_fbb.create_vector(x)` with no
  alignment, and Swift writes `addVectorOf` with no alignment. TypeScript and Kotlin write
  `startVector(8, n, 8)`. The frame writer is Rust, so the `Aligned8` helper and the reader
  offset check are required. Do not trust the attribute alone.

- `flatc --rust-module-root-file` writes one file per type under the namespace path plus one
  `mod.rs` at the output root. All schema files in one invocation share that `mod.rs`. Each
  file must put `include` lines before `namespace`; otherwise `flatc` stops with "includes
  must come before declarations". A failed invocation can leave earlier output files behind,
  so the check script writes into a temporary directory first.

- A field named `table` is accepted by `flatc` (tested on the D45 shape). A table may hold
  three union fields (`ui`, `web`, `mobile`), and an `enum : ushort` is accepted. The D44,
  D45 and D47 shapes generate in Rust without a workaround.

## Toolchain (2026-11-14)

- mise has a backend for every native tool, so no download script is needed:
  `github:google/flatbuffers`, `java` (`core:java`), `gradle`
  (`aqua:gradle/gradle-distributions`), `kotlin` (`github:JetBrains/kotlin`) and
  `android-sdk` (`vfox:mise-plugins/vfox-android-sdk`). Feature 17 pins them.

- The host JDK is not the Android JDK. The machine had Java 27, and Gradle 9.9.1 starts on
  it, but a build with `jvmToolchain(21)` fails with "Cannot find a Java installation".
  Temurin 25 through mise plus `jvmToolchain(25)` compiles with the Kotlin JVM plugin 2.4.21.
  Pin the JDK; do not rely on the host Java.

- The Android SDK on the host had only the NDK. There is no `sdkmanager`, platform or
  build-tools. Feature 15's Gradle build needs all three, so feature 17 adds a
  `toolchain:android` task that installs them.

- Xcode is installed, but `xcodebuild` fails because the active developer directory is
  `/Library/Developer/CommandLineTools`. The one-time fix is
  `sudo xcode-select -s /Applications/Xcode.app/Contents/Developer`. The toolchain check
  reports this fault with that command, instead of a bare "Xcode not found".

- The CI image tools live in `ci/platform-images/mise/mise.toml`. A path such as
  `ci/platform-images/images/linux/mise/mise.toml` does not exist. The image is generic and
  installs its tools in locked mode; a repository pins its own tools in its own
  `mise.toml`.

## Operation groups (2026-11-14)

- A count of a shared set must name its scope. D53 says "four shared types", and it is right
  for the mapping of the nineteen `DomOp` variants. Read as the size of the whole shared
  set, it is wrong: node creation, props, subtree replacement and the action binding are
  shared too. Write a count with the set that it counts (D59).

- The shared set is twelve types. Every runtime must apply all twelve, so a wrong member
  costs three implementations, not one. Cut a type from the shared set only when a platform
  cannot do it at all, not when that platform has a faster form.

- A platform type is for what the shared set cannot express. `WebElement` stays for HTML
  tags outside the ten core elements; it is not a second way to create a `Stack` (D36).

## Revision pass for D41 to D59 (2026-11-14)

- One root schema is cheaper than two roots. Two roots (`ActionRequest` and the message
  frame) need two identifiers, two content types and two readers. One root `Frame` with a
  `FrameKind` union needs one of each, and the kind check is one typed error
  (`WireError::WrongKind`). The cost is one extra box per message frame (`Box::new` in the
  object API), which is small.

- The request size cap belongs after the identifier check and before the field reads. A wrong
  identifier is cheap and needs no cap; a big buffer with the right identifier needs the cap
  before a vector length is read.

- `ErrorTrace::new` needs `C: core::error::Error + Send + Sync + 'static`. The generated
  `ActionErrorT` implements neither `Display` nor `Error`, so the wire crate needs
  `error_ext.rs`. Do not write a second error type; add the two impls to the generated type
  (rule 5).

- A behaviour feature that owns the vocabulary cannot land after the features that lower and
  apply behaviours. Feature 16 owned the name tables, the lowering and the web registry, and
  it depended on the native renderers. That made feature 08 depend on a later feature and
  made the web features depend on native. Split by platform instead: the names are
  vocabulary (feature 02), the lowering is the macro's (feature 08), the web registry is the
  applicator's (feature 09), and the matrix and the native bindings stay last (feature 16).

- A phase order and a dependency graph can disagree. The Index groups features into phases,
  but the dependency edges decide the order. Check both after a revision: a feature in an
  early phase must not depend on a feature in a later phase.

- A plain action needs a reference text. If `origin()` is `Option<Origin>` and a plain action
  has no origin, the html macro must still write a prefix. Write `host:name`, and let the
  manager fall forward to `server:<name>` only when the server lists the name (D50 (8)).
  Reject one name under two origins in the registry, so the fall-forward is unambiguous.

- The example app must be the one the decisions name. The counter example survived in
  features 10, 11, 12 and the requirements after D48 moved to the todo list. Grep for the
  example crate name in every file, including verify commands and CI lists.

- A linter and a test cannot see a renamed example crate. Run the verify commands of each
  feature after a rename; `cargo test -p ui_counter` fails only when a person runs it.

- The dedicated spec branch includes decisions D41 to D59. Feature 01 requires the toolchain check from feature 17.

<!-- uniffi-proof:start -->
## UniFFI toolchain proof

- Rust: rustc 1.100.0-nightly (feaadeeac 2026-09-19)
- UniFFI: 0.32.2
- Swift: Apple Swift version 6.4.2 (swiftlang-6.4.2.1.9 clang-2100.4.1.1)
- Xcode: Xcode 27.2
- iOS simulator SDK: 27.2
- Kotlin: info: kotlinc-jvm 2.4.21 (JRE 25.0.5+8-LTS)
<!-- uniffi-proof:end -->

## Feature 17 platform checks

- The FlatBuffers compiler 25.9.23 generates Rust, TypeScript, Swift and Kotlin bindings from the same test schema.
- Xcode 27.2 includes the simulator kit. The Apple proof selects full Xcode for its process when the host selects Command Line Tools.
- The full host check reads the Java Development Kit from `JAVA_HOME`. A host Java executable can take precedence on the shell search path.
- Gradle 9.9.1 builds a debug Android application with platform 36 and build tools 36.0.0. The test verifies the signature and the launcher.
- A second package installation finds the platform tools, platform 36, build tools and native development kit 27.0.12077973 already present.
- The Android project for feature 15 does not exist yet. Test fixtures verify the wrapper and catalog checks until that project exists.

- The original Android installer accepts a wrong lock checksum. The direct download backend rejects it on a clean install.
- The Android Gradle Plugin uses an older Kotlin plugin by default. Selecting the pinned Kotlin plugin removes the Gradle deprecation warning (issue #615).
- The borrowed FlatBuffers operation tag preserves unknown numbers. The generated object maps an unknown operation to `NONE` during unpacking.
- A generator input that includes the frame and state schemas gives one Rust module index with all 92 generated type files.

- Feature 18 is reserved for the WebView control. Feature 19 owns native package publication and tests installation from clean starter applications.

## Feature 01 verification

- FlatBuffers 25.9.23 preserves the numeric tag on borrowed unknown operations. Unpacking returns the empty member.
- Generation adds an outer table check for unknown union members. The drift test covers this step.
- The generated Rust writer does not align every numeric column to eight bytes. Feature 03 must use the aligned vector helper.
- The Rust runtime pin also satisfies the Arrow library dependency. The Arrow tests pass with this shared version.
- Node 24.21.0 runs the fixture tests with an explicit file name. Its test runner rejects the directory argument.
- Cargo cannot list package contents directly in this linked checkout. An isolated package listing confirms the browser runtime archive includes the new module.
- Workspace manifest warnings for unused dependencies remain tracked in issue #596. The wire crate passes strict Clippy checks.

- A standalone consumer example uses the generated public constructors and defaults. The test-only table macro is unavailable to consumers.
- The FlatBuffers compiler can encode and decode a hand-written action request with the frame schema and an explicit union tag.

## Signed state dependencies

- The new HMAC library requires the `KeyInit` trait for key construction. Existing authentication tests cover this change.
- BoringTun still needs the older digest interface for its BLAKE2 implementation. The workspace manifest records this dependency constraint.
- The new digest output lacks hexadecimal formatting. The shared hexadecimal encoder preserves lower-case output in authentication, code generation and deployment exports.

## Feature 02 vocabulary verification

- The existing browser tag and attribute tables match the Rust order. Their maxima are 114 and 81.
- Version 1 has no behaviour identifiers. Version 2 adds four shared identifiers.
- A legacy child removal and a legacy node removal use the same shared wire form.
  The reverse conversion returns a node removal.
- The test writes the browser vocabulary module from the Rust tables.
  The browser module keeps numeric indexes for properties, node kinds and events.
- The vocabulary classifies forms. The frame writer applies the mount filter in feature 03.

## Signed state verification

- A tag over decoded fields cannot detect a change to unused FlatBuffers padding.
- Private state adds a nonce and an authentication tag. The envelope limit includes these bytes and the table overhead.
- The runtime opens a private body larger than one mebibyte when the host raises the body limit.
- Issue #619 tracks direct failure recording with a missing clock. The dispatcher checks the same context before it records a failure.

## Producer migration

- The feature 02 vocabulary example uses the legacy converter. Feature 20 replaces it with a direct writer example.
- Moving producers also requires the local and streamed browser readers. A change to the Rust operation type alone cannot switch the default protocol.


## State and action boundaries

- The wire writer currently parses attached state. Signer-owned formats require it to carry those bytes without parsing them.
- The later action discussion gives the receiver to the caller. The caller drains it on success and discards it on error.

- Web, Swift and Kotlin use the triggering node by default. A binding can explicitly name another state owner in the same user’s view.

## Frame writer

- Four offsets changed from zero to six in the three-row fixture. The reader also accepts the earlier encoding.
- A table slice needs its target on the outer data message and on the inner table. Issue #621 covers this case.
- The generated object packer can produce a valid frame with misaligned numeric data. Tests measure offsets in the finished buffer.

- `Ctx::fork()` copies the typed map. A request can replace its signer entry while sharing the signing key with other contexts.

- The same action receiver accepts state alone or a rendered view followed by its next state. The caller drains both on success.

- The login worker already resolves a trusted user through its session manager. A context fork isolates each request’s signer entry and current user.

- A template reference needs component-instance scope. Two mounted todo lists can both name their owner “todos”.

- The template parser accepts one colon between names. The reserved name `primal::state` needs no author syntax. The reactive emitter already knows each element’s node number.

- A template marker selects a destination during rendering. Signing once gives each marked control identical bytes, even with a randomized signer.

- A signed attachment describes one render. A later action can reuse the same render function with a new value.

- The receiver form of html! queues operations before returning Html. The tree carries the root identifier for later placement. Flush performs delivery.

- A frame can describe a complete section while another frame describes its sibling. Sharing a stream identifier does not make those renders interchangeable.

- The server signal and browser island examples demonstrate content delivery. They do not couple two Rust signal graphs. A socket handoff and action completion have separate lifetimes.

- A genuine attachment can outlive a domain change. A state-only update can also make the bound input newer than its visible label.

- The signer checks binding and expiry. A wire reader only bounds and copies its bytes. Request forks and persistent hosts both use the same signer interface. A context clone shares its entries. A fork isolates request binding changes.

## 2026-11-20. Action registration and event delivery

A server name alone cannot tell a client whether a missing local action permits fallback.
A client can keep the plain declaration when it omits the local implementation. The manager then checks the advertised server names.

An absent state attachment must stay absent in the wire request. An empty byte vector incorrectly reached the signer as a submitted attachment.
The stateless routing test checks the generated request field. The fix closes [issue 627](https://github.com/ewe-studios/ewe_platform/issues/627).

The host can share one context across actions. Each invocation keeps its own receiver and trusted render target.
A delivery handler can call `prepare_messages` to sign the output once before it selects an encoder.

## 2026-11-21. Action macro expansion

An absolute crate path lets the macro find the runtime through a dependency alias.
The runtime also names itself for code expanded within the crate. A separate consumer builds with the alias `ui` on both targets.

The expansion checks the author's declared return type, including early returns. The runtime future then checks the action result and target-specific bounds.
The native compiler rejects a reference-counted local value held across an await. The same handler can compile for WebAssembly.

The release configuration pairs the macro crate and the core crate. Generated Cloudflare manifests now read the shared macro dependency from the workspace.

## 2026-11-21. Negotiated connections and trusted render targets

The resume counter describes outgoing replies. A client can resume after reply 19 and submit request 1.
The dispatcher acknowledges request 1 in reply 20. Local transport admits ordered requests before their futures can be polled in a different order.

The host owns the mapping from a triggering control to its render target. One persistent context can serve several components with a host lookup.
The receiving control still supplies its own signed state. A wire parent field cannot change the mapping.

A renderer can encode a state attachment as a direct operation, a nested operation or a columnar row.
The receiver finalizer rejects all three forms and signs only the state queued through its state methods.

## Feature 20 migration review

The legacy browser keeps a detached child registered. It unregisters a removed node.
The current wire bridge encodes both as the same operation. Its round-trip test accepts this loss.
A browser parity test must reattach the detached node by identifier. [Issue 629](https://github.com/ewe-studios/ewe_platform/issues/629)

The columnar-native receiver writes directly to column buffers. A row-batch adapter covers only one of the existing receiver paths.

The wire vocabulary uses tag and attribute names from the traits crate independently of the legacy converter. Keep the wire-to-traits dependency after migration.

The feature headers, requirements totals and progress table track the same 19 features. Count full completion separately from implementation with an open verification check.

## Templates and browser actions

- Slot zero is a valid WebAssembly allocation. The action bridge uses the maximum unsigned 64-bit integer for a missing reply.
- A detach operation keeps the node handle. A later operation can attach the same node to another parent.
- The bound control supplies its signed state. The parent node number describes the tree and does not select state.
- The frame reader delivers errors and capabilities through its control callbacks. This gives action replies and streamed updates one control path.

- The signal API supports exhaustive matches on `Part`. Signed-state requests use attribute metadata with an owning node identifier.
- A local action claims its request sequence before Valtron polls it. Browser wake order can differ from request order.

### Feature 09 browser frame reader, 2026-11-21

A child detachment must retain its registry entry. Node deletion must remove the entry.
The parity test now checks reattachment, lookup after deletion and a mismatched parent in both readers.
Resolve a placement child against direct children. A matching grandchild can occur before the correct child.
A typed array needs an aligned byte offset. A frame received in an unaligned network slice uses a copy for its numeric columns.
Chromium initially timed out during target attachment. The protocol engine shared its 30-second command budget with each socket read.
The existing configurable socket timeout from specification 68 fixes the browser harness. The underlying transport issue remains open.

A frame replacement can end a local Rust signal scope. The renderer calls the WebAssembly instance that owns the removed node.


A server frame cannot dispose a Rust scope through a JavaScript observer alone. The local renderer associates each node with its originating WebAssembly instance.
The receiver tracks every root in a fragment. A partial removal keeps the scope alive. The last root releases it.
Dropping an effect closure can drop a captured child context. The graph must release its mutable borrow before running those destructors.

A reconnect can replay more frames than a bounded socket queue can hold. The socket driver sends shared history before the live queue.

### Feature 20. Small frame costs and slot ownership

Both Rust receiver paths now queue the same generated operations. This also reaches callers that selected the columnar receiver directly.
The host can release a browser slot synchronously. The receiver releases the global arena lock before it calls the host.
A single text update measured 136 bytes and 18 allocations with the frame writer. The legacy columnar path used 80 bytes and two allocations.

A callback can submit another native push. One drain loop preserves the sequence while the callback runs outside the queue lock. Issue #634 has two regression tests.

The browser calls a mount callback again when the mount removes its own placeholder. Issue #635 records the interrupted small-frame delivery.

### Feature 15. Android compiler and reader dependencies

The November 2026 Compose release needs Android compile platform 37.2. Platform 36 remains the emulator target.
The Google FlatBuffers Java runtime for tag 25.9.23 is absent from Maven Central.
Gradle builds the tagged source alongside the generated Kotlin. Both use an isolated package to avoid duplicate consumer classes.
The pinned cargo-ndk accepts `--platform 26` for the minimum Android API. Its lowercase package option takes a crate name.

### Native and browser package checks, 2026-11-21

A single bare startup frame needs a different media type from a length-prefixed response stream.
The examples use `application/vnd.ewe.frame` for one frame and `application/primal-frame` for a stream.
A Swift view can retain an old record when a replacement reuses its node identifier.
The simulator exposed that failure during the add flow. Keep the record identity in the view.
The Android demo links the registered Rust actions and native core in one library. Both wrappers must use its callback table.

The generated native wrappers must share the callback registry of the app-specific Rust library. The Kotlin todo selects that library before initialization.
The package-only starter loads the generic native library directly. It needs no generated files or native build.
An idle server-sent event stream needs its own read timeout policy. Normal HTTP action requests keep their configured timeout.

### Named behaviour ownership

A named behaviour belongs to the element that contains its marker. Creating a separate marker node loses that owner.
Bind after a frame inserts its children. A focus group must see its items before it selects the first item.
Keep the mode in the binding key. A component can install two gesture modes on one element.
The four shared browser bindings retain the component algorithms. Unmount hooks release observers, listeners and scroll locks.

### WebView bundle entrypoints

The bundle planner found mode attributes, but the Rust target generator scanned for an older attribute.
Pass the same function path to both generators. A nested-module test covers each supported mode.
Test the generated wrapper with its actual WebAssembly module. A JavaScript load alone cannot verify their shared calling convention.

The host consumes each completed instruction batch. Keep that ownership in the public call.
A reply decoder can consume the return hints after the caller records the expected number of values.
This avoids a second copy of the hints. A pending test future borrows its name until the host schedules another poll.

### Check a package starter on a device

Read the first count before you change the server store. A second client can call the public action to add a todo.
Check the old count again before you tap Refresh. The new count then verifies both the request and the rendered response.
Android 36 needs about 7.2 GB of free disk space to create a new emulator, even with a smaller data partition.

The first iPhone starter check observed a tap without checking the returned count.
Seeding a todo from a second client exposed a truncated request when the headers arrived before its body.
The server now supplies a socket readiness source when the handler takes the request body.
The regression test sends headers, verifies that the handler waits, then sends the body and checks its bytes.

The macro tests build their own dependency tree. A source edit during that build can leave one run with different dependency revisions.
Rerun the affected suite after both crates contain the final change.

The Swift package serves consumers through a repository root manifest. Its distribution URL can remain separate from its development source.

The public Swift manifest can point its targets at the example sources in the same repository.

The Android starter and todo activity repeat host setup. The Android package can own that lifecycle while each app supplies its Rust actions.

A local filter in the server example does not prove offline operation. The on-device test must cover initial rendering, task mutations and count updates with zero network calls.

### Two native execution modes

Rendering the first screen in local Rust removes the server from the on-device startup path.
Keep the device store and its filter in the core across actions. Applications declare controls and supply business actions.
The renderer interprets presentation roles and delivers events with the state attached to each control.
Vocabulary version 3 drops the optional version 4 presentation property and retains the underlying controls.

A transitive Android manifest can add network permission during packaging. Inspect the installed package to verify offline operation.
At accessibility text sizes, the iPhone form uses the large sheet height before the keyboard opens.
The full-height change passed the iPhone simulator tests in dark appearance with large text.

### Mobile styles and public hosts

The Rust template already sends numeric color and font tokens. A theme can change their native values without another frame.
The low-level renderers accept mappings. The public host screens currently create these renderers without passing application mappings.
Swift also renders recognized presentation roles before it applies ordinary node modifiers. Check both rendering paths with the same token.
See [issue 652](https://github.com/ewe-studios/ewe_platform/issues/652).

Each worktree pins its own agent submodule revision. Use the same revision so relative skill links resolve in each checkout.

### Mobile project templates

The platform generator embeds templates from bin/platform/templates during its build. The plain configurator supplies project values without adding Rust workspace settings.

### Release integration

The standalone toolchain proof needs its own workspace declaration for a locked Cargo metadata check.
The Android toolchain test needs both platform directories in its fixture.

### Mobile entry points

Android uses the manifest to select the first activity. Swift uses the application type marked with `@main`.
Both Todo Server entry points supply a Rust core constructor to the screen.

### Mobile WebSocket actions

The browser already sends action frames over WebSocket. Mobile packages can reuse the same frame format and return no immediate reply.

### Connection sharing and resume

The existing mobile channels add stream and cursor values to the endpoint query. Shared connections need subscription-level resume messages.

### Proposed connection sharing defaults

Sharing and replay are separate choices. A late listener can receive new events without changing the position of existing listeners.

### Two transport interfaces

The native action transport exchanges encoded user interface frames. The Connect transport opens an exchange with a response head and streamed body. The adapter must preserve both contracts.

The todo server and browser test both read the Wasm file from the old module workspace. Both need the same new artifact path.

LocalDispatcher already owns the context, dispatcher, and connection. The browser bridge keeps it alive after startup returns.

NativeCore already creates the local dispatcher. Its native transport adapter feeds the same action manager that accepts Rust transports.

Both native event bridges read action references and signed state from the tapped node.

Container identity must survive Rust lowering. Native components can then own child layout without inspecting application-specific strings.

A browser modal must open after its children enter the document. Opening it after the frame applies lets the browser focus the input.
The bottom sheet sends submit to the field that owns the action. Its submit button uses the same field value as the keyboard.
A wire vocabulary change needs a new version row. Older versions retain their original node limits.

The Either macro numbers branch elements before the next sibling. This gives each branch and each sibling a distinct identifier.
Rust builds only the selected children. A selected button keeps its signed state and action on the same node.

A remote signal update can reply from its owner closure. This gives the caller its own mutation result before later updates change the signal.

Thread-safe signal values need isolated dependency tracking and coordinated stabilization. Each process owns its runtime.

The hub acknowledges queue admission. A reply from the update closure confirms the mutation itself.

A type alias preserves existing local imports. Selecting the multithread runtime also changes ownership and callback bounds.

### Feature 29. Reactive ordering across threads

A new dependency can lead to a dirty computation above the observer's previous height.
Refresh that computed dependency during the same pass before returning its value to the observer.
Record the dependency while the value remains locked. This ordering prevents a writer from missing the first subscription.
A successful update publishes its candidate even when equality ignores a changed field. Equality controls notification.

A server advertises its callable actions for the surface in the hello request. The client consults that list when selecting a remote route.

The wire verifier borrows a complete slice. The frame reader must own the input bytes independently of its transport.

Serve supplies the context, incoming request, and connection. A view adapter can wrap the first two and retain the connection for response delivery.

HttpApp exposes its router. A route registration function can accept that router directly.

### Feature 30. Keep the payload until the write

A pure tree can carry a state attachment marker when its caller supplies the state.
The one-argument form of the server html macro returns the tree and its state together.
A full view replaces the contents of the receiving mount. The surrounding application keeps its shell.
Stable region identifiers let later actions update the page, dialog, or count.
The frame writer allocates other nodes above all declared region identifiers.
The HTTP adapter uses a bounded channel between the synchronous body serializer and the asynchronous socket writer.
A serializer failure before the first chunk produces an error response.
A failure after the first chunk closes the response without a successful stream ending.

The state argument in a pure template uses an explicit name, `state = value`.
This preserves the compile-time diagnostic when a receiver argument is missing.
The iOS and Android applications must advertise the complete-view vocabulary before requesting their initial content.

The macro validates shared properties before lowering. Both checks must accept the class attribute.
Native hosts retain the connection and apply the current theme during rendering.
Changing the theme must preserve the identity of the text input.
An Android automated test device can pass interaction tests and still produce a black screenshot.
Use a full system image for visual review. Use the device appearance control for iPhone dark screenshots.

The light entry screenshots show stacked buttons on both platforms. Check the button positions with the keyboard open on each device.

Use the action slots on a Material alert dialog. A column inside a generic dialog stacks its children.
The confirmation check compares the visible button bounds. It also verifies the outcome of each button.

Group sheet buttons when rendering the children. Their parent in the store still identifies the text input.
The iPhone test reads the keyboard frame and captures the full screen, including the separate keyboard window.
The Pixel test reads the visible input-method inset and compares it with the button bounds.

The standalone browser package used release optimization in its test profile. The workspace test profile enables debug information.
The browser build now uses the workspace release profile. The resulting module is about one megabyte.
A native provider returns a concrete UniFFI sender handle. The handle keeps the underlying Swift or Kotlin callback alive.
Returning a raw callback trait from an asynchronous foreign callback failed Rust lifetime checks.

The action context wraps a host context. The host names its typed write method store. The mobile renderers consume thirteen presentation roles.

A successful UI frame exchange verifies only the UI adapter. A separate native application call verifies ordinary typed replies.

Fetch and URLSession decode HTTP content encoding before they return response bytes.
The Connect adapter removes that encoding header after platform decoding. Connect message compression keeps its own header.
A reserved Swift callback name produced backticks in a generated C header. The callback now uses newExchange.
A generated client now joins a base address ending in a slash to one procedure path. Issue 667 tracks the fix.

Portable placement needs view-scoped identity in each renderer. Sharing an endpoint alone cannot make different application targets equivalent.

### Mobile procedure calls

URLSession and Fetch decompress the response body before Rust receives it.
Their adapters remove the consumed HTTP encoding and length headers. The Connect envelope keeps its own compression settings.
A separate service-only router lets each native test verify application data without starting the screen.
An expired replay cursor requires a fresh baseline. Reconnecting with the same cursor cannot recover discarded history.

### Direct host methods and shared controls

A derived host context keeps its existing type and sharing rules when called from an action context.
The host random number generator uses a target-specific mutex. Expose that mutex type through the forwarding method.
A reserved property fails writer validation before the writer filters operations for the client.

Snapshot content and stream cursors must describe the same revision. Separate reads can capture a change under the wrong cursor.

The starter requests a rendered count and a Refresh button. Its route name describes the screen content across platforms.

- A simulator can expose an off-screen keyboard in the accessibility tree. Check its visible bounds before recording an entry sheet.
- The Android binding owns the core library. Each todo app owns its application library. Copying all build outputs duplicated the core library.

- XCTest can attach a hardware keyboard during `typeText`. Taps on software keys keep the keyboard visible throughout the capture.

The ewe_gamify browser host supplies a canvas. Rust opens the device and queue. Compute jobs can run without a surface.

- The TypeScript generator normally writes an empty vector for an absent object field. Action input needs field presence to select its encoding.
  The generation step preserves absence when it packs and unpacks the structured field.
- A typed template binding can infer the action input type from the action type. It does not need an action instance.

A local action can convey readiness and a lease. Safe reuse also requires completion of outstanding device commands.

Feature 28 requires a presence check on the optional structured input field. The TypeScript object generator otherwise writes an empty vector for absent input. Test both field presence and byte content. Decode action input before awaiting the handler to release temporary values on native hosts.

The Mathalley reference separates game rules and scene construction from its browser canvas and audio implementation.

Use one vocabulary revision for each mobile release candidate and its test server. A stale server binary rejected the new handshake. Rebuilding the server restored both Refresh flows. The project generator embeds its files in debug builds so an installed command can run after the build directory is removed.

The debug generator passed a macOS sandbox check that denied access to its source checkout. Use the UniFFI Cargo metadata feature to find each application binding configuration. The Android build copies only its application library for each architecture. Issue #673 records idle CPU use in the reference server.

Two Apple static libraries can each copy a module map into the same Xcode include directory. The starter supplies its application headers through a separate Swift include path. The Android emulator placed the counter button under its status bar. Compose safe drawing insets now keep the button accessible.

### Feature 25. Verify the WebSocket handshake with a client

A valid upgrade response includes a computed acceptance key. Checking the header name missed a wrong identifier in the server.
Using the existing network library fixed the handshake. Two clients can share a socket when the server echoes each capability request tag.


An action uses the stream identifier negotiated with its remote peer. Local handlers keep their local identifier.
The publisher must signal a disconnection to managed listeners. Dropping only the publisher's callback leaves the HTTP response open.
A dedicated subscriber resumes through an authenticated POST after its downstream reconnects. Its delivery URL stays stable.
The browser loads both the application Wasm module and the generated Connect client. Build both after a wire version change.


### Feature 32. Receiving views

A capture from the running todo server supplies the same count frame to the three renderer tests.
Separate device tests also check the route, transport, and rendered count together.
A renderer that serves several views needs a stream position for each receiving view.
Two separate devices cannot detect interference between views that share one renderer.
A named native view owns one node store. A named browser view owns one node registry.
Two listeners can share a connection and still apply the same sequence number in different views.
Check names from columnar operations before removing the outgoing subtree.


### Feature 33. Snapshot recovery

Capture the cursor and state under the same lock. Begin replay at zero when the snapshot includes an empty stream.
Replay can deliver a publication that occurs while the snapshot travels.
Removing an expired listener must preserve other listeners on the same connection.
A late response from an old generation must never replace a new view.
A view with correct content can still display an obsolete connection error. Clear that error after a successful installation.

### Feature 34. Host graphics

Browser graphics objects must stay on their event loop. Native jobs can run on a dedicated graphics thread.
A host can run compute jobs before it creates a surface. Readback needs an explicit device poll on native targets.
A standalone browser graphics object does not need a new Foundation context. The application can install it in its existing context.

Android Vulkan needs an Android display handle when the instance was created without one.
The Android instrumentation test can read a presented SurfaceView pixel with PixelCopy.
A headless Swift package test has no window scene. A readable Metal layer lets it inspect the actual presented texture.
After the Rust queue completes, the test copies one pixel into a Metal buffer. This also works on the iPhone simulator.

### Feature 35. Host device verification

An idle borrowed device can report loss before Rust submits another command.
The native loss callback closes the work queue and returns the result to the host. [Issue 684](https://github.com/ewe-studios/ewe_platform/issues/684)

The iPhone host can keep its display texture separate from the texture that Rust borrows.
The host copies the result to its display after the completion callback.

The live channel belongs to `foundation_wasm_ui::server`. The browser test module also exports this type.
A direct dependency lets the todo server use the channel without linking the browser driver.

### Live stream compatibility

The simulator log records status 400 on repeated requests without bodies after the initial snapshot succeeds.
The running server reproduces this result for a valid live stream greeting with vocabulary version 10 or 11.
The handshake accepts these versions. The live channel previously required version 12 when it attached the listener.
The channel now applies the wire writer's vocabulary rules to retained and new frames.
Each retained frame can share one encoding per surface and vocabulary version. [Issue 688](https://github.com/ewe-studios/ewe_platform/issues/688)

## Shared todo verification

A count update proves delivery of one value. It cannot verify the contents of a list.
Test the row title and completion state on another client after each change.
The current snapshot contains visible items. It omits the last allocated identifier and retained delivery history.
A server restart loses that state.

### Todo list synchronization

The todo mutation reply and live stream use different delivery paths.
A late mutation reply can replace newer content if both paths write to the list.
The stream now owns the list. The action reply owns the dialog for the client that made the request.

Each filter has a stable section and list target. The local action changes section visibility.
The server replaces list content beneath those sections. The server does not need each client's filter.

An active server can export the list from the signed initial frame. The restore option retains the identifiers of the saved rows.
A server restart starts a new replay history. Compare only the saved rows when other users can edit the live store.

The shared browser stylesheet must hide a node even when its regular style uses a flex layout.
The host clears a transient iPhone transport issue after its active delivery accepts a resume request.
An older delivery can return a late resume error. The new delivery keeps its own result.

The Android Google APIs image reports input timeouts when the host uses software graphics during the three-client check.
The Android thread trace enters text layout. The test waits for the full title before it submits the form.
Issue #698 tracks this test environment limit. The check uses the Android Automated Test Device image.

## External drive restart

The copied worktrees retain the original drive paths in their Git files.
The three fresh worktrees use the external repository for shared Git data.
The command `git rev-parse --path-format=absolute --git-common-dir` shows that path in each worktree.

### Android to Chrome and iPhone

The Android test emulator accepts the full title on a fresh start with software graphics.
Chrome exposes the current action on each todo row.
The iPhone uses a check mark and draws a line through a completed title.
Text recognition can misread that title. Inspect the screenshot to verify the completed row.
The simulator keeps the test server address until the next app launch.
After an isolated test, launch the app with the regular server address.
Verify that the saved list appears before ending the test.

## Graphics worker shutdown

The Linux graphics test process can exit with a segmentation fault after it reports all test results.
A native graphics worker can still release its Vulkan resources after the work queue closes.
Wait for the worker to release its resources with `shutdown().await` before the host exits.
The completion signal wakes every service clone. [Issue 696](https://github.com/ewe-studios/ewe_platform/issues/696)

The browser owner reads two queues. Shared work and canvas work use separate senders.
Application code can start shutdown through the shared service while the browser still holds the canvas sender.
The owner must close the canvas queue after the shared queue drains.
It signals completion after it releases the graphics resources. [Issue 700](https://github.com/ewe-studios/ewe_platform/issues/700)

The Arm Linux guest can run all three adapter tests through Mesa llvmpipe.
Use a separate test run to include the adapter tests.
The Vulkan adapter report identifies software rendering in this guest.

### Review the master merge

Check a resolved package dependency against the version of its local crate.
The two branches can update the same crate to different versions.
The higher shared version can satisfy both branches.
Compare the workflow steps by job. A valid workflow can omit a step from one branch.
The wire crate uses the error trace library in its tests. Its production code does not need that dependency.
Cargo accepts dotted dependency keys. A manifest check must recognize them.
The error derive treats a single unnamed field as a source by default.
The graphics text fields contain diagnostic messages. Mark these fields as messages to keep the original error behavior.
The migration test searches parent directories for its fixture.
A build directory outside the checkout has different parent directories.
Use both directory layouts to check the repair for issue #712.

### 2026-11-22 — Native graphics imports

Apple imports a device through Metal. Android imports a device through Vulkan.
The grant constructor needs the same target conditions as those two imports.
Other platforms expose the surface method but have no native window or layer to pass to it.

## Todo target audit

The peer tests check displayed rows. They can pass when a list update still uses a numeric anchor. Inspect frame targets separately.

The dialog checks the device marker only to choose between duplicate mutation handlers. Refresh renders the count screen. It does not exercise data-only cache delivery.

A timed-out submission can already have a committed server record. Retry must retain the original record identifier.

A write reply and a live publication can describe one committed change. Both must enter the same ordered local update path.

### Todo browser assets after a checkout move

The server embeds its build directory in the executable. Moving the checkout can leave the home page available while its module returns 404.
Set `EWE_TODO_MODULE` to select the module at startup. Keep the generated `rpc` directory beside it for browser Connect requests.
The native screens use `/initial` and `/action` without loading the browser module. The data-only server uses its own routes.

The default Cargo test command skips three compute adapter tests because they carry the Rust ignore attribute.
The Linux AMD64 image includes a software Vulkan adapter that runs these tests.

## External worktree paths

A copied worktree can retain Git links to its original drive.
Check the shared Git directory and each submodule after a move.

The live count verifies named replacement for one element. It does not verify target selection for dialogs, lists, or host startup.

A successful version check can hide a moved package when the checker permits missing input. Test missing input explicitly.

### Android tool versions after the package move

The old drift check treated its former default directory as optional. The Android package now lives in `mobile/android`.
Check the real package and exercise missing files in a temporary directory.

A native server can execute a browser-scoped server action. Handler compilation requires the output role as well as the caller scope.

### 2026-11-22. Receiving views and action builds

Capture the receiving view before an event handler awaits its transport. A reply can remove the original control.
A snapshot can carry cache data after its full-view instruction. All three renderers must accept that message order.
A server action's scope selects callers. A web-scoped server action still needs its handler in a native server.
Cargo selects optional platform dependencies before macro expansion. The runtime supplies role macros to the consumer.
An excluded body can then refer to an absent platform library. Shared markup still refers to its declaration.

The browser smoke tests use the public browser macro without a direct template types dependency.
A macro must resolve generated paths through the package that the caller imports. Issue #723 records this case.

A frozen search can contain a row after its authoritative deletion.
Use the pending deletion identifier to remove only that row after saving the updated pending requests.

A temporary transport failure and a rejected title need different controls.
Retry uses the original creation identifier. Editing a rejected title starts a new submission.

Hold an action request at the test proxy to verify the visible state before the server receives it.
A successful retry must increment the confirmed count once.

UniFFI converts foreign exceptions to the declared Rust error when that error implements the callback conversion.
Without that conversion, the failed callback can leave its caller before the caller saves the retry state.

### 2026-11-22 — Capturing pending additions on devices

A held network request can arrive before Android draws its pending row.
Wait for both the request and the visible row before taking the screenshot.
A second delete request for a known deleted record succeeds. This lets a host retry after losing the first response.
Test rejection with an unknown record identifier. Place the initiating element inside its registered view.

Android can continue to show a channel error after live updates resume.
Check the actual row and the channel status separately. Issue #732 tracks the stale message.

The Todo browser test host supplies its staged public directory because Cargo can place test executables outside the server directory.
The native read tests use the installed application bundle and asset manager. The iPhone test runner bundle contains different files.

### 2026-11-22 — Todo identity during retries

The row identifier also identifies its original submission. A deletion receipt preserves that identifier after the row disappears.
Stream positions still order deliveries. Two additions with the same title require separate identifiers.

## Feature 32 backup conversion

Numeric records can grow when a universally unique identifier (UUID) replaces the original number.
Apply the restore size limit to both the input stream and the converted bytes.
Reserve all saved creation identifiers before generating identifiers for other rows.
Keep the original order and the old identifier map. A pending numeric delete needs this map.

### 2026-11-22 — Verify Todo identity on the receiving hosts

The integration test reads the record identifier from each rendered action binding.
It checks uniqueness across records and equality across updates. Each removal leaves the expected row absent.
The typed mobile lookup also exercises the public string boundary with a valid, unknown, and malformed identifier.

A data-only acceptance test needs an isolated service. It does not require an alternate mode in the production launcher.

## Feature 37 native transport selection

The iPhone framework already manages WebSocket connections. The Todo launcher repeats core setup when it selects the sender. Each selection still loads the initial view over Hypertext Transfer Protocol (HTTP).

## Feature 37 Android transport selection

Android already uses one Todo core factory. Its launcher passes two independent options to `RemoteEweScreen`. One transport setting makes the choice explicit.

## Todo startup checkpoint

The server uses a synchronous listener. The Valtron entry attribute supports synchronous functions and keeps the worker pool alive until the function returns.

## Android title input during transport verification

The first Android Connect run displays “Android server” and confirms one item. The test expects “Android server todo”.
A fresh run accepts the full title. Issue #737 tracks the input and submission checks.

## Independent WebSocket ownership

A host action can use the platform network client through its Rust context.
The action socket keeps its own connection when the caller closes an application socket.
The Rust networking crate also provides a direct WebSocket connector.

## Shared identifier format

A constructor can accept an entropy function without adding a context dependency to the compact crate.
The shared identifier preserves the text representation in both backups and action payloads.
A retry after restart uses the identifier from the saved submission.

## Order of nested action updates

An action normally returns its view updates when it completes.
A pending preview needs delivery before the child request starts.
The native test records the number of delivered frames inside the transport send method.
It also checks immediate reply delivery and a transport failure after the preview.

## Calls from a browser action

A host action can remove its submit button before the server replies.
The browser manager captures the receiving view when it starts the host action.
It sends the pending update to that view before the server request.
Each managed socket request uses its sequence number to match its reply.
A retry uses the saved todo identifier and leaves the current dialog open.

## Browser module loading

Chrome reports a compiler error when the compiler receives a missing resource response.
The shared module loader can check the HTTP status before it starts compilation.
It can keep the original compiler or linker error as the cause when the resource exists.

## Android replay verification

Android instrumentation starts a separate process. It must select the same native library as the application.
The replay client must also apply the server capability reply before it sends structured input.
The data service can verify each deletion by its record identifier.

## Browser host registration

The Rust builder can publish its action declarations before it transfers the registry to the dispatcher.
The browser can then install a fresh registry for each startup attempt.
A failed handshake can occur after the runtime attaches a view and a transport.
The caller can retry the same application after the runtime releases both resources.

## Feature 37 browser storage tests

A reload discards listeners on the old document. The new document reports readiness after its Rust module starts.
The IndexedDB provider waits for transaction completion. A successful request can still belong to a transaction that aborts.
The standalone suite uses the pinned wasm-bindgen version from the wasm environment.
Earlier Todo versions store all pending records under one origin-wide key. Todo serves the page and receives its actions at that origin.

## Feature 37 browser pending storage

The browser must keep a receipt after it deletes a confirmed imported record.
An older tab can still write to the original map after a newer tab imports it.
The next boot checks each identifier again. A receipt prevents the confirmed change from returning.

Generated WebAssembly bindings keep memory views and callback finalizers for one instance.
A binding factory gives each boot separate state. Loading one module per boot would retain modules in the browser cache.

A disposed timer registry discards later wakeups. A reused loader needs a fresh registry and memory before it loads another instance.
The browser can queue a view update after its first snapshot arrives. Boot waits for that update before it returns.
The storage bridge carries each encoded Rust value as text. This preserves 64-bit integers when JavaScript moves records into IndexedDB.

A Chromium network trace shows a framework module request on a reused connection. The request receives no response headers within 30 seconds.
The document cannot start its application until all module imports complete. Issue #784 records the trace.
The storage regression uses the browser reload operation. It waits until the new document contains the Todo view.

Six clients also request scripts from the normal Todo server with Python's HTTP client. Three connections stall after 41, 63, and 85 successful requests.
This reproduction uses no browser or storage service. The default server remains in the browser test fixture.

The Todo search uses a generic text input. The iPhone sheet gives only its dim background permission to extend beyond the safe area.

The markup parser and the runtime lowering table both require an entry for each built-in control.
Existing clients can use the search query through the ordinary text field in older vocabulary versions.

The list appends pending additions after it selects confirmed search results.
Both paths need the same case and whitespace rules. Filtering the view preserves the pending operation for retry.

### Android search and simulator checks

Both Android Todo hosts load their native action core from the application library.
The library must support vocabulary version 13 before the server sends the search control.
An older application library requests the ordinary text field for compatibility.

The Pixel emulator received 17 seconds of processor time in 12 minutes under default service scheduling.
The same emulator connected in 47 seconds when its macOS service selected interactive scheduling.

The Android on-device build leaves new tasks pending without a server. [Issue 821](https://github.com/ewe-studios/ewe_platform/issues/821)
Light mode uses white system status icons above a light background. [Issue 819](https://github.com/ewe-studios/ewe_platform/issues/819)

The iPhone test closes its application during cleanup. A successful test does not establish what the owner sees afterward.
Open Todo Server after the last test. Check a new simulator screenshot for the expected task titles after startup.

A Compose color scheme controls the app surface. Each Todo activity also calls the AndroidX edge-to-edge setup for system icon contrast.
Android sets its light status bar flag when it draws dark icons. The flag clears when Android draws white icons.

### On-device Todo actions

The on-device build registered the display actions without a sender for business actions.
An independent local dispatcher can own the task store and send typed confirmations to the display cache.

## Feature 36. Mathalley

The reference separates the game, scene, renderer, and sound score. Its browser host owns the frame loop.
The reference commit is `11052a15411ed1534c2ba9e5f982fd7dc975ffaf`.

## Mathalley sound

The reference separates the Rust score from browser playback. Its playback clock controls note timing independently of frame timing.

The drawing surface owner must present each frame after the game submits its drawing commands.
A game can keep its pipeline between frames and release it without closing the host device.
Pause clears the previous frame timestamp. The first frame after resume advances the game by zero.
A sound session needs its own playback position and lifetime. Two tracks can share a host without sharing their cues.

A navigation test can pass while the graphics worker fails. Check a presented game frame before accepting a game screenshot.
The downlevel graphics limits permit textures up to 2048 pixels wide or high. Modern phone display surfaces can exceed either dimension.
The compositor can discard a browser canvas after presentation. A screenshot preserves the pixels that the user saw.

The iPhone host must finish removing a named surface before a new view can use its name.
The iPhone regression checks both an unsupported resize and a later successful presentation.
Seed 13 uses the same six Train answers on every host. A pixel from the sky identifies Train.
A pixel from the machine base identifies Gumball. A landscape capture can rotate its underlying image.
The Android test checks the visible orientation. The Apple test checks the scene color across the capture.
The Android test device renders game surfaces with Vulkan. The Google APIs emulator also captures ordinary Todo controls.

The game command contains fourteen bytes. The previous host listeners expected thirteen.
All hosts call one Rust decoder that requires the complete string value.


The server test uses a second browser to add a Todo while the iPhone shows the game.
A matching row and count after return confirm that the subscription still delivers updates.
The offline test uses two consecutive game visits to check surface cleanup and Todo state together.
Simulated audio rendering verifies generated samples. A phone is necessary to check its speaker and operating system interruptions.

The graphics replay previously imported generated JavaScript through a public route.
A test can load private bindings from the Cargo output directory and reuse the binary fetched by the application.
This permits a separate device and sound check with the production browser bundle.

The MessagePack prefix for the 14-byte Mathalley route is 0xae. The old hosts compared it with 0xad.
One typed Rust command removes manual length calculations from each host.

Android can present the Mathalley title while its accessibility tree exposes only the return button.
Use a frame capture to check rendered pixels. Check the native surface label separately.

The landscape Android game window can start past the left edge of the display.
The side inset leaves Todo visible at x=0. Frame sampling and taps must use the game window bounds.

Both parent branches declared the game route module at different positions. Git retained both declarations without a conflict.
Building the merged crate exposed the duplicate.

### Native pending records

An old pending file can outlive a confirmed request. A durable import receipt prevents that request from returning after restart.
Conditional database updates protect a confirmed removal when a second screen retries with an older display cache.
The database filename includes the complete old filename. This keeps same-stem files and old SQLite-named files separate.
The database connection needs the host clock. An empty context map preserves that clock without keeping the closed screen alive.
Android can retain the previous accessibility dump across process restarts. Remove the old dump before checking a new screen.

### Confirmed Todo records

A disconnected client can drop its response while the database writes a record.
An executor task must own the transaction through publication.
Encoding the frame before the write prevents a formatting failure after the database accepts a change.
The last position includes deleted records. The server uses that position when it resumes the stream.

The typed view data decoder checks the message topic before it reads the Todo payload.
The server constructors can mount the packaged resources without accepting a browser module path.
A prefix-only proxy exposes root-relative requests that a resource-only browser fixture misses.

### Android drawing surface accessibility

A Compose accessibility label on an Android view wrapper can disappear from the device accessibility tree.
Set the description on the native drawing surface when its label changes.

### Master integration

Both sides bumped crate versions with separate releases. Take the larger version for each crate. Set every path requirement to that version.
A branch refactor that deletes a script must carry the master change into its replacement. The branch keeps the ordered publish through `publish_crates.py`.
A moved directory breaks a script that names the old path. `sync_release_tree.py` now reads the templates under `foundation_ui_project`.
Git pairs a deleted file with a similar file in the other branch. It applied the master patch of `ci.py` to `tests/planner_reference.py`, which is correct here.
The mise environment sets the development profile to Cranelift. Cranelift cannot compile the wasmtime inline assembly. Check the workspace with `--profile uat`.

### A re-export disappears with a major dependency

`foundation_proxy` named `sha2::digest::generic_array::GenericArray` to turn a P-256 coordinate into bytes.
sha2 0.11 does not re-export that type, so the workspace build failed.
Call `as_slice` on the value instead. This works for either array type.
The mise environment sets the development profile to Cranelift. Cranelift cannot compile the wasmtime inline assembly.

## Shared Android library staging

`cargo ndk -o <directory>` copies the library it selects, which was a dependency here.
Build with an explicit `--target-dir` and copy `lib<library>.so` from `<target-dir>/<triple>/release` yourself.
Name the Cargo library separately from the package. The two are equal today, but the artifact name follows the library.

## A public alias must not fix a feature-dependent bound

`ExchangeContinuation` forced `Send` on every non-wasm target, but the split extensions are sendable only with `multi`.
Name the concrete continuation type. Its bounds then follow the feature set of each build.
The host build hid the defect because the default features include `multi`.
Check a crate with the feature set that the consumer uses, not only with its defaults.
