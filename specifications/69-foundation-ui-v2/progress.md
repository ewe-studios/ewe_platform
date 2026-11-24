# Spec 69 progress

## Todo Server launch on iPhone

Issue [825](https://github.com/ewe-studios/ewe_platform/issues/825) covers the installed server address and the launch screen.
Todo Server stores its server address at build time. A launch argument can override it.
The simulator tests sign the app with a local signature. The app declares its supported orientations.
A test with an invalid server address shows the configuration error.
A fresh signed test bundle showed the [Todos launch screen](features/23-native-design-and-themes/design/ios-todo-launch.png), then all three tasks from port 8181.
The installed iPhone 17 build also showed the title and all three tasks without launch arguments.
Issue [831](https://github.com/ewe-studios/ewe_platform/issues/831) tracks the simulator launch service timeout during first installation.

## Resume work on the external drive

Use the project root at `/Volumes/Outside/Dev/ewe_platform`.
Keep each active checkout under its `.worktrees` directory.
Keep build caches, install prefixes, and verification outputs under `/Volumes/Outside/Dev/ewe_platform/target/agent-work/`.
Commit and push each work checkpoint. Verify the remote commit before stopping.

| Agent | Worktree | Branch | Assignment |
| --- | --- | --- | --- |
| Tasks | `.worktrees/combined` | `resume/combined` | Continue the Specification 90 build tasks and bundle generator. |
| Todo | `.worktrees/combined` | `resume/combined` | Continue the native release task migration. |
| Graphics | `.worktrees/mathalley` | `work/69-mathalley` | Implement Specification 69 feature 36. |

Use the feature progress files to select the next check.
The combined branch includes the checkpoints from both retired worktrees.

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

## Historical feature checkpoint

The following table records an earlier stage. See the [completion audit](completion-audit.md) for current status.

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

### Mobile project templates

Feature 24 proposes two mobile templates for the existing project generator. Each supports a server or on-device Rust host. Implementation awaits review.

### Release integration checks

The feature branch now includes current master and release changes.
Cargo resolves 116 workspace packages. The release script verifies 22 nested lockfiles.
Three macOS key provisioning tests pass with age 0.12.
The release checks still need to pass on all three systems.

### Todo application walkthrough

Part 1 records the Android and iOS entry points. The next part follows the screen into the Rust core.

### Mobile WebSocket actions

Feature 25 proposes WebSocket action delivery for Swift and Kotlin. The specification and the walkthrough link to the delivery contract.

### Connection sharing and resume

Feature 25 now specifies shared connections on all three platforms. Each subscription resumes through a protocol message.

### Proposed connection sharing defaults

Feature 25 now defines the sharing attribute and its transport defaults on all three platforms.

### ConnectRPC transport proposal

Feature 26 proposes host network adapters for Connect remote procedure calls on mobile and WebAssembly. Implementation awaits design review.

Feature 27 proposes one Rust package for the todo server, browser module, and mobile library. Implementation awaits review.

Feature 27 now proposes a browser LocalApp builder that accepts the application context and action registry.

Feature 27 now covers mobile startup and a Rust client that calls a remote Foundation service.

Features 26 and 27 now describe direct transport injection and native provider lookup.

Feature 23 now specifies explicit container tags and per-tag components. The owner approves implementation of this section.

### Explicit container tags in Feature 23

Vocabulary version 5 adds Modal and BottomSheet. Overlay keeps its existing identifier.
Rust now sends the same Add dialog tree to all three platforms. The browser example no longer copies a named form field in JavaScript.
Each mobile tag has a dedicated component that renders its supplied children. The browser opens a native modal dialog.
The Rust runtime passed 68 tests. The Rust wire package passed 88 tests. All 12 TypeScript fixture checks passed.
Chrome verified measured layout, background input, modal focus, Escape dismissal, button submission, and keyboard submission.
Both todo examples passed their application flows on iPhone and Android. The native offline Rust test also passed.
The JavaScript suite passed 146 tests. One legacy test skipped because its optional fixture was absent.

Feature 08 implements Either in the shared markup macro. The todo toggle now selects its action inline.
The runtime passed 70 tests. The browser macro and reactive structure suites passed 41 tests.
The todo action and native offline tests passed. Compiler tests cover invalid branches and conditions.

Feature 28 proposes typed action input with String as the default. The signal-backed todo store remains a discussion example.

Feature 29 proposes an optional thread-safe signal runtime for native applications. The design awaits review.

Feature 29 now requires combined-build tests for queued hub updates and per-action replies.

Feature 29 now names SingleThreadedRuntime and MultiThreadedRuntime. Both create signal scopes. Runtime remains an alias for the local type.

### Feature 29. Thread-safe signals

Both named runtimes now expose scope creation. The new native writer returns the result of each atomic update.
Both runtimes share the height-ordered work queue. The local type alias and hub remain available.
All four package configurations passed on macOS. The browser todo flow passed in Chrome.
Linux and Windows cross-checks passed. Their native test runs remain part of the configured continuous integration matrix.

Feature 05 now specifies local-first lookup on both web and mobile surfaces.

Feature 30 proposes reusable frame readers and a simpler HTTP route surface for the todo example.

Feature 30 now separates HTTP route attributes from the proposed view integration crate.

Feature 30 assigns explicit and attributed route registration to Router.

Feature 30 defines Router::mount for handwritten and generated route handlers.

Feature 30 now gives initial routes a full-view response contract and leaves placement targets to explicit updates.

The owner approves the Feature 30 view-handler integration. Implementation starts with its route and response dependencies.

Feature 30 adds HttpApp::mount as a direct delegate to Router::mount.

Feature 30 requires lazy response bodies and independent tests for source inspection and rendered output.

The owner assigns Features 23 through 30 to one implementation agent. Finish and verify Feature 30 first.
Then review Features 23 through 29 and complete the remaining work one feature at a time in dependency order. Feature 29 already records completion.

### Feature 30 implementation

The wire reader now accepts files and asynchronous byte streams. Both readers use one decoder.
The shared router registers a method, path, and handler. Native adapters accept ordinary functions.
The view handler selects HTML, a frame, or JSON in its return value.
The body writer retains the request scope and writes each body once.
The todo server registers its routes without reading or writing frames. A successful mutation publishes the open count.
The browser integration test passes with the installed Chrome executable. It covers add, cancel, delete, filter, and live replay.
The iPhone simulator passes all three offline and server application tests.
The Android emulator passes startup, mutation, replay rejection, and offline store isolation.
The HTTP, wire, runtime, and signal suites pass. The generator reproduces the checked-in bindings.
Feature 30 is complete.

Feature 23 now requires shared utility classes and platform-specific overrides from one theme.

### Feature 23. Shared classes and native hosts

Vocabulary 7 carries the class list on a shared control. Earlier vocabulary rows stay fixed.
One Rust theme supplies the browser stylesheet and the mobile utility manifest.
Both public hosts accept a theme. Changes preserve the screen connection.
Android uses a Material top app bar, segmented buttons, list items, and checkboxes.
The runtime and wire suites pass. Chrome verified computed color and padding on a real todo row.
The iPhone and Android application checks cover local and server actions.
Visual checks and device tests for a live theme change continue.

The revised entry sheet design places Cancel and Add on one row on iOS and Android. The user approves the entry sheet layout on both platforms.

The Material confirmation dialog now places Cancel and Confirm beside each other.
The Pixel checks passed in light, dark, and branded appearances.
Each check compares the button positions, cancels once, and then confirms deletion.
The [device captures](features/23-native-design-and-themes/feature.md#device-captures) show each result.

Both entry sheets now group Cancel and Add below the input. The controls keep their node identifiers.
The iPhone and Pixel tests verified both buttons above the software keyboard in all three appearances.
Cancel discards the typed draft. The submit button and keyboard retain the original input binding.
The six entry captures show the revised layout. The Pixel test also checks its keyboard inset.

Feature 23 is complete. Twenty Swift simulator tests and six Android device tests passed.
The on-device and server applications passed on both platforms after the sheet change.
The browser verified theme changes on the displayed row. The wire, runtime, and theme suites passed.
The accessibility checks used the platform control trees. No person reviewed synthesized speech.
Feature 27 starts with the single todo package. Keep the three platform outputs and existing application calls.

### Feature 27. One todo package

The server, browser and both mobile hosts build from one manifest. The public mobile constructors keep their names.
The browser loaded the new Wasm module and ran local and remote actions. Both iOS and Android application suites passed.
The Rust network client passed immediate and deferred negotiation, local service access, and invalid reply tests.
The native provider looked up each service once. Two actions used the same native handle.
Feature 27 stays open for the Connect service provider from Feature 26.

Feature 31 proposes shared control tags. Feature 05 proposes direct host service access. Both design changes await approval.

The user approves direct host service access in Feature 05 and all control tags in Feature 31. The current implementation agent owns both changes.

Feature 26 now requires an ordinary typed service call from Swift and Kotlin through their native network clients.

### Feature 26. Connect through a platform client

The generated Actions and Todos clients use the same platform HTTP transport.
The example serves ordinary typed data on a separate router with one procedure.
Android instrumentation received task 42 and a typed NotFound error from that service.
The Android Connect screen passed add, filter, delete and isolation checks.
The generated streaming test received two messages before the response ended.
Swift and browser verification continues.

Feature 32 defines named placement for all applications. The todo migration verifies one live endpoint across three platforms.

Swift and Kotlin each called the generated GetTodo procedure through the native network client.
Both received task 42, “Buy milk”, and a typed missing-task error for 999.
The service ran without a screen or a Foundation frame.
Both mobile application suites passed. Chrome passed remote actions, local actions, and event-stream replay.
Features 26 and 27 are complete. Work starts on direct host methods in Feature 05.
Issue 668 tracks view recovery after live replay history expires.

Feature 05 exposes the host context methods on the action context. Applications and tests now use those methods.
The runtime passed 75 integration tests, nine documentation tests, Clippy, and its WebAssembly build check.
Feature 31 adds twelve node kinds. The existing Modal and BottomSheet composition covers the thirteenth control.
The wire tests and the Android library build pass. Device and browser checks continue.

Feature 33 proposes automatic recovery for a view after replay expires. Issue 668 remains open until platform verification passes.

Feature 32 specifies two shared screen routes. The full todo screen uses /initial. The count screen uses /open-count.

The user approves Feature 32 with both shared screen routes. Implementation is queued.

Android ran all six renderer tests and both todo applications after the control migration.
The nine new captures preserved the approved layouts. Each theme passed the entry and confirmation button checks.

Feature 31 is complete. The iPhone 17 ran 20 Swift package tests and all three theme reviews.
The entry screenshots show the keyboard, Cancel and Add. The shared controls passed on web, iOS and Android.

Feature 34 proposes optional host graphics services on web, iOS, and Android. It awaits review.

The user approves Feature 34. The implementation agent adds it to the existing queue.

Feature 28 adds structured input to actions. Text remains the default.
Rust accepts a second input type. The browser, Swift, and Kotlin bridges expose explicit structured submission.
The wire fixture records nested input. Rust tests cover signed state, unit state, aliases, and decode failures.

Feature 35 proposes existing device access. Rust requests the device. The host invokes the configured local Rust action when ready.

Feature 28 is complete. All 78 runtime tests and 97 wire tests pass. Swift passes 21 simulator tests. Android passes its unit and emulator suites. Browser action tests, WebAssembly compilation, generated files, and strict Clippy checks pass. Feature 24 is in progress.

The user approves Features 28 through 35 for implementation. Feature 24 is in progress.

The user approves Feature 25 for implementation after the current mobile template work.

Feature 36 proposes Mathalley as a second todo view. The user schedules it after every other feature.

Feature 24 generates four mobile projects. The packaged archive contains 63 files, including the binary Gradle wrapper. Both remote starters pass the Refresh check. The iPhone simulator and Android emulator each show the server count change from zero to one. On-device application checks are in progress. The first public command installation depends on registry release issue #672.

Feature 24 is complete against local package candidates. Both generated on-device counters changed zero to one with networking disabled. The Apple build produced device and simulator slices. Android built both supported application binary interfaces. The generator, package, macro, and existing template tests passed. Strict Clippy passed. Issue #675 is closed after the Android safe-area check. The public mobile installation remains with Feature 19. Feature 25 is in progress.

### Feature 25. Shared connections

Two browser action managers used one live socket to the todo server. The server assigned each screen a separate session.
One screen added a todo. Only that screen received the three reply messages.
The focused browser suite passed 20 tests. The Swift package passed 21 simulator tests. Both Android unit test tasks passed.
Subscription replay, connection lifecycle tests, and native network checks remain in progress.


Feature 25 is complete. Chrome tested local actions, remote actions, and replay after a stream disconnected.
Two independent WebSocket clients shared one connection. They received their own replies and replayed from different cursors.
The server also delivered replay through server-sent events (SSE) and a length-prefixed HTTP response.
The iPhone 17 simulator and Android emulator submitted a todo over WebSocket.
Both platforms also received targeted replay errors through all three live transports.
The Swift package passed 23 tests. Android passed core tests and built the Compose library.
The browser suite passed 152 tests and skipped one. All 95 Rust wire integration tests passed.
The runtime, native core, HTTP adapter, and six live feed tests also passed.
Feature 32 is next. Feature 19 still requires release access for public package verification.


### Feature 32. Portable view targets

The browser, iPhone simulator, and Android emulator displayed one count update from the same running server.
Both mobile starters loaded `/open-count` and refreshed the count. All three full applications loaded `/initial`.
The common placement fixtures passed on all three renderers.
Each renderer applied the same frame to two views with identical element names and node identifiers.
The second view retained its count after the first closed. Columnar and table placements rejected duplicate names before replacement.
The browser suite passed 153 tests and skipped one. The iPhone simulator passed 24 tests.
Android passed its core tests and built the Compose library. Rust passed 79 runtime tests and 100 wire tests.
Rust also passed 11 runtime documentation tests and ignored two examples.
The first Feature 32 stages cover framework placement and the count. The Todo target migration remains open.


### Feature 33. Recover a receiving view

The browser, iPhone 17 simulator, and Android emulator recovered after the server discarded the requested history.
Each client started at one. Each loaded exactly one replacement snapshot through revision four.
The test published five before sending the snapshots. Each client applied that missing update and displayed five.
All three clients passed with a typed stream error and again with an HTTP 410 response.
Android also cleared its temporary connection error after recovery.
Thirty concurrent snapshot requests matched the committed count during twenty adds.
The Swift package passed 26 tests. Android passed core tests and built Compose.
The browser suite passed 156 tests and skipped one. Feature 34 is in progress.

### Feature 34. Host graphics in progress

- The Mac runs the shared compute shader on Apple M5 Metal. Its output is 1, 5, and 15.
- The wire vocabulary now includes the DrawingSurface element. All 101 wire integration tests pass.
- The native core exposes the host graphics service through its application context.
- The iPhone device and simulator libraries build. Browser and mobile presentation checks are in progress.

### Feature 34 complete. Feature 35 starts

- Chrome WebGPU, iPhone simulator Metal, and Android emulator Vulkan ran the shared shader.
- Each renderer passed two-surface pixel checks, zero-size suspension, stale handle rejection, and target recreation.
- The Mac Metal device passed loss and reinitialization. All five Rust graphics checks passed.
- The wire suite passed 101 checks. The JavaScript suite passed 157 checks and skipped one existing check.
- The todo browser module rebuilt. Its Chrome action, streaming, and reconnect checks passed.
- The graphics and native crates passed Clippy with warnings denied. No physical phone was connected.
- Feature 35 now adds requests for a host-owned device and replies through a registered local action.

### Feature 35. Host device verification

- The Mac passes nine graphics tests, including loss of a borrowed device and cancellation during a replacement.
- The iPhone 17 simulator passes the original Metal queue, texture, visible preview, and local refusal tests.
- The Swift package passes 29 tests on the simulator.
- The todo application imports its live channel from `foundation_wasm_ui::server`.
- The server uses the stream handler from `foundation_ui_http`. Only browser tests enable the browser driver.
- A fresh iPhone launch receives status 200 for the live stream and status 204 for the resume request.
- The investigation continues for the reported status 400. Browser and Android graphics verification also continue.

### Live stream compatibility verification

- Seven live channel tests pass. They cover replay and new publications for an older iPhone vocabulary.
- The todo server passes end-to-end subscription checks for version 11 over server-sent events and version 10 over streamed HTTP.
- Both requests resume on the open delivery session. The server still rejects a request that names another session.
- Chrome passes the server and local action test after the live route moves out of the browser test module.
- The iPhone 17 simulator opens the live stream with version 11 and version 12 in separate application runs.
- Both runs receive status 200 for delivery and status 204 for resume. Add, complete, and reopen work without manual reconnection.
- The original server on port 8080 keeps its in-memory todos during these checks. Each test uses an isolated server.

Feature 07 also passes its last check. The repository time source check reports zero violations.

## Host graphics and shared todo checkpoint

Feature 35 passes its graphics checks in Chrome, the iPhone 17 simulator, and the Android emulator.
The native Rust suite passes eight tests. The Swift suite passes 29 tests.
The Android graphics tests verify the final texture on the displayed surface.
Issue #692 tracks a separate Android dialog test failure. Physical phone coverage remains open.

The shared todo server publishes the open count. Connected clients also need the changed rows.
The sync repair owner handles the framework changes and the browser-to-phone test.
Port 8080 keeps the current session and its data during this work.

### Todo list synchronization

- Issue #693 tracks missing list changes on clients that subscribe to the todo stream.
- The server puts the list projections and open count in one ordered frame.
- The receiving view keeps its filter, dialog, and live mount during list changes.
- Browser and mobile checks exercise mutations from a second client and replay after a disconnect.
- The worktree server restarted on port 8080 with all four existing todo identifiers, titles, and completion values.
- Chrome and the iPhone simulator verified live row creation and completion on port 8080. The temporary check row was removed.
- The isolated iPhone test verifies draft and filter preservation plus replayed deletion without pressing Reconnect.
- The two-browser check, 95 wire tests, todo action test, and streamed replay checks pass.
- Chrome and the iPhone each added, completed, reopened, and deleted a todo. Both peer platforms received the changes.
- Chrome, iPhone, and Android each recovered by replay. The test recorded three initial snapshots and six resume requests.
- Android-origin checks remain open. The Android emulator stalled during text entry on the Google APIs image.
- The Android test image run stopped before the Android-origin sequence completed. Issues #693 and #698 remain open.

### 2026-11-22 — Android to Chrome and iPhone

The Android test emulator starts all four actions on one isolated server.
The table shows the row named `Android peer todo` on each receiving client.

| Android action | Chrome row | iPhone 17 simulator row |
| --- | --- | --- |
| Add | The row has a Complete button. | The row has an empty circle. |
| Complete | The row has a Reopen button. | The row has a check mark and a line through its title. |
| Reopen | The row has a Complete button. | The row has an empty circle again. |
| Delete | The list has no matching row. | The list is empty and shows zero open todos. |

The server reports three live connections after deletion.
Each client loads its initial view once.
Both receiving clients show all four changes without manual reconnection.
These checks complete the two remaining directions in the six-direction test.
Issues #693 and #698 contain the results and are closed.
After the test, the iPhone simulator connects to the regular server on port 8080.
The restored screen shows five open todos.

## Graphics worker shutdown

The base Linux Advanced Micro Devices 64-bit (AMD64) test process exited with status 139 in four default-concurrency runs.
A serial run on the same binary passed.
The fix adds an asynchronous shutdown method and a test for concurrent waiters.
The fixed Linux AMD64 suite passed three default-concurrency runs. Each run passed seven tests and skipped three hardware tests.
The full crate checks and native Linux 64-bit Arm (ARM64) checks remain open.

### External drive verification on 2026-11-22

Branch `resume/69-graphics` starts at `35939b13e`.
Its worktree uses the Git directory on the external drive.
The browser build at this checkpoint fails with Rust error E0063.
The browser constructor omits the worker completion receiver. [Issue 700](https://github.com/ewe-studios/ewe_platform/issues/700)

The repair closes the canvas queue when the shared queue drains.
Both queues finish their accepted work before the owner releases its resources.
A browser test starts shutdown from a service clone while the browser host remains open.

The WebAssembly library and its browser test program compile.
The full Chrome graphics test passes on the Apple Metal 3 adapter.
The shared service completes the accepted canvas request before shutdown returns.
The next canvas request fails as expected.
The screenshot samples match red, blue, and blue.

Native Arm Linux passes seven default tests and three adapter tests in separate runs.
The adapter report identifies Mesa llvmpipe as a software renderer.
On macOS, the Metal adapter also passes all ten tests across those two runs.
Clippy rejects two unnamed default values in the lease tests.
The test now names both graphics descriptor types. [Issue 701](https://github.com/ewe-studios/ewe_platform/issues/701)

After the lint repair, two native Arm Linux runs each pass all ten tests together at default concurrency.
Both runs use `cargo test --profile uat -p foundation_ui_gpu --test integration -- --include-ignored`.
Strict Clippy passes on native Arm Linux, Linux AMD64, and macOS.
The macOS repeat passes seven default tests and all three Metal adapter tests.

The full Linux AMD64 crate check passes formatting, build, strict Clippy, tests, and the WebAssembly check.
The default integration test passes seven tests and skips three adapter tests.
The workspace manifest still reports three unused dependencies. [Issue 596](https://github.com/ewe-studios/ewe_platform/issues/596)

### 2026-11-22 — Integration with master

The local candidate merges preview `dc3776fa4` and master `17e64db06`.
Its merge commits are `d2cce054e` and `1307ac73a`.
The review covers 96 package manifests, 12 lockfiles, four Rust files, and one workflow.
All 118 workspace packages resolve with the locked versions.
All local version requirements match their workspace packages.
The workflow retains all 11 changed feature steps and all seven changed master steps.

- Issue #702 covers the clock methods on the action context. The repository check now recognizes these approved methods.
- Issue #703 covers the native error derives. The error types retain their names, variants, and message text.
- Issue #704 covers development dependencies. The repository check now counts error traces in test sources.
- Issue #705 covers the graphics error derives. The graphics service uses the workspace derives.

All 131 infrastructure tests pass. The browser script suites pass 172 tests and skip one.
The native, wire, web handler, and remote call suites pass 129 tests and ignore one documentation example.
The keychain and graphics suites pass 85 tests. A separate macOS run passes all three graphics hardware tests.
The action context test passes for the forwarded clock methods.
The runtime suite passes 79 integration tests and 11 documentation tests on macOS. It ignores two documentation examples.
The macro and browser crates pass 44 unit tests.
The macro integration suite passes 54 tests with an external build directory.
The remaining migration test passes when Cargo builds inside the checkout.
Issue #712 tracks the external build directory failure.

### 2026-11-22 — Native graphics checks on Linux

Issue #713 records two Linux build warnings in the native graphics bridge.
Only Apple and Android build the device grant constructor. Both platforms use it to import a device.
Other targets return the existing incompatible surface error. That path now ignores both the target and the graphics instance.
The Linux AMD64 gate passes formatting, the all-targets build, and strict Clippy at `e0640f9cf`.
Linux passes eight default, ten app, and eleven tasks integration tests.
Each configuration passes one documentation test and skips the builder example.
macOS passes eight default integration tests and one documentation test. It skips the same example.

## Todo target audit

Feature 32 migrates the live count. The Todo agent owns the remaining target migration under issue #714.

The revised Feature 32 plan assigns mutations to the server. Host actions own dialogs, cached search, filters, and game navigation. The revision awaits approval.

The owner bans numeric anchors in application code. Issue #714 covers removal of the remaining Todo anchors.

The review adds optimistic submission, frozen search results, and a separate one-minute block demonstration to the proposed design.

The proposed full Todo page renders record changes from the local store. Server writes return and publish the same typed change.

The proposed optimistic flow covers Add and Delete. The local store tracks each pending request until reconciliation.

### Todo browser module after a checkout move (issue #715)

`EWE_TODO_MODULE` selects the browser module at server startup. The server checks the file and header before it prints the address.
On macOS, five temporary servers verified normal startup, bounded replay history, backup restoration, native startup, and data-only requests.
The first three served the same 1,001,876 module bytes and the generated Connect client from the external checkout.
The restore check found the saved test title in `/initial`. The data-only check returned task 42 with a missing module override.
Both Rust action tests and both JavaScript transport tests passed on macOS.

One local action renders the full Todo page rows and count from the same store revision. The store confirms a pending todo after applying its committed change.

### Complete Linux AMD64 graphics run

The project Linux image runs the graphics test binary from commit `776b98b6c` with `--include-ignored`.
Mesa supplies the software Vulkan adapter. All ten tests pass at default concurrency. No tests skip.

## Todo target migration status

Twelve application constants still assign numeric targets from 40 through 51.
The Todo agent owns removal from shared rendering, actions, and host startup.
Issue #714 stays open until the migration passes renderer checks.

## Specification 69 completion review

The [completion audit](completion-audit.md) covers every assigned feature number.
The audit separates current checks, earlier platform runs, and pending work.
The shared Todo renderer now has one entry point. Feature 10 no longer directs callers to the retired browser wrapper.

### Feature 17 Android tool version check

Issue #720 fixes the repository check for Android tool versions. The workflow checks the wrapper and catalog in `mobile/android`.
The current Gradle and Kotlin versions pass. Missing files and mismatched versions fail the check.

Feature 06 proposes compiler selection for action bodies. The current macro already provides runtime scope selection.
The owner approves the extension. The Todo agent owns the compiler changes and external consumer checks.

### 2026-11-22. Todo target migration and action output roles

The combined worktree has one Todo registration list and one dialog declaration for each intent.
The shared page uses named targets for the list, filter, count, dialog, search, and notification.
A committed change carries data to the receiving view. The host applies it to the local cache in order.
The Android core passes 18 tests, including named-view allocation. Compose compilation passes.
The iPhone simulator passes 29 tests and a separate named-view allocation test.
The macOS Swift suite passes 24 tests with the local task-enabled library and zlib.
The JavaScript suite passes 158 tests and skips one test.
The browser bundle check passes in Chrome for the readable and compact files.
The action macro keeps metadata on each output. The runtime selects browser, mobile, and server implementations independently.
All four external dependency graphs select the expected platform libraries. Each release artifact contains only its eligible handler markers.
The native server and combined builds each pass two dispatch tests. Chrome sends `server:add_todo` from the browser-only build.
The application uses one registration list. The browser release of Todo builds with local pending storage.
The server keeps the original position for a repeated deletion.
Pending Todo checks include durable retries, restart recovery, and the updated three-host run.
Issue #714 stays open.

Feature 27 proposes build-time staging beside the executable and one server constructor.
The revision moves the seeded data service to test support. It awaits approval.

The Feature 27 proposal now uses one asset declaration for development disk reads and embedded release files.

The browser test build found a public macro dependency failure in issue #723.
The browser package now exports its template types. The macro uses that export when the caller imports the browser package alone.
The compiled Chrome fixture passes for both runtime bundles after the fix.

### Feature 32 search during a pending deletion

The host removes its confirmed deletion from frozen search results. Unrelated results stay visible until the query changes.
The restart test keeps the original creation identifier and displays the pending title and retry button.
Four Todo action tests and three native dispatch tests pass on macOS.
Chrome adds, completes, reopens, and deletes a row on the isolated server.
The JavaScript suite passes 158 tests and skips one. The updated three-host run is pending under #714.

The native restart test now covers a server validation error after retry.
The app preserves the title, shows the server message, and lets the user edit the rejected submission.
A corrected title gets a new creation identifier. The pending store removes the rejected entry in the same write.
All three native dispatch tests pass.

A fresh full runtime run passes 80 tests, including the action and view compiler tests.
Chrome passes the delayed addition, failed request, retry, and confirmed deletion checks.
Feature 32 records the pending and retry screenshots.

Android now reports a foreign transport exception through the Rust error result. Issue #729 records this fix.
The callback conversion test passes. The Android app displays **Try again** after HTTP 503 and retries the saved addition.
All three hosts then display the new record, its completion, its reopening, and its deletion.

### 2026-11-22 — Todo changes reach all three hosts

One isolated server served Chrome, an Android Pixel emulator, and an iPhone 17 simulator.
The installed three-host test passed in 175.534 seconds.

| Origin | Add | Complete | Reopen | Delete |
| --- | --- | --- | --- | --- |
| Chrome | Both phones showed Browser peer todo. | Both showed Reopen. | Both showed Complete. | Both removed the row. |
| iPhone | Chrome and Android showed iPhone peer todo. | Both showed Reopen. | Both showed Complete. | Both removed the row. |
| Android | Chrome and iPhone showed Android peer todo. | Both showed Reopen. | Both showed Complete. | Both removed the row. |

Both phones showed pending additions before server confirmation. Each received a temporary server error and showed **Try again**.
Each retry produced one confirmed row. The browser kept its draft and selected filter during peer changes.
The iPhone resumed live delivery after disconnection and applied the deletion.
The test logs are in `target/agent-work/todo-logs/threehost-{peer,xcode}-before-resource.log`.

Chrome also passed the held-deletion test. The pending row showed **Deleting…** and retained the confirmed count.
A temporary error showed **Try again**. The successful retry removed the row and changed the count from one to zero.
The browser action regression passed over HTTP, WebSocket, and Connect RPC. Its separate external-peer test requires a live server.

The second run used the rebuilt native resource bindings. It passed the same three-host test in 185.201 seconds.
The four native pending and retry screenshots are in the Feature 32 screenshots directory.
The peer log and Xcode log are in `target/agent-work/todo-logs/threehost-peer.log` and `threehost-xcode.log`.
The Android screenshot also shows a stale channel error after successful reconnection. Issue #732 tracks that status message.
The separate one-minute block demonstration remains open under issue #714.

The owner approves one UUID per Todo in Feature 32. The Todo agent owns the record, protocol, binding, and fixture updates.

### Feature 27 resource verification

The Todo icon has identical bytes in Chrome, the installed iPhone simulator app, and the installed Android emulator app.
Both native readers also identify missing hosts, invalid paths, and browser-only resources.
The moved development server serves disk edits. The release server embeds its assets and runs alone.
Specification 90 Feature 3 records the resource implementation, package checks, and installer dependency.

### 2026-11-22 — One identifier per Todo

The submitting host generates a version four universally unique identifier (UUID) from injected entropy.
Pending additions, confirmed records, deletion receipts, and the typed data service use that same identifier.
The server rejects missing, numeric, and malformed identifiers. A delayed Add after deletion leaves the record absent.
Four action and startup tests and three native tests pass. Both socket replay and concurrent snapshot tests pass.
The Android data procedure returns the requested UUID and rejects a numeric string. The three-host run is in progress.

Feature 32 now documents routed child actions and direct transport errors. The next owner handles implementation.
Commit `2fc68c85a` already removed the separate notification marker.

## Feature 32 saved Todo list

The server restores version 4 universally unique identifiers (UUIDs).
Use `ui_todo --convert-backup SOURCE NEW` once for a numeric backup.
Save its identifier map with the two backup files.
Three command-line tests pass on macOS with the `uat` profile.
A separate loopback server restored eight converted records from a private copy of the live backup.
The GetTodo procedure returned each saved title and its new identifier.
All eight completion states and record positions matched the source.

Feature 32 assigns UUID generation with injected entropy to the shared UUID module. Implementation belongs to the next owner.

### 2026-11-22 — Three-host UUID acceptance

Chrome, the Android Pixel emulator, and the iPhone 17 simulator shared one isolated server.
Each host added, completed, reopened, and deleted its own record. Both peers received each change.
The test passed in 212.168 seconds. The browser recorded the same identifier through all four changes.

| Origin | Record identifier |
| --- | --- |
| Chrome | `fac5c420-80f2-4486-bae3-96ae757085f4` |
| iPhone | `d028b3cc-deb1-4b34-9c42-129adce51fc6` |
| Android | `47e076d3-ca6e-4928-a38d-627305db3547` |

Each phone retried a failed addition. Chrome separately retried both an addition and a deletion.
Both native data clients returned the requested UUID, reported an unknown UUID, and rejected a numeric string.
The browser regression passed over HTTP, WebSocket, and Connect RPC. Only the submitter saw its confirmation.
Seven Rust action and mobile tests passed. Both concurrent snapshot and socket replay tests passed.

The device log is `target/agent-work/todo-logs/uuid-threehost-xcode.log`.
The row log is `target/agent-work/todo-logs/uuid-threehost-peer.log`.

The packaging contract assigns Todo browser module production, validation, and staging to the build. Runtime migration remains pending.

Feature 27 requires the server build hook to produce the browser application and Connect bindings from a clean checkout. Implementation remains pending.

Feature 32 explicitly retires the custom Todo outbox when the shared action and storage path passes its acceptance checks.

Feature 32 assigns wire encoding and decoding to the framework. Todo handlers supply typed data or markup.

Feature 32 extends the simplification review to the entire shared Todo directory. The implementation remains with the next owner.

Feature 27 requires one deployed JavaScript entry and one framework resource handler. The current per-file routes await replacement.

Feature 37 consolidates the Todo redesign. The production startup branches still exist. Implementation and acceptance checks remain open.

Feature 37 now assigns browser startup, storage adapters, and transport wiring to their framework owners. Implementation remains open.

Feature 37 selects IndexedDB for durable browser Todo submissions and assigns both browser storage providers to `foundation_db`. Implementation remains pending.

## Feature 37 native transport selection

Feature 37 now specifies one iPhone startup path for all three action transports. Implementation and native verification remain pending.

## Feature 37 Android transport selection

Feature 37 requires all three transport tests and the independent WebSocket test on both Android and iPhone. Implementation and verification remain pending.

## Todo startup checkpoint

Both native Todo hosts now select one action transport and use one core factory. The Rust server uses `#[valtron]`. All three backup command tests pass. Native transport checks continue.

## Feature 37 native transport verification

Commit `e23b24019` passes the three transport checks on the iPhone 17 simulator and Android emulator.
Each test loads a fresh server view, opens the local dialog, adds a todo, completes it, and reopens it.
Each test checks the confirmed count. Each transport uses a separate server process.

- Swift runs `TodoTests/testAddTaskToFreshServerStore` through `xcodebuild test-without-building`.
- Android runs `scripts/ui_native/check-android-todo.py` with the `server` argument.
- Rust runs `cargo test -p ui_todo --test backup_cli --profile uat`. All three tests pass.

The first Android Connect run submits a partial title. The repeat passes. Issue #737 tracks this result.
The independent native WebSocket service remains pending.

## Independent native WebSocket service

Rust actions can open a host WebSocket through `AppWebSocket::open`.
Swift uses URLSession. Kotlin uses OkHttp. Each socket has its own lifetime.
The iPhone simulator and Android emulator pass the independent socket test.
Each test exchanges text and binary messages with an echo server.
Each closes that socket and receives a rendered count from the Todo action socket.

Run `TodoTests/testIndependentWebSocket` with `EWE_SOCKET_ECHO` and `EWE_TODO_URL`.
Run Android instrumentation `ReplayCheck` with `socket_echo` and `server`.
Supply a WebSocket echo endpoint and a running Todo server for each test.

## Shared Todo identifiers

Todo uses the version four identifier from `foundation_compact`. The host entropy source supplies its bytes.
The backup converter and pending Add use the same constructor. Todo no longer needs `shared/id.rs`.
Two identifier tests, four action tests, three backup tests, and three mobile Rust tests pass.
The browser WebAssembly target passes its Cargo check. The next action change moves pending Add into the host submission action.

## Typed child calls from native actions

A native host handler can now call `ctx.trigger_structured("server:save", &input).await`.
The framework publishes queued view updates before the send. It uses the screen's existing action manager and transport.
Immediate replies reach the screen listener. A socket handoff can finish before a reply arrives.
The handler receives routing and immediate transport errors through its Rust result.

All 11 native core integration tests pass on macOS. The network application test and the browser compilation check also pass.
Browser child dispatch and the shared Todo submission actions remain pending. The existing Todo send adapters still run.

## Feature 37 shared submission actions

The browser and mobile clients use the same host actions for Add, Delete, and Retry.
Each action saves its pending change, renders the preview, and calls the server through its context.
The browser runtime handles reply matching and child errors.
The mobile core uses the selected transport directly.
The browser manager tests, Chrome action test, and mobile Rust tests pass.
Browser startup, storage providers, and resource packaging still need work.
The iPhone simulator and Android emulator each added and updated a todo over all three transports.
The checks covered HTTP, ConnectRPC, and WebSocket.

## Feature 37 browser module loading

Todo uses the shared module loader for its browser module.
The loader reports the address and HTTP status for a missing resource.
The loader tests cover missing resources, invalid module bytes, and a valid streamed module.
The framework task regenerated the readable and minified bundles. Its check passed.

## Feature 37 three-client verification

Chrome, iPhone, and Android each sent changes that reached the other two clients.
The live test kept the receiving client's search and unfinished draft during updates.
The iPhone and Android tests retried a failed Add with its saved identifier.
A separate Chrome test retried failed Add and Delete requests and checked the confirmed count.
The Android replay client also submitted two Delete requests for the same identifier.
It checked both replies and verified that the data service returned NotFound after each request.

## Feature 37 browser startup

Todo now calls createApp with its view, initial route, delivery action, and transport selection.
The framework installs the browser host, registers Rust actions, negotiates the connection, and opens the view.
A separate browser application used default and custom transports in Chrome.
It recovered from a failed handshake and shared one startup operation between concurrent callers.
Closing the application removed the view and closed its custom transport.
The readable and minified runtime bundles both export createApp.
Todo still has browser storage imports. Its deployed entry still loads separate JavaScript modules.

## Feature 37 browser storage providers

The optional `browser-storage` feature in `foundation_db` supplies IndexedDB and localStorage.
Three Chromium tests pass for a standalone Rust consumer.
Both stores retain concurrent writes after two tabs reload.
Deleting one record preserves the other record.
The tests cover access denial, quota exhaustion, transaction abort, and empty memory storage.
The macOS database suite passes 255 tests.
Todo still uses the legacy browser callbacks. Issue #728 remains open until application tests verify independent pending changes.
The standalone consumer dependency check excludes the user interface runtime and the Cloudflare Worker crate.
Feature 37 scopes pending changes to the Todo server. Older browser records use the server at their original browser origin.

## Feature 37 browser pending records

The browser runtime installs the configured store during startup.
Todo selects IndexedDB and uses its server origin as the namespace.
Each pending Add or Delete has a separate record.
Startup validates the old localStorage map before it writes any records.
IndexedDB commits imported records and their receipts together. The old map remains available to older tabs.

The Linux database runner passes 535 tests across its configurations. Its package formatting check passes.
The browser runtime JavaScript suite passes 159 tests and skips one test.
Two Todo tests pass for shared connections and concurrent snapshots.

The macOS Chromium database suite passes four tests. The tests include transaction rollback and concurrent writes from two tabs.
The live mobile-peer test needs a separate server and remains disabled by default.
Two tabs submit different changes at the same time. Both changes survive reload and independent retries with their original identifiers.
Migration tests cover a corrupt source, a transaction abort, concurrent import, a late source write, and receipts after confirmation.
The lifecycle test boots all three providers from one cached binding module. It also retries after a failed boot.
The JavaScript loader suite passes 78 tests and skips 20. The eight loader-specific tests pass.

The native browser runtime passes 114 Rust tests in the `uat` profile. Two documentation examples are disabled.
The Todo release build produces the application bindings and all three WebAssembly modules.
The checked-in runtime bundles match the output of `ewe-wasm-bundle runtime --check`.
The tests use the working tree based on `b05f4a5a8`. The final macOS run includes documentation changes through `32379fa98`.

All changed Rust files pass the formatting check. The workspace check reports existing differences in 26 unchanged files.
Issue [#782](https://github.com/ewe-studios/ewe_platform/issues/782) tracks the loader restart fix.
Issue [#784](https://github.com/ewe-studios/ewe_platform/issues/784) tracks a module request that waits for HTTP response headers during Chromium navigation.

The native and WebAssembly Clippy checks finish successfully. Todo still reports existing warnings in its shared and theme code.

| macOS check | Command |
|---|---|
| Todo release modules | `examples/ui_todo/build-module.sh` |
| Runtime Rust tests | `cargo test --locked --offline --profile uat -p foundation_wasm_ui` |
| Todo integration tests | `cargo test --locked --offline --profile uat -p ui_todo --features browser-tests,mobile --no-fail-fast -- --test-threads=1` |
| Storage in Chromium | `mise exec -- target/uat/wasm-testbed bindgen backends/foundation_db/integration/browser --headless` |
| Runtime JavaScript tests | `node --test backends/foundation_wasm_ui/integration/test/*.test.js` |
| Loader JavaScript tests | `node --test backends/foundation_wasm/integration/test/*.test.js` |

The standalone storage test starts Chromium with popup windows enabled. The native Todo test sets this browser option itself.

The final default-server Todo run passes 14 tests on macOS and ignores one live mobile-peer test.
The two browser storage tests pass in 29.03 seconds. The server and host action test also passes.
The new test targets verify concurrent confirmation and migration separately. Issue #784 remains open for the independent HTTP stall.

The owner reports search styling defects on both mobile platforms and a gap under the iPhone entry sheet. Feature 23 contains the proposed revision.

Todo now declares search with the shared SearchInput control. Vocabulary version 13 adds this control.
The wire suite passes all 96 tests. The JavaScript runtime suite passes 160 tests and skips one.
Both mobile renderers are under test.

Issue #820 tracks pending additions that ignore search. Both Android Todo hosts now filter pending titles and restore them after clearing.
All three Rust mobile tests pass. The runtime suite passes all 80 tests.

### Android search verification

The Pixel emulator passes the search control test. The test checks editing, keyboard submission, clearing, restored values, and disabled controls.
Todo Server displays two tasks with an empty query. Search for “Review” displays the matching task. Clear restores both tasks.
The server test uses a local relay that rejects writes. Both original tasks remain on the shared server.

Compare the [light screen](features/23-native-design-and-themes/design/android-search-light.png), [open keyboard](features/23-native-design-and-themes/design/android-search-keyboard.png), and [large dark text](features/23-native-design-and-themes/design/android-search-dark-large.png).

### iPhone search and sheet verification

Both iPhone Todo hosts pass their search checks. The on-device test runs in light and dark appearances.
The large-text test checks that the keyboard leaves both sheet buttons visible.
The sheet reaches the physical bottom edge when the keyboard closes.
After the tests, the iPhone and Pixel show both original tasks from the server on port 8181.

Both Android Todo activities now select system bar icon colors from the current system appearance.
The Pixel shows dark status icons on the light surface and white icons on the dark surface.
The server host passes in [light appearance](features/23-native-design-and-themes/design/android-status-server-light.png) and [dark appearance](features/23-native-design-and-themes/design/android-status-server-dark.png).
The local host passes in [light appearance](features/23-native-design-and-themes/design/android-status-local-light.png) and [dark appearance](features/23-native-design-and-themes/design/android-status-local-dark.png).

### On-device Todo actions

Issue #821 connects the on-device Todo core to an in-process action service on both mobile platforms.
The local and network senders use the same display core factory.
Both local screens forward typed action results to the receiving view.
The Pixel emulator passed Add, Complete, filter, Reopen, and Delete in airplane mode.
The open count changed from zero to one after Add and returned to zero after Delete.
The deleted title disappeared from the device tree. The Pixel then returned to Todo Server and displayed both original tasks.
The iPhone 17e simulator passed the same workflow. Its test also verified that the Pending label cleared after Add.
All four Rust mobile tests passed. The new regression test checks the actual operations each platform receives.

## Feature 36. Mathalley

The owner approves Feature 36. Work starts on branch `work/69-mathalley` at `279a5fa2b`.
The reference commit is `11052a15411ed1534c2ba9e5f982fd7dc975ffaf`.

## Shared audio proposal

The owner approves the Feature 36 host sound session. The graphics agent owns implementation and playback checks.

### Mathalley integration checkpoint

The game runs on branch `work/69-mathalley` in `.worktrees/mathalley`.
Five Rust tests pass on macOS. A seeded run completes both games, three rounds, the tally, and restart.
The other tests check letterbox input, paused time, cues, volume, and independent sound sessions.

Chrome opens both games through the shared entry. A second client changes the count while the game is open.
Chrome retains the selected filter across two opens. Its independent sound output test passes.
The browser host copies a presented game pixel to its own texture. The Rust device lease exchanges red and blue.
The host reads the changed pixel from the same texture.

The iPhone 17 simulator presents the game twice and returns to the selected filter.
A second client adds a task while the iPhone displays the game. The server-backed test passes.
The iPhone drawable measures 1206 by 2241 pixels. Issue #722 tracks the old device limit of 2048 pixels.
The repair requests the adapter resolution limit and rejects unsupported surface sizes before configuration.

The Android application displays Train on the Arm emulator with the Vulkan graphics backend.
Two open and close cycles preserve the task and filter. The application also survives a background and foreground transition.
Browser, iPhone, and Android game captures are in the Feature 36 screenshots directory.

Chrome, iPhone, and Android each receive a task from a second client while Mathalley is open.
Each host shows both tasks when the user returns. The iPhone and Android tests use simulators.
Seed 13 selects two Train levels followed by Gumball. All three hosts display both scenes and receive answer input.
The iPhone and Android tests rotate the game. The rendered Gumball scene survives the change.
The screenshots directory contains the Train and Gumball scenes for each host.
The iPhone screenshots also include Gumball in landscape.
The iPhone Metal regression passes with a large drawable and rejects an unsupported resize.
All ten native graphics tests pass on macOS. Strict Clippy passes for the mobile and WebAssembly example builds.

Each native sound test compares seeded music with the same music plus the intro cue.
Both hosts preserve mute across a pause and resume. Closing the game releases only its own output.
Apple renders samples from the second view through its audio engine. Android advances playback through its audio track.
The browser uses Web Audio. Physical phone speakers and operating system interruption notifications still need a device review.

### Mathalley and offline Todo integration

The merged mobile hosts share the game command decoder and the local Todo service.
The signed iPhone server test opens the game, receives a peer update, and restores the filter and count.
The signed iPhone 17e passed both offline tests on commit `e114627ee`. Two game visits preserved the selected filter and Todo.
Train and Gumball passed the seeded input and rotation test. The captures show both scenes and the restored list.
The independent Apple audio test also passed. Physical playback and interruption tests remain open.

The Todo build now packages the Mathalley browser library with the other application resources.
Its JavaScript bindings remain private. The game requests its binary when the player opens its view.
The browser regression check uses the production bundle and records resource requests across two game visits.
The package task `todo:test-android-game` now runs the Android capture check.

Issue #834 blocks delivery of the Mathalley action. The old hosts reject the 14-byte string because they expect 13 bytes.
Rust now uses one command type for both encoding and classification.
The standalone regression accepts the real action and rejects malformed messages.
The signed iPhone 17e opens Mathalley, keeps the Todo subscription, and passes the independent sound test.
Chromium opens the game from the production bundle and returns to Todo. Android verification remains in progress.

The integrated package passed six Mathalley tests and four mobile Todo tests after the fix for issue 840.

### Native pending records

Issue #728 replaces the native pending file adapter with the framework key-value store.
Each Todo server has its own SQLite file. Each pending request uses its original operation identifier as the key.
A transaction writes each imported record and its receipt together. The old JSON file remains available for recovery.
Four database tests passed. They cover rollback, concurrent connections, import receipts, and timestamps from the host clock.
The SQLite provider passed release checks for Android arm64 and the iPhone arm64 simulator. Application tests are in progress.

### Confirmed Todo records

The server saves each submission, receipt, current value or deletion, and stream position in one record.
The maintenance command imports confirmed records and converts old backups.
All seven durable store tests pass. They cover restart, malformed content, cancellation, concurrent writes, encoding failure, and database errors.
All ten live channel tests and three maintenance command tests pass.
Three independent server processes used the same temporary database. The second process served the original item through the initial page and GetTodo.
The third process rejected a lookup after deletion. Retrying the original creation kept the tombstone at position two.
