---
feature: "legacy-wire-retirement"
description: "Prove that the frame wire delivers the same web function, switch the defaults to it, move the internal code from DomOp to the generated ops, then remove the old envelope, encoders, parser and DomOp."
status: "pending"
priority: "medium"
model: "sonnet"
depends_on: ["08-html-macro-core-vocabulary", "09-web-frame-applicator", "10-web-action-manager", "11-live-mounts-and-server-streams"]
estimated_effort: "large"
created: 2026-11-13
last_updated: 2026-11-13
author: "Main Agent"
crates: ["foundation_ui_traits", "foundation_ui_wire", "foundation_wasm_ui", "foundation_macros", "foundation_arrow", "foundation_ui_components", "foundation_browser", "foundation_signals", "foundation_wasm"]
decisions: ["D8", "D9", "D10", "D27", "D28", "D29", "D40"]
tasks:
  completed: 0
  uncompleted: 14
  total: 14
  completion_percentage: 0
---

# Feature 12: Retirement of the old wire

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** user interface (UI); Document Object Model (DOM); JavaScript
(JS); WebAssembly (WASM); application binary interface (ABI); inter-process communication
(IPC); continuous integration (CI); identifier (id); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

After this feature, the UI has one wire. The frame wire is the default on the server, in
WASM and in the mount elements. The internal Rust code builds generated ops, not `DomOp`.
The hand-written columnar code, the JSON and Arrow `DomOp` encoders, the 6-byte envelope for
UI frames, the JS `ColumnarParser` and the `DomOp` type are gone. Nothing that a web page
could do before is lost (D27).

This feature only moves and removes code. It adds no behaviour. Its story sections say
which behaviour must not change.

## 2. Story

- **S1**: "A maintainer proves that the frame wire does everything the old wire did on the
  web."
- **S2**: "A maintainer switches the defaults to the frame wire, and every test still
  passes."
- **S3**: "A maintainer changes the internal code from `DomOp` to generated ops, and every
  test still passes."
- **S4**: "A maintainer removes the old code, and the workspace still builds and passes."
- **S5**: "A proof item of the parity gate fails."

| # | Use case | Actor | Trigger | Result |
|---|---|---|---|---|
| U1 | Parity gate | Maintainer | The check list of section 6.2 | A table of evidence in `progress.md`, or a stop |
| U2 | Switch defaults | Maintainer | Step A | `App::new()`, the server sink and the mounts use frames |
| U3 | Move internals | Maintainer | Step B | No Rust code outside `foundation_ui_traits` names `DomOp` |
| U4 | Remove | Maintainer | Step C | The items of section 6.5 do not exist |

## 3. Bounded context

**Inside this feature:** the parity evidence; the default switch; the constructor module
`foundation_ui_wire::ops`; the mechanical change of every `DomOp` user; the removal list.

**Outside (must not change):**

| Thing | Owner | Why it stays |
|---|---|---|
| `Html`, `HtmlTag`, `AttrName`, `TAG_NAMES`, `ATTR_NAMES`, `Part`, `IntoHtml`, `markup`, `platform_types` | `foundation_ui_traits` | The authoring tree and the web name tables |
| `MorphDom`, `Hydrator`, `EventDispatcher`, `NodeRegistry`, the transports, the mount elements, `<primal-island>` | `foundation-wasm-ui.js` | D8, D40 |
| The HTML text protocol (`send_text`, `routeHtml`, the `html` SSE event, `primal-html`) | `foundation_wasm_ui` | D8, D40 |
| JSON signal patches and the JSON morph wrapper (`routeJson` cases `json` and `json-morph`) | `foundation-wasm-ui.js` | They are not `DomOp` encodings |
| `Broadcaster`, `BroadcastTx`, `FrameTransport` | `foundation_wasm_ui::server` | The live mode (D15). They carry bytes of any wire |
| Arrow IPC for data: `foundation_arrow` derive macros, `apache-arrow.js`, the `ArrowIpc` body | `foundation_arrow` | D28 |
| `WasmEnvelope` and protocol byte 0 (the batch-instructions stream of the WASM ABI: function calls, timers) | `foundation_wasm` | It is the general WASM ABI, not the UI wire. Only its use for DOM ops goes |
| `MessagePackBlob` | Schema | D9 |

| Entity | Meaning | Owner (the one writer) |
|---|---|---|
| Parity evidence | The table in `progress.md` | The executor of this feature |
| Removal list | Section 6.5 | This file |

| Verb | Kind | Trigger | Effect |
|---|---|---|---|
| prove | story | Start of the feature | Runs the gate; writes the evidence |
| switch | story | Gate passed | Changes three defaults |
| move | story | Switch done | Replaces `DomOp` construction with `ops::*` |
| remove | story | Move done | Deletes the list |

**Words.** "Old wire" is the 6-byte envelope with the columnar, JSON or Arrow encoding of
`DomOp`. "Frame wire" is the schema of feature 01. "Parity" means the same DOM result for
the same template and the same signal changes.

## 4. Rules

### 4.1 Law worksheet

```
Entity: Web behaviour
Conserved value:         for every template and every sequence of signal changes, the DOM after this feature equals the DOM before it
Bound:                   none
Inverse pairs:           none (a removal is not undone; the gate comes first)
Idempotent verbs:        none
Ordering pairs:          prove before switch; switch before move; move before remove
Monotone values:         the count of DomOp uses only decreases
Freeze points:           after remove, DomOp does not exist
Links to other entities: none
```

### 4.2 Invariants

| # | Invariant |
|---|---|
| I1 | No step starts before the step before it is complete and its tests pass. |
| I2 | After each step, `cargo test` of every crate in the `crates` list and `mise run ui-web:test` pass. |
| I3 | After step A, an `App` made with `App::new()` ships frames, and the DOM result of each existing App test is unchanged. |
| I4 | After step B, `grep -rn "DomOp" --include=*.rs backends examples` finds matches only in `backends/foundation_ui_traits` and in `backends/foundation_ui_wire/src/vocab/dom_op.rs` and its test. |
| I5 | After step C, the same `grep` finds nothing, and `grep -n "ColumnarParser\|DomOpApplicator\|columnarHandler" backends/foundation_wasm_ui/runtimes/*.js` finds nothing. |
| I6 | The HTML text protocol, JSON signal patches, the JSON morph wrapper, Arrow IPC data and protocol byte 0 function calls work after step C as before. |
| I7 | No item of the "Outside (must not change)" table is removed. |

### 4.3 Must have

- The evidence table in `progress.md` before the first deletion.
- `foundation_ui_wire::ops`: one constructor function for each old `DomOp` variant.

### 4.4 Must do

- Stop and report to the main session when a gate item fails. Open a GitHub issue for the
  failure (rule 9). Do not weaken a test to pass the gate.
- Make each step its own commit, so a step can be reverted alone.
- Bump the version of every crate that loses or gains public API, and of its consumers.

### 4.5 Must not

- Do not skip, disable or delete a test to get green. A test of removed code is deleted
  only in step C, together with that code, and only if section 6.5 lists its file.
- Do not remove anything on a benchmark result. The rule is function, not speed (D27).
- Do not edit generated files under `examples/**/gen/` or bundled copies
  (`examples/**/public/app/bundle.js`); rebuild them with their build task if their
  example is built in CI, else leave them and note it.

## 5. Acceptance scenarios

```gherkin
Feature: Retirement of the old wire

  Scenario: The parity gate passes before anything changes
    Given features 08 to 11 are complete
    When the maintainer runs every item of the gate of section 6.2
    Then each item passes
    And progress.md holds the evidence table with the command and the result of each item

  Scenario: A failed gate item stops the feature
    Given one parity case fails
    When the maintainer runs the gate
    Then no default is switched and no file is deleted
    And a GitHub issue names the failing case

  Scenario: The default App ships frames
    Given step A is complete
    When a test makes App::new(), mounts <span>{count}</span>, sets count to 5 and stabilizes
    Then the shipped bytes are a Frame with the identifier "EWF1"
    And applying them gives a span with the text "5"

  Scenario: A mount with no protocol attribute reads frames
    Given step A is complete
    And <mount-stream api="/live"> with a transport that delivers a binary frame
    Then the frame applies

  Scenario: The old protocol name gives a clear error
    Given step C is complete
    And <mount-stream api="/live" protocol="arrow">
    When the mount connects
    Then the console shows "mount: protocol \"arrow\" was removed; use \"frame\""
    And nothing is applied

  Scenario: The HTML text protocol still works
    Given step C is complete
    When a server calls BroadcastTx::send_text("<p>hi</p>") to a <mount-stream protocol="html">
    Then the page shows <p>hi</p>

  Scenario: A JSON signal patch still works
    Given step C is complete
    When a mount receives the JSON {"count": 5} on the json protocol
    Then the signal bridge got the patch

  Scenario: The old types do not exist
    Given step C is complete
    When a crate names foundation_ui_traits::DomOp
    Then it does not compile
```

## 6. Technical breakdown

### 6.1 Measured facts (2026-11-13; measure again at the start of the feature)

| Fact | Source |
|---|---|
| Files that name `DomOp`, by crate: `foundation_wasm_ui` 25, `foundation_ui_traits` 9, `foundation_macros` 3, `foundation_ui_components` 2, `foundation_arrow` 2, `foundation_wasm` 1, `foundation_signals` 1, `foundation_browser` 1 | `grep -rl --include=*.rs DomOp backends` |
| The largest users: `html_macro/codegen.rs` (24 lines), `instruction/receiver.rs` (13), `slots.rs` (12), `foundation_arrow/src/dom_ops.rs` (9), `reactive.rs` (7) | `grep -c DomOp <file>` |
| `App::new()` uses the columnar wire; `App::with_protocol` takes any protocol | `backends/foundation_wasm_ui/src/app.rs:52`, `:116` |
| `BroadcastSink::new` is the default columnar server sink | `backends/foundation_wasm_ui/src/server/mod.rs:186-191` |
| Users of `App::new()` or `BroadcastSink::new` or `protocol="arrow"` outside `foundation_wasm_ui`: `foundation_browser/tests/smoke.rs`, `foundation_browser/src/test/server.rs`, `foundation_http/tests/http_app_ctx_tests.rs`, `foundation_macros/src/lib.rs` (the `wasm_ui_server` macro), `examples/platform_ios/app`, `examples/platform_android/app`, `examples/platform_android/app-hello` | `grep -rn 'App::new()\|BroadcastSink::new\|protocol="arrow"'` |
| `WasmEnvelope` is also the frame of protocol byte 0, the general batch-instructions ABI | `backends/foundation_wasm/src/protocol.rs:93-110`; `backends/foundation_ui_traits/src/lib.rs` module doc |
| The name `Envelope` in `foundation_connectrpc`, `foundation_openapi`, `foundation_cloudflare_types`, `foundation_proxy`, `foundation_netio`, `foundation_db` and the deployment crates is a different type | `grep -rl "Envelope\b"`; none of them imports `foundation_ui_traits::Envelope`. Confirm with `cargo tree -i foundation_ui_traits --workspace --depth 1` |

### 6.2 The parity gate (step 0)

Run each item. Write one row for each in `progress.md`: the item, the command, the result,
the date.

| # | Item | Command or check |
|---|---|---|
| G1 | Each of the 19 `DomOp` variants gives the same DOM on both wires | `mise run ui-web:test` (the `frames-parity` cases of feature 09) |
| G2 | Both tiers give the same DOM | `mise run ui-web:test` (`frames-columnar`) |
| G3 | The live channel passes | `cargo test -p foundation_wasm_ui --profile uat live_channel` |
| G4 | The counter example passes in a browser with three origins and a live region | `mise run test:browser` (the `ui_counter` tests of features 10 and 11) |
| G5 | Every existing App test gives the same DOM through `FrameV1` as through the columnar wire | A new test, `backends/foundation_wasm_ui/tests/frame_parity_app_tests.rs`: for each scenario of `tests/app_tests.rs`, `tests/reactive_structure_tests.rs` and `tests/slots_tests.rs` that records sent batches, run the same steps with a recording `FrameV1` protocol and with `MockProtocol`; assert that the ops of the frames equal `web_form` of the recorded `DomOp`s |
| G6 | The browser smoke tests pass on the frame protocol | Run `backends/foundation_browser/tests/smoke.rs` with the test server switched to `FrameBroadcastSink` and `protocol="frame"` by an environment variable `EWE_UI_WIRE=frame` that the test server reads (add it in this step; remove the variable in step C) |
| G7 | The component behaviours pass | `node --test backends/foundation_ui_components/integration/test/` with the frame runtime installed in the test setup |

A failed item stops the feature (section 4.4).

### 6.3 Step A: switch the defaults

1. `App::new()` builds on `FrameV1::new(0)`. Add `App::columnar()` with the old body, so
   step B can keep the comparison of G5 until step C removes it.
2. In `server/mod.rs`, mark `BroadcastSink` with
   `#[deprecated(note = "use FrameBroadcastSink over a LiveChannel")]`. Change the
   `wasm_ui_server` macro (`backends/foundation_macros/src/lib.rs:782`) and the
   `foundation_browser` test server to `LiveChannel` and `FrameBroadcastSink`.
3. In `foundation-wasm-ui.js`: `registerWasmApp` installs the frame runtime when
   `globalThis.EweFrames?.installFrameRuntime` exists (the page loads
   `foundation-ui-frames.js`, which sets `globalThis.EweFrames`; add that one assignment at
   the end of `foundation-ui-frames.js`). A mount with no `protocol` attribute on a binary
   channel already detects a frame by its identifier (feature 09 seam 2).
4. Change the three example apps of section 6.1 only if CI builds them
   (`python3 scripts/ci/ci.py plan` lists them); else leave them and note it in
   `learnings.md`.
5. Run the verification of section 9. Commit.

### 6.4 Step B: move the internals from `DomOp` to generated ops

1. Add `backends/foundation_ui_wire/src/ops.rs`: one function for each old variant, with
   the old field names, that returns the `OpT` of feature 02 section 6.4:

```rust
#[must_use] pub fn create_element(node_id: u32, tag: &HtmlTag, class: &str) -> OpT;
#[must_use] pub fn create_text_node(node_id: u32, content: &str) -> OpT;
#[must_use] pub fn set_text(node_id: u32, text: &str) -> OpT;
#[must_use] pub fn set_attribute(node_id: u32, name: &AttrName, value: &str) -> OpT;
#[must_use] pub fn remove_attribute(node_id: u32, name: &AttrName) -> OpT;
#[must_use] pub fn set_property(node_id: u32, name: &AttrName, value: &str) -> OpT;
#[must_use] pub fn add_event_listener(node_id: u32, event_name: &AttrName) -> OpT;
#[must_use] pub fn remove_event_listener(node_id: u32, event_name: &AttrName) -> OpT;
#[must_use] pub fn append_child(parent_id: u32, child_id: u32) -> OpT;
#[must_use] pub fn remove_child(parent_id: u32, child_id: u32) -> OpT;   // gives Remove(child_id)
#[must_use] pub fn remove_node(node_id: u32) -> OpT;
#[must_use] pub fn insert_before(parent_id: u32, child_id: u32, ref_id: u32) -> OpT;
#[must_use] pub fn replace_node(old_id: u32, new_id: u32) -> OpT;
#[must_use] pub fn set_style(node_id: u32, prop: &AttrName, value: &str) -> OpT;
#[must_use] pub fn add_class(node_id: u32, class: &str) -> OpT;
#[must_use] pub fn remove_class(node_id: u32, class: &str) -> OpT;
#[must_use] pub fn morph_node(target: MorphTarget<'_>, action: MorphAction, content: &str) -> OpT;
#[must_use] pub fn register_node(node_id: u32) -> OpT;
#[must_use] pub fn unregister_node(node_id: u32) -> OpT;

/// Replaces `foundation_ui_traits::TargetSelector`.
pub enum MorphTarget<'a> { NodeId(u32), Id(&'a str), Class(&'a str), Query(&'a str) }
```

   `MorphAction` is the generated enum. A test asserts
   `ops::x(args) == web_form(&DomOp::X { args })` for each function; that test is deleted
   in step C with `web_form`.
2. Change each producer, one crate at a time, and run that crate's tests after each:
   `ProtocolMethods<Vec<DomOp>>` becomes `ProtocolMethods<Vec<OpT>>`;
   `InstructionReceiver` queues `OpT`; `slots.rs`, `reactive.rs`, `app.rs`,
   `foundation_ui_components`, `foundation_signals` and the reactive codegen of `html!`
   (`html_macro/codegen.rs`, with a new `foundation_ui_wire_path()` in `crate_paths.rs`)
   call `ops::*`. `FrameV1` and `FrameBroadcastSink` take `Vec<OpT>` and drop their
   `web_form` step.
3. Tests that compare recorded `DomOp` values compare `OpT` values built with `ops::*`.
   This is a change of the expected-value type only. Do not change which ops a test
   expects or their order.
4. `MockProtocol` records `Vec<OpT>`.
5. Run the verification. Commit.

### 6.5 Step C: remove

Delete, then fix the build, then run the verification.

| Remove | From |
|---|---|
| `dom_op.rs`, `encoder.rs`, `columnar_encoder.rs`, `json_encoder.rs`, `envelope.rs`, their re-exports, and `tests/protocol_tests.rs` | `backends/foundation_ui_traits` |
| `vocab/dom_op.rs`, its test, the `ops` equivalence test, the `foundation_ui_traits::DomOp` uses | `backends/foundation_ui_wire` |
| `src/dom_ops.rs` and its tests | `backends/foundation_arrow` |
| `protocol/columnar.rs`, `protocol/json.rs`, `protocol/arrow_ipc.rs`, `protocol/frame_sink.rs`, the DOM part of `protocol/batch_instructions.rs` (`DomOpsBatch`, `BATCH_OP_APPLY_DOM`), `instruction::ColumnarReceiver`, `App::columnar`, `App::arrow_ipc`, `BroadcastSink`, the `arrow` feature if nothing else uses it, `tests/protocol_impl_tests.rs`, `tests/wire_bench.rs`, `tests/frame_parity_fixtures_tests.rs`, `tests/frame_parity_app_tests.rs`, `integration/fixtures/frames/*.old.bin` | `backends/foundation_wasm_ui` |
| `Op`, `TAG_NAMES`, `ATTR_NAMES`, `resolveWireName`, `ColumnarParser`, `DomOpApplicator`, `columnarHandler`, `BATCH_OP_APPLY_DOM`, `registerDomBatchOperation`, `jsonRowsToBatch`, `arrowTableToBatch`, `isJsonDomOpBatch` and its branch in `routeJson`, `ArrowHandler`, the `protocol === 1` and DomOp `protocol === 2` branches of `decodeEnvelopeFrame`, the `arrow` and `arrow-ipc` cases of `Patcher.applyRoute`, `Patcher.ensureApplicator` (replace it with a function that makes only the seeded `NodeRegistry`, and update `installFrameRuntime`), `registerArrowIpc` | `backends/foundation_wasm_ui/runtimes/foundation-wasm-ui.js` |
| `integration/test/frames-parity.test.js`, `batch-protocol.test.js` cases for DOM ops, and the columnar cases of `foundation-wasm-ui.test.js` and `protocol-negotiation.test.js` | `backends/foundation_wasm_ui/integration/test` |
| The `EWE_UI_WIRE` variable | `backends/foundation_browser` |

Keep in `decodeEnvelopeFrame`: the frame check, and the JSON branch for non-DomOp JSON.
`ProtocolHandler.named("arrow")` and a content type `primal-arrow` log
`mount: protocol "arrow" was removed; use "frame"` and return a handler whose result is
`{ type: "raw", text: "" }`.

Also update: the module docs of `foundation_ui_traits/src/lib.rs` and its `Cargo.toml`
description; the docs of `server/mod.rs`; `ci/packages.toml` if a feature name it lists
went away (the plan guard fails on an unknown feature, rule 15).

### 6.6 Fault tables

**prove**

| Step that fails | What the system does | What the caller sees | What is left behind |
|---|---|---|---|
| A gate item | The feature stops | A report with the failing item and a GitHub issue | The evidence rows that passed; no code change except the new test of G5 and the variable of G6 |

**switch, move, remove**

| Step that fails | What the system does | What the caller sees | What is left behind |
|---|---|---|---|
| A test fails after a change | The executor fixes the cause in the changed code, or reverts that change | The failing test output in the report | The commits of the steps before it. Each step is one commit, so `git revert` of the last commit restores the state |
| A consumer outside the workspace list uses a removed type | The build of that crate fails | A compile error | Stop; add the crate to the feature's list and move it in step B first |

## 7. Tests

| Law or scenario | Test | Oracle |
|---|---|---|
| I1; Scenarios: gate passes, failed gate stops | The evidence table in `progress.md`; the commit order (prove, A, B, C) | expected outcome |
| I2 | The commands of section 9 after each step | expected outcome |
| I3; Scenario: the default App ships frames | `backends/foundation_wasm_ui/tests/app_tests.rs::default_app_ships_frames` (new) and the unchanged DOM assertions of that file | example |
| G5 | `frame_parity_app_tests.rs` | metamorphic |
| I4, I5; Scenario: the old types do not exist | The `grep` commands of section 9; `backends/foundation_ui_traits/tests/trybuild/fail_dom_op_removed.rs` | expected outcome |
| I6; Scenarios: HTML text protocol, JSON signal patch | The existing `integration/test/sse.test.js`, `protocol-negotiation.test.js` (HTML and JSON cases) and `morphdom.test.js`, unchanged; `foundation_arrow` tests of the derive macros, unchanged; `foundation_wasm` tests of protocol byte 0, unchanged | expected outcome |
| Scenario: a mount with no protocol attribute | `integration/test/frames-routing.test.js` "no protocol attribute reads frames" (new case) | example |
| Scenario: the old protocol name | `frames-routing.test.js` "protocol arrow logs a removal message" (new case) | example |
| I7 | A review item in "Done when": the reviewer checks each row of the "must not change" table against `git diff --stat` | expected outcome |

## 8. Tasks

- [ ] 1. Measure section 6.1 again; record the numbers in `learnings.md`.
- [ ] 2. Write the G5 test and the G6 variable.
- [ ] 3. Run the gate; write the evidence table. Stop if an item fails.
- [ ] 4. Step A item 1: `App::new()` and `App::columnar()`.
- [ ] 5. Step A items 2 to 4; verify; commit.
- [ ] 6. Step B item 1: `foundation_ui_wire::ops` with its equivalence test.
- [ ] 7. Step B: `foundation_wasm_ui` producers and tests.
- [ ] 8. Step B: the `html!` reactive codegen.
- [ ] 9. Step B: `foundation_ui_components`, `foundation_signals`, `foundation_browser`; verify; commit.
- [ ] 10. Step C: Rust removals; fix the build.
- [ ] 11. Step C: JS removals; fix the JS tests that named removed exports.
- [ ] 12. Step C: docs, `ci/packages.toml`, version bumps; verify; commit.
- [ ] 13. Close the GitHub issues that this removal fixes, if any name the removed code.
- [ ] 14. Update `progress.md` and `learnings.md`.

## 9. Verify

Run after each step:

```bash
cargo fmt --all -- --check
for c in foundation_ui_traits foundation_ui_wire foundation_ui_runtime foundation_macros \
         foundation_arrow foundation_signals foundation_wasm foundation_wasm_ui \
         foundation_ui_components foundation_browser foundation_http ui_counter; do
  cargo clippy -p "$c" --profile uat --no-deps -- -D warnings && cargo test -p "$c" --profile uat || break
done
cargo check -p foundation_wasm_ui --profile uat --target wasm32-unknown-unknown
mise run ui-web:test
mise run ui-wire:check
mise run test:browser
python3 scripts/ci/ci.py plan
```

After step B: `grep -rn "DomOp" --include=*.rs backends examples | grep -v "backends/foundation_ui_traits/\|vocab/dom_op\|vocab_dom_op_tests\|ops_equivalence"` prints nothing.

After step C:

```bash
grep -rn "DomOp\|ColumnarEncoder\|JsonEncoder\|encode_with_envelope" --include=*.rs backends examples   # nothing
grep -n "ColumnarParser\|DomOpApplicator\|columnarHandler" backends/foundation_wasm_ui/runtimes/*.js      # nothing
cargo tree -i foundation_ui_traits --workspace --depth 1    # each listed crate builds
```

`foundation_ui_traits` keeps its `clippy-deny-warnings = false` override in
`ci/packages.toml` only if its warnings remain; if the removal clears them, delete the
override (its reason says so).

## 10. Done when

- [ ] All tasks are complete and every command of section 9 passes with no warning.
- [ ] The evidence table is in `progress.md`, with every gate item passed, dated before the
      first deletion commit.
- [ ] The four steps are four commits, in order.
- [ ] No item of the "Outside (must not change)" table was removed.
- [ ] No test was skipped or weakened.
- [ ] The check list of the `story-driven-specification` skill passes.

---

_Created: 2026-11-13_
