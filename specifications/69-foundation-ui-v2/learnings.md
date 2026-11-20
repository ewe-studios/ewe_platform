# Spec 69 learnings

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
