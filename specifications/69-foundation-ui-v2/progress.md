# Spec 69 progress

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** pull request (PR); user interface (UI); AeroSpace and Defence
Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## Landed

- 2026-10-27: research v1, as PR #157. One file per idea from the rearchitecture document.

- 2026-10-27: rework after owner comment. FlatBuffers became the top format (D7). The research
  split into eleven idea files under `research/`, each with the same five parts.
  `questions.md` holds 25 questions.

- 2026-11-12: the owner's review comments on PR #157 are applied.
  - D8: the HTML morph stays. It becomes `HtmlMorph`, a union member for the web target only.
    `MorphNode` and decision 027 are not retired.
  - `01` now answers "do we not already do this?" in §2. The compile-time id mechanism exists
    on the web. The new part is one schema and native readers.
  - `03` defines `HtmlMorph` in the `Op` union. `02`, `04`, `05` and `11` describe the web gate.
  - The stale bug notes are correct. `TokenSigner::verify` has a constant-time compare and a
    key id since commits `25cab64fe` and `540611b30`. The `docs/bug-backlog.md` file is gone.

- 2026-11-12, second review pass (13 comments):
  - D4 revised: the web comes first, with the `<mount-stream>` and `<mount-data>` paths
    unchanged. Native comes after.
  - D9: MessagePack stays as an opaque body type.
  - D10: one vocabulary. `DomOp` retires. The core names work on every platform, the web adds
    a web-only subset with a string fallback, and mobile reports a name it cannot render.
  - D11: a mobile app may mount a WebView. `ClientHello` gains `mount: Native | Web`.
  - D12: the web state rides as a base64url string.
  - New file `12-scoped-behaviour.md`: scoped JS stays on the web, and each behaviour gets a
    compiled native form.
  - `06` gains the WebView surface and the Rust-to-native command path, and no longer claims
    to rebuild the platform.

- 2026-11-12, third review pass (7 comments):
  - D13: the signed state is a handle, not the truth. The server can ignore the client copy and
    load the authoritative state from the backend. `07` §3.5 is new.
  - D14: single use is a domain rule. The server rejects a second request for the same resource
    (a payment, a delete). No nonce store and no used-id list. `08` §4.2 and §4.4, `08b` §3 and
    §4, and Q19 are changed. Q19 is now answered.
  - D15: the server decides how it handles state. It sends one update or a stream. The signal
    server, the `Broadcaster` and its replay backlog stay, and the research no longer presents
    them as a problem. `09` is rewritten around this, with a plain answer to "how is this any
    different?".
  - `09` title changes to "any replica, no per-user memory". Its current-state table marks the
    signal server, the broadcaster and the transport as keepers.

- 2026-11-12, fourth review pass (6 comments on `10`, all negative):
  - D16: an action returns a logical `Response`. The handler that delivers it decides the
    encoding. Encoding is not the action's business. `10` §1 and §3.3.
  - D17: `Ctx` wraps `foundation_context::Ctx`, so clocks and randomness pass down. `Context`
    wraps `Ctx` and adds action fields. `10` §3.4.
  - D18: actions are not remote procedure call (RPC) procedures. `10` §3.7 is rewritten, Q21 is
    withdrawn, and Q4 no longer ties the transport to ConnectRPC.
  - `10` §3.1 returns to the document's signature. The invented `ctx.authorize`, `ActionCtx`,
    `ActionResult` and `Result<ActionResult<T>>` are removed. The error path is Q28.
  - `10` §3.3 and §3.2 no longer claim a ConnectRPC client stub.
  - `questions.md` gains Q28 (how an action reports an error).

- 2026-11-12, fifth review pass (1 comment):
  - D19: the `#[action]` macro does more than register an action. It reads the signature and
    generates the decode of `Interaction<T>` and the call glue. `10` §3.2 is rewritten. The
    shared steps stay in the library.

- 2026-11-12, sixth review pass (4 comments):
  - D20 (answers Q1): element ids are `primal-id`, as today. An id is not state, so it needs no
    stability rule. A `View` or a `Div` has one fixed id; a custom element uses a string or a
    user-defined id. `01` §3 item 5 and §5 change.
  - D21 (answers Q2): the wire crate is `foundation_ui_wire`. `02` §3.1, §4 and §5 change.
  - D22: one registry model on web and device. The event names the action; the registry finds
    it; the action gets the node, the parent and the data we extract. The action can be local
    or remote.
  - D23: the registry is user-built. The macro generates a struct named after the function, and
    the app registers it. No generated global registry, no fixed list of kinds. `11` §1 and
    §3.2 are rewritten. Q30 asks about `#[remote_action]`.

- 2026-11-12, seventh review pass (30 comments and answers):
  - D24 (Q3): enforce 8-byte alignment. D25 (Q4): the transport is a boundary concern; SSE,
    HTTP, WebSocket and ConnectRPC are all possible. D26 (Q5): stored `foundation_ai` data is
    out of scope. `02` §2 and §5 change.
  - D27 (Q7): retire the hand-written columnar code when the consolidated form delivers the
    same web functionality; one representation for tier 1 and tier 2. D28 (Q8): keep Arrow IPC
    and the other data formats. `03` §3.2, §3.5 and §4 change.
  - Q10, Q12 to Q17 are answered; the recommendations stand. `04`, `06`, `07` and `08` record
    the answers.
  - D29 (Q11): the server, WASM and Rust all send frames; the native side processes them. `05`
    §3 changes.
  - D30 (Q20): use the existing `<mount-stream>` paths and header negotiation; do not
    prescribe a Durable Object. `09` §3.4 and §5 change.
  - D31: an action is identified by its name, not a derived numeric id. `02` §3.3 and §3.5,
    `09` §3.2, `10` §3.2 and §3.6, and `11` §3.2 change. Q22 is removed.
  - D32 (Q23): the framework delivers state updates or DOM/mobile UI updates. D33 (Q24): the
    native update policy is not a current concern. D34 (Q25): build on spec 52 and decide what
    to keep or remove as the work proceeds. Q23 and Q24 are removed; `11` §5 changes.
  - `10` §3.1 states that `Interaction<T>` is typed by its `T`, and `input_value` stays a
    `String` inside that typed struct.

- 2026-11-12, eighth review pass (north star, PR review 5868672793):
  - D35: the north star. One FlatBuffers contract carries structured UI, data and actions
    across web, mobile, WASM and compiled Rust. Do not redesign everything.
  - D36: one cross-platform form set, plus platform-only forms for what the set cannot
    express. A change has one form; there is no core form beside a web or mobile form. Each
    runtime applies the common forms, applies its own platform forms, and skips the rest.
    `02` §3.2 and §3.5, `03` §3.1 and `04` §3.1 change.
  - D37: one action model with a registry in Rust or JS. The web `<script>` functions that
    become callable after mount stay, join the JS registry and never reach mobile. `05` §3,
    `10` §3.8, `11` §3.2 and `12` §3.1 change.
  - D38: platform capability hooks. Shared standard at the top, platform sets at the edges.
  - D39: `mount-stream` and `mount-data` on mobile too, over local, SSE, WebSocket or HTTP.
    `06` §3.7 is new.
  - D40: build on the current shape; keep the agreed parts.
  - New file `13-platform-capabilities.md`: the standard, the shared runtime and the platform
    sets, the message variant rule, the action and behaviour hooks, the live mount contract and
    the reuse map.
  - `README.md` is rewritten around the north star, with the four goals, the layer diagram and
    D35 to D40. Every idea file gains a short north star note.
  - `questions.md` gains Q31: the exact platform scope syntax.

- 2026-11-12, ninth pass (action markers):
  - D37 corrected: an action has two explicit markers. The declaration gives the platform
    scope (`#[action]`, `#[action(web)]`, `#[action(mobile)]`). The reference gives the origin
    (`server:`, `native:`, `js:`). A central action manager reads both and routes the call.
    The two markers are independent and compose.
  - D38 notes the composition: a scope routes a hook to a platform; an origin routes a call to
    a registry or transport.
  - The web `<script>` functions register as `js:` actions and stay web only.
  - `02` §3.3: `ActionRequest.action` carries `server:name`, and the server rejects another
    origin. `05` §3, `10` §3.2, §3.6, §3.8, §4, `11` §3.2, §4, `12` §3.1 and `13` §3.3,
    §3.5, §4 change.
  - Q31 covers the scope syntax and the origin registration. Q30 notes that the `server:`
    origin lowers the need for `#[remote_action]`.

- 2026-11-14: the owner review of the 17 items ends, and all 17 are closed (D41 to D58).
  - Item 16 was measured: `flatc` was downloaded and run; the pin is 25.9.23 (D57).
  - New feature 17, `toolchain-and-ci`, owns every external tool: the pins in the root
    `mise.toml`, the Android SDK task, the host check, the UniFFI proof and the CI steps
    (D58). Feature 01 loses its `flatc` pin to it. The "WebView control" feature becomes 18.
  - The tools are not installed yet. The feature installs and proves them when it starts.

- 2026-11-14: the revision pass folds D41 to D59 into `requirements.md` and all 17 feature
  files.
  - One root frame (D47): `Frame { schema_version; stream_id; sequence_id; kind }`, with the
    `FrameKind` union of `FrameRequest` and `FrameMessage`. One identifier, `EWF1`; one
    content type, `application/primal-frame`. A reader of the wrong kind gives
    `WireError::WrongKind`. Feature 01 writes the schema, the reader and `error_ext.rs`.
  - `Operation`, `Data` and `Control` are tables, and `Message` holds the three (D41, D44,
    D45). The shared set is twelve types, the web set sixteen, the mobile set three (D59).
  - An action returns `Result<(), ErrorTrace<ActionErrorT>>` and writes to `ctx.receiver()`
    (D49, D52). `Response` is removed. `ErrorTrace::new` needs an error type, so
    `ActionErrorT` gains `Display` and `Error` impls in feature 01.
  - The action reference text comes from `action_reference(origin, name)` (D50 (8)): a plain
    action writes `host:name`. The web manager falls forward to `server:<name>` when
    `ServerCaps.actions` holds the name. Feature 05 rejects one name under two origins.
  - Behaviours split by owner (D56): the names and the shared/web sets are feature 02
    (section 6.7), the marker lowering is feature 08, the web registry is feature 09
    (section 6.9), and feature 16 keeps the authoring enum, the component lists, the matrix,
    the build check and the four native bindings. This removes the cycle between features 08
    and 16 and the web features' dependency on feature 16.
  - The example is a cross-platform todo list (D48): `examples/ui_todo` in features 10 and
    11, with server actions, host actions, a live open count and a web page script.
  - Feature 17 gains the `cargo-ndk` 4.1.2 pin (six pins in total).
  - Feature 03 renames to `03-frame-writer`; the "tiers" wording goes with D41.
  - The state travels as a plain `[ubyte]` `EWS1` buffer, not a `nested_flatbuffer`
    (requirements R2; feature 01). A `nested_flatbuffer` makes the frame verifier decode
    attacker bytes before the tag check of feature 04. The reader calls
    `read_signed_state` at the ordered point.
  - The counts in requirements R5 and the feature 02 description are corrected to the D53
    and D59 split: sixteen web forms and the behaviour name tables.

- 2026-11-13: the design specification is written from the owner decisions D1 to D40.
  - `requirements.md`: glossary, the decisions adopted, the spec stories, the context map,
    the three layers, the limits table, the rules for every feature and the Feature Index.
  - `start.md`, and one `start.md` and one `feature.md` for each of 16 features under
    `features/`. Each feature is a story with a bounded context, law worksheets, numbered
    invariants, acceptance scenarios, a technical breakdown with fault tables, a test table,
    the verify commands and a "Done when" list.
  - The eight open questions take their recommendations (A1 to A8 in `requirements.md`
    section 2.2). Nine design refinements over the research are listed as R1 to R9 in
    section 2.3. The owner confirms both lists at approval.
  - Three GitHub issues record what the design work found: #547 (CI does not run the
    JavaScript runtime tests), #548 (the `Broadcaster` backlog has no limit), #549
    (`chacha::encrypt` takes its nonce from the global entropy source).

## Features

Seven of 19 features are complete. Feature 07 is implemented. Its repository clock check remains open under [issue 594](https://github.com/ewe-studios/ewe_platform/issues/594).

| # | Feature | Model | Status |
|---|---|---|---|
| 01 | wire-schema-and-codegen | Sonnet | complete |
| 02 | vocabulary-and-platform-forms | Sonnet | complete |
| 03 | frame-writer | Sonnet | complete |
| 04 | signed-state | Sonnet | complete |
| 05 | action-runtime | Sonnet | complete |
| 06 | action-macro | Sonnet | complete |
| 07 | dispatcher-and-handshake | Sonnet | implemented |
| 08 | html-macro-core-vocabulary | Sonnet | complete |
| 09 | web-frame-applicator | Sonnet | complete |
| 10 | web-action-manager | Sonnet | complete |
| 11 | live-mounts-and-server-streams | Sonnet | complete |
| 12 | legacy-wire-retirement | Sonnet | in progress |
| 13 | native-core-and-commands | Sonnet | complete |
| 14 | native-ios-renderer | Sonnet | in progress |
| 15 | native-android-renderer | Sonnet | in progress |
| 16 | behaviour-matrix | Sonnet | in progress |
| 17 | toolchain-and-ci | Sonnet | complete |
| 19 | native-package-releases | Sonnet | in progress |
| 20 | migrate-to-frame-writer | Sonnet | complete |

Features 08 and 10 are complete. Feature 09 verifies local scope disposal. Implementation of features 11 through 16 is pending. Feature 12 now follows feature 20 for legacy removal.

**Phase gate for feature 13:** not open. It opens when the browser scenario of feature 10
is recorded here as passed.

## In progress

- The owner approves features 17 and 01. Feature 17 starts first.

- The owner reviews the adopted items one at a time (17 items: A1 to A8, five design
  choices, `WasmEnvelope`, `NativeBehaviour`, two known gaps). All 17 items are closed.
  The decision for each one is in `requirements.md` section 2.4 (D41 to D59).
  - Item 12 (R5) is closed by D53: four shared operations, and one web type for each of the
    other operations, with no flag.
  - Item 13 (R6) is closed by D54: the server sends a `ServerAction { hash, name }` list.
    There is no registry hash table, no released-version state and no client list.
  - Item 14 is closed by D55: `WasmEnvelope` goes away for UI operations only and stays
    for the general WASM ABI.
  - Item 15 is closed by D56: behaviours use the shared, web and mobile grouping, with an
    app-defined `custom` string in the web and mobile sets. `NativeBehaviour` becomes
    `UiBehaviour`, and the vocabulary version 2 adds the three enums and their maxima.
  - Item 16 is closed by D57: `flatc` 25.9.23 was downloaded and run. The flags of feature 01
    section 6.3 work, and the generated Rust and Swift writers ignore `force_align` on a
    vector, so the `Aligned8` helper is required. The pin is 25.9.23 because the npm runtime
    has no 25.12.19.
  - Item 17 is closed by D58: feature 17 (`toolchain-and-ci`) owns the toolchain. It pins
    `flatc`, the JDK, Gradle, Kotlin and the Android cmdline-tools in the root `mise.toml`,
    adds the `toolchain:android` task, and adds `scripts/toolchain/check.sh` and
    `scripts/toolchain/proof.sh`. Xcode stays host-managed. The tools are not installed
    now; the feature does that work when it starts.
- R4 is closed by D59: the shared set is twelve types (`CreateNode`, `SetText`, `SetProp`,
  `AppendChild`, `InsertBefore`, `Remove`, `ReplaceSubtree`, `BindAction`, `UnbindAction`,
  `AttachState`, `LiveMount`, `UiBehaviour`). D53 reads on top of it: five `DomOp` variants
  map to four of them, and the other fourteen get one web type each. D59 supersedes any
  earlier text that gives the shared set another size. R7 needs no decision: D50 states the
  same rule.
- The revision pass for D41 to D59 is complete: the 17 feature files and `requirements.md`
  sections 2.2, 2.3, 5.3, 5.4 and 5.6 are updated. The owner reviews the result.

## Current implementation

- Feature 17 passes the full macOS toolchain checks, simulator build and Android debug build.
- Feature 01 provides verified readers, writers, all four generated language bindings and the embedded browser module.
- All 23 Rust integration tests and 11 browser module fixture tests pass. The readers share the same 11 binary fixtures.
- The WebAssembly and standard-library-free builds pass. The generator detects changed output and preserves the last successful output on failure.
- The owner approved the release plan for feature 19. It specifies separate Swift and Kotlin source repositories and public template installation tests.

- The [wire package guide](../../backends/foundation_ui_wire/README.md) documents the schema, actions, manual messages and shared binary fixtures.
- The Rust example runs in a separate consumer crate. The JSON request encodes to binary and decodes with the original fields.
- Each feature now requires a README task and a documentation check before completion.

- Commit `c359d7274` records the wire format, package guide, example and native toolchain.
- Issue #617 fixed two fixture lint errors. All 23 Rust tests and 11 browser fixture tests pass on Linux and macOS.
- The host authentication and compact library tests pass with the shared cryptography dependencies and explicit nonce support.

- Commit `6d4faf280` adds explicit nonce encryption and fallible context accessors. Issue #549 is closed.
- Features 03 and 04 each have a proposed contract change for owner review. Other checks continue.

- Feature 02 defines the vocabulary tables and migration from legacy operations.
  The generated browser tables match the Rust tables.
  All 40 wire tests pass with and without the standard library.
  The browser package tests pass with the embedded vocabulary.
  The [wire package guide](../../backends/foundation_ui_wire/README.md) includes a runnable vocabulary example.

- Feature 04 passes 29 runtime integration tests and its package guide example. The host and WebAssembly checks pass.
- Signed state verifies each field and the tag. The owner must decide how the reader treats equivalent encodings.
- The authentication, keychain, context and generator tests pass. Issue #594 tracks four existing clock reads outside the runtime.

## Next

1. Review [feature 20](./features/20-migrate-to-frame-writer/feature.md). Its implementation depends on browser parity checks.
2. Apply the reviewed migration stages before switching the default browser protocol.
3. Apply the browser acceptance gate before native implementation.


## Revised state and action plans

- Feature 04 now proposes a signer installed in the context. The signer owns its output format.
- Feature 05 follows a counter from registration through a button event to the receiver.
- The existing state tests verify the earlier prototype. The new signer interfaces and examples are design work.

- Feature 04 now shows the same state round trip on the web, iOS and Android. The issuing host owns verification.

- Decision D60 lets the author choose the state owner on web, iOS and Android. A separate trigger can explicitly reference that owner.

## Feature 03 verification

- Version 0.0.3 provides the frame writer, aligned columnar batches and lazy table frames.
- All 87 Rust wire tests pass with and without default features. All 11 TypeScript fixture tests pass.
- Formatting, strict Clippy, WebAssembly checks, binding drift checks and the executable writer example pass.
- The runtime consumer passes 29 integration tests and its documentation example.

- The revised signer plan keeps the optional signing binding in the host context. Render and dispatch apply the same host policy.

- Feature 05 now uses the agreed todo action as its main example. Feature 08 tests its view update and state attachment together.

- Features 04 and 05 now trace the protected todo route from worker startup through session lookup, rendering and action invocation.

- Feature 05 shows the todo page, sibling input, action, protected routes and two consecutive submits in one example.

- The owner approved feature 04. Features 05 and 08 show how the receiver selects the todo list. Only the renderer writes the reserved state attachment.

- The todo route and action share one render function. The author chooses the page or section to emit. Feature 08 tests a section-only render.

- Features 05 and 08 now distinguish the returned tree, receiver output and root placement. The todo helper returns its tree to callers that compose views.

- Decision D61 defines the render boundary. Features 05, 07, 08 and 11 now use one complete render reply and ordered stream delivery.

- Added research study 15 and linked it from the requirements. The action plans now describe HTTP deadlines, socket handoff and asynchronous Rust completion. The renderer plans specify local scope disposal.

- Feature 08 specifies targeted append, insert, swap and removal. Feature 04 explains independent state and view updates with a signed todo row.

- Feature 04 is complete. Runtime 0.0.2 installs context-bound signers. Wire 0.0.4 transports their bytes. Protected route, custom fixture and Wasm entropy checks pass. Feature 05 distinguishes the shared persistent context from each action receiver.

## 2026-11-20. Action runtime and dependent features

- Feature 05 is complete in runtime 0.0.3. The manager routes events. The host invokes a typed handler with its own context.
- The todo test submits twice. It reads the signed attachment from the first reply and supplies it with the next request.
- Checked all 19 feature documents. Updated 01 and 04 through 10, plus 13 through 15.
- Feature 06 is ready for review. Its expansion uses the runtime action module and preserves the function's declared return type.
- The dispatcher, template, web and native plans share the same rules for invocation, signed output and transport replies.
- On macOS, 30 integration tests and three documentation tests passed. Strict Clippy, formatting and the WebAssembly target check passed.

## 2026-11-21. Action macro

Feature 06 is complete. Applications can import `#[action]` from runtime 0.0.4 and register the generated value by name.
The [runtime README](../../backends/foundation_ui_runtime/README.md#register-and-run-an-action) includes a tested counter action.
The paired macro and core packages now use version 0.0.22.

- The native macro and runtime suites pass. The runtime suite includes 15 compiler diagnostic cases.
- The renamed consumer builds for macOS and WebAssembly. The WebAssembly action retains a local reference across an await.
- Strict Clippy and workspace formatting pass. Six generator manifest tests pass.
- Feature 07 is next. Its dispatcher can invoke the registered action with the host context.

## 2026-11-21. Handshake and action dispatch

Feature 07 ships in runtime 0.0.5. Server and local hosts use the same action dispatcher.
The [package example](../../backends/foundation_ui_runtime/README.md#negotiate-a-connection-and-dispatch-an-action) runs a hello and submits an action.

- Accepted connections advertise scoped server actions. Resumed connections keep independent request and reply counters.
- The host selects a render target per request or looks it up for each control on a persistent host.
- Independent replicas exchange signed counter state. Concurrent users retain separate bindings, render targets and receivers.
- Raw view operations cannot supply signed attachments. The regression covers direct, nested and columnar operations. [Issue 628](https://github.com/ewe-studios/ewe_platform/issues/628)
- On macOS, 51 integration tests and five documentation tests pass. Formatting, strict Clippy and the WebAssembly build pass.
- The repository clock check still reports four existing violations in the agent RPC crate. [Issue 594](https://github.com/ewe-studios/ewe_platform/issues/594)
- Feature 08 can queue state targets through the receiver when template lowering begins.

Feature 20 review found a blocking browser parity case before the producer switch.
Child detachment and node deletion need distinct semantics. The current default still uses the legacy protocol. [Issue 629](https://github.com/ewe-studios/ewe_platform/issues/629)

Feature 20 now defines crate ownership, compatibility stages and browser checks. The owner approved the revised plan with the remaining features.

## 2026-11-21. Templates, browser frames and actions

Feature 08 is complete. Features 09 and 10 are in progress in this worktree.

- The template macros emit shared operations. Targeted renders keep the selector and its new subtree together.
- Vocabulary version 3 adds placement and child detachment. The writer filters both for older clients.
- The browser action manager passes 30 JavaScript checks. They cover shared routing vectors, state attachments, transport errors and pending calls.
- The template tests cover all ten shared elements and the five placement macros.
- One registered action appends a row and replaces a section in one frame. The test verifies all signed state values.
- The browser task export passes native tests for reverse-order wakes, teardown, and allocation ownership.
- The todo example passes its browser checks. Feature 09 also checks disposal of local signal scopes.

After features 08–10, proceed with the remaining features in dependency order. Feature 12 requires feature 20.

### Feature 09 browser verification, 2026-11-21

The browser applies EWF1 frames through its existing node registry. Both encoders agree on all 19 legacy operations.
The Rust test confirms the committed fixtures. The browser checks row and column encodings for every eligible shared operation.
Chromium received the Rust counter frame over server-sent events. A morph kept focus and released only the discarded local scope.
The browser and component JavaScript suites passed. The optional legacy megatron binary was absent.
The `foundation_wasm_ui` and `foundation_browser` UAT Rust tests passed. The strict `foundation_wasm_ui` Clippy check passed.
Issues [547](https://github.com/ewe-studios/ewe_platform/issues/547) and [629](https://github.com/ewe-studios/ewe_platform/issues/629) are closed.

The macOS toolchain check passes under mise, including Xcode, the Android packages, Gradle and the FlatBuffers compiler.

## Browser action acceptance

The feature 10 todo scenario passes in Chromium. A local WebAssembly action opens the dialog.
The server adds a todo and updates the live count. Local filtering and server toggling both work.
A second delete of the same item reports the domain error. The browser task passes all 19 smoke tests and the todo scenario.
This result opens the feature 13 native core gate.

The compiled WebAssembly fixture verifies external signal cleanup for feature 09. Features 11, 13 and 20 are in progress.


The compiled WebAssembly fixture registers a real Rust scope on a button. A remote frame replaces the button.
The `ewe_ui_dispose_mount` export runs its registered cleanup once. Removing the replacement leaves the count unchanged.
The local bridge isolates native threads. Removing a captured child scope releases the graph borrow before its destructor runs.

Feature 11 passes four Rust tests for live delivery and replay. Feature 14 shares six binary fixtures with the Android renderer.

### Feature 20. Rust producer migration

The default local and server transports write EWF1. Templates, signals, slots, the theme and mock receivers use generated operations.
The custom protocol example and external renamed-template consumer compile. Receiver tests cover failed delivery, sequence continuity and slot release.
All 19 operation constructors match the saved frame fixtures. The package guide records release measurements for one, four and sixteen text updates.

The native core passes eight application tests, its README example, both mobile target checks, and binding generation.
The core preserves frame order during concurrent pushes and listener callbacks (issue #634).
Swift and Kotlin renderer work has started. The release build now assembles device and simulator libraries.

## Web wire retirement checks, 2026-11-21

| Check | Command | Result |
|---|---|---|
| G1 and G2 | `mise run ui-web:test` | Browser fixtures cover all 19 old operations and equivalent columnar rows. |
| G3 | `cargo test -p foundation_wasm_ui --profile uat --test live_channel_tests` | All four channel tests pass. |
| G4 | `mise run test:browser` | Two Chromium pages receive the live todo update. The server disconnects both. Both resume. |
| G5 | `cargo test -p foundation_wasm_ui --profile uat --test app_tests --test reactive_structure_tests --test slots_tests` | All 31 tests pass. Each captured batch has the same ordered effects on both wires. |
| G6 | `EWE_UI_WIRE=frame mise run test:browser` | The browser smoke suite passes. The local and streamed button have identical markup. |
| G7 | Component behaviour tests through the installed frame runtime | All 13 assertions pass. |

Feature 11 and feature 20 are complete. Feature 12 can now remove the old encoders.

### Feature 15. Android package

The sibling package contains the core, binding and renderer modules.
The Kotlin reader matches all six native snapshots. The Kotlin tests cover gaps, event state, caps, commands and mount ownership.
The native binding builds for ARM64 and x86-64. The Compose renderer builds against Android 37.2.
The starter resolves the three local Maven artifacts by coordinates. Device verification is in progress.

### Post-retirement and native checks, 2026-11-21

Feature 12 removes the old browser and Rust encoders after the seven recorded checks.
The four Rust package suites passed 187 tests. A compile-fail test verifies that the retired operation type has no public import.
The rebuilt general WebAssembly fixture sends a host message and releases its slot.
The final frame-only browser suite passed on Chromium and Firefox. Two browser pages also passed the live todo test.

Both native todo applications passed the add, complete, filter and delete flows on the simulator and emulator.
The Android test repeated a deletion. Rust returned the expected action error.
Feature 19 validates 15 Maven inputs and the Apple device, simulator and macOS archive.
Public package verification still requires the first Git tags and Maven publication.

### Feature 15 completion on 2026-11-21

The Android API 36 emulator ran the Kotlin todo and the independent package-only starter. The todo added, toggled, filtered and deleted an item.
The live count changed from zero to one and back. A second delete returned `ActionFailed already deleted`.
The starter resolved the local Maven candidate, displayed zero, and refreshed to one after another app added a todo.
Kotlin source revision `fc42a81cf668084857ba65f503f6476e97a34ef6` is ready for release staging.
The final ARM64 and x86-64 binaries match their binding archive members byte for byte. The emulator and the temporary server stopped.

### Feature 16 browser and matrix verification

The three Rust matrix tests check the authoring identifiers, component usage and marker placement.
Both native source checks found all four shared bindings. Chromium moved focus after the frame installed a focus group.
The browser tests compare each shared binding with its component source. They also verify the binding registry against the matrix.
Strict Clippy passed for the wire, runtime, component and WebAssembly UI crates.

### Native iOS renderer and release candidate, 2026-11-21

Swift source commit `577a64abdb86793ace5bd50abcda36e8888b9f8b` passed 11 tests and matched the six Rust snapshots.
The iPhone 17 simulator ran the todo flow with Xcode 27.2. An outside tap, Escape and a downward swipe each closed the sheet.
The package-only starter imported EweUI. Its first Refresh check observed new control identifiers.
A later check changed the count before Refresh. The host reported a truncated request body. The visible count check is in progress.
The native package manifest records the three source commits, wire versions, package coordinates and binary digests.
The first local release preview uses core commit `07f90ee83`. Its Apple archive checksum starts with `1c339daa`.

### 2026-11-21 — WebView example bundles

The bundle command builds all three platform examples from their mode attributes.
The Android package contains the same frames, wrappers and WebAssembly modules as its public assets.
Chrome loaded the two Android source pages, their two packaged copies and the iOS page.
Each page rendered its heading. An Android user agent also rewrote the application link on all five pages.

Feature 16 passed 53 component tests, three matrix tests, 68 runtime tests and 91 reference-enabled wire tests.
The four shared behaviours have bindings on both native platforms. The web suite passed 160 tests with one optional fixture skipped.

### 2026-11-21 — WebAssembly host checks

The default and web feature sets pass strict Clippy for all targets in `foundation_wasm`.
The package matrix now repeats both checks. The web feature passes 79 Rust tests.
The rebuilt runtime fixtures pass 74 host integration tests and 145 interface integration tests.
Twenty-one optional tests skip because their older fixture is absent.
Issue [#598](https://github.com/ewe-studios/ewe_platform/issues/598) tracks the remaining testbed matrix coverage.

### Feature 19 Android starter runtime check

The standalone package starter passed on a new Android 36 ARM64 emulator.
Gradle resolved the three modules from the Maven candidate. The app displayed zero open todos.
A second client added one todo through the public action endpoint. The starter displayed one after the test tapped Refresh.
The staging script stopped the Rust host and removed the temporary device. Public resolution runs after the first publication.

The release tests built the Swift starter from a clean checkout and the Android starter with an empty Gradle cache.
The Android device check saw zero, added one todo through the real action endpoint, then saw one after Refresh.
Kotlin release input `a23e916f12a851ecca9d60fb4e13495912d1321f` adds the default branch to its test workflow.

### Feature 19 iPhone starter and request body check

The standalone Swift starter passed on an iPhone 18 Pro simulator running iOS 27.
The app displayed zero. A second client added a todo. Tapping Refresh changed the count to one.
The Android and iOS checks both used the public action endpoint and the local package candidate.

The HTTP server now waits for unread request bytes when headers arrive first.
All six connection tests passed. Three additional requests with delayed bodies each returned HTTP 200.
[Issue 648](https://github.com/ewe-studios/ewe_platform/issues/648) records the fix.
Public package installation requires the first release.

### Feature 12 final verification and feature 19 local candidate

Feature 12 is complete. The application and browser use the frame writer and reader.
The migration example builds a custom transport and reads its text update. Its package guide documents both interfaces.
The final macOS run passed 211 tests across the macros, signals, WebAssembly, browser and todo crates. It reported 42 existing ignored tests.
Five focused HTTP suites passed 38 tests. The full macOS HTTP suite has a repository exception under [issue 546](https://github.com/ewe-studios/ewe_platform/issues/546).
The additional connection suite passed six tests. The seven parity checks preceded removal. The migration spanned more than four commits.

The release preparation command built both mobile artifacts from core commit `686761cd9`.
The local candidate manifest is `target/native-ui/candidate/native-ui.json`.
It records three source revisions and both artifact digests.
Both standalone starters built from these artifacts. Swift passed eleven package tests. Android completed all 36 build tasks with an empty Gradle cache.
Public release signing and installation remain open.
Seventeen of the nineteen indexed features are complete. Feature 07 records its open clock check. Feature 19 awaits publication.

Feature 19 has a proposed monorepo source layout and iPhone todo design. Both await owner review.

Feature 19 now specifies GitHub Releases and Swift package tags in ewe_platform. Implementation awaits plan approval.

Features 21 and 22 specify the installable libraries under mobile/ios and mobile/android. Feature 19 releases both. The example apps consume them.

Features 21 and 22 now specify four runnable todo apps. Each platform has Todo Server and Todo On Device. The package-only starters remain release checks.

### Features 21 and 22. Mobile packages and four todo applications

Both libraries and all four applications now live in ewe_platform. Both packages passed clean consumer builds from candidate `8c5243b26`.
The Swift package passed eleven tests. The Kotlin library passed thirteen tests.
The Rust suites passed 92 wire tests, one server test and one offline action test for both platform configurations.
The browser integration suites passed 145 and 160 tests. Each suite skipped one optional fixture.

Todo On Device rendered its first screen and completed its task flow on each simulator with the test server stopped.
Android also verified that the installed application has no network permission.
Todo Server ran on both platforms. A second client read the saved task. Each on-device app kept its own store.
Both package-only starters displayed zero, sent Refresh, and displayed one after a second client changed the server store.
The iPhone simulator tests also passed with dark appearance and accessibility-extra-large text.
The unsigned physical iPhone build passed. Physical device execution was outside the simulator test run.

The monorepo candidate is in `target/native-ui/mobile-release-preview/`. Its manifest records both archive digests.
Swift built from a clean candidate package. The Android starter executed all 36 build tasks using a fresh Gradle cache.
Feature 19 still requires public publication and public dependency resolution.
Nineteen of 21 indexed features are complete. Feature 07 retains its repository clock check.

### Feature 23. Native design and themes

The project now has local design skills for Android and iPhone.
Feature 23 proposes native defaults, application themes and checks for both host types.
The design preview and baseline captures are ready for owner review.
Nineteen of 22 indexed features are complete. Features 07, 19 and 23 have open work.

The specification now links to both skills in the main project’s shared agent submodule.
