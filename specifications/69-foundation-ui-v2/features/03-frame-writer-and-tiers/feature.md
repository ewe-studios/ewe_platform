---
feature: "frame-writer-and-tiers"
description: "The Rust frame writer: validation, the platform gate, tier choice between OpBatch and ColumnarBatch, table streaming, and the stream sequencer."
status: "pending"
priority: "high"
model: "sonnet"
depends_on: ["01-wire-schema-and-codegen", "02-vocabulary-and-platform-forms"]
estimated_effort: "large"
created: 2026-11-13
last_updated: 2026-11-13
author: "Main Agent"
crates: ["foundation_ui_wire"]
decisions: ["D8", "D16", "D24", "D27", "D28", "D29", "D36", "A1", "R8"]
tasks:
  completed: 0
  uncompleted: 10
  total: 10
  completion_percentage: 0
---

# Feature 03: The frame writer and the tiers

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** user interface (UI); HyperText Markup Language (HTML);
identifier (id); inter-process communication (IPC); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

The module `foundation_ui_wire::writer` turns a list of ops into the bytes of one `Frame` for
one client. It checks the ops, removes the ops that the client must not receive (the gate),
and picks tier 1 (`OpBatch`) or tier 2 (`ColumnarBatch`). It also writes data tables as a
stream of frames and gives each frame its `sequence_id`. Every producer of frames (server,
WASM, device Rust) uses this one writer.

## 2. Story

- **S1**: "A producer writes a small update for a web client."
- **S2**: "A producer writes a large list render, and the writer uses columns."
- **S3**: "A producer writes one batch that has a web form and a mobile form, for a native
  client."
- **S4**: "A producer sends a large data table as many frames."
- **S5**: "A producer gives a property the wrong value type."
- **S6**: "A producer writes a frame that is larger than the limit."
- **S7**: "A producer writes a state that nobody signed."
- **S8**: "An old client does not know a new node kind."

| # | Use case | Actor | Trigger | Result |
|---|---|---|---|---|
| U1 | Write ops | Producer | `write_ops` | Frame bytes and a `WriteReport` |
| U2 | Tier choice | Writer | The op count after the gate | `Tier::Ops` or `Tier::Columnar` |
| U3 | Gate | Writer | `ClientCaps` | Ops for another platform are not in the bytes |
| U4 | Stream a table | Producer | `table_frames` | An iterator of frames with at most `max_table_rows` rows each |
| U5 | Reject | Producer | A bad op or a large frame | `WriteError` and no bytes |
| U6 | Number frames | Producer | `StreamSequencer::next` | Strictly increasing `sequence_id` |

## 3. Bounded context

**Inside this feature:** `ClientCaps`, `WriterConfig`, `FrameHeader`, `WriteReport`,
`WriteError`, `StreamSequencer`, the write functions, the column layout of tier 2.

**Outside:**

| Thing | Owner |
|---|---|
| The schema, `WireLimits`, `create_aligned_vector`, `finish_frame` | Feature 01 |
| `form_platform`, `applies_on`, `check_prop`, `vocab_version` | Feature 02 |
| Sealing a state (the tag) | Feature 04 |
| Which names a client knows; building `ClientCaps` from `ClientHello` | Feature 07 |
| Reading the columns | Features 09, 14, 15 |

| Entity | Meaning | Owner (the one writer) |
|---|---|---|
| Frame bytes | One finished `Frame` buffer | `writer` |
| `ClientCaps` | What one connection declared | The caller (feature 07 builds it) |
| `StreamSequencer` | The counter of one stream | The producer that owns the stream |
| `WriteReport` | What the gate removed and which tier was used | `writer` |

| Verb | Kind | Trigger | Effect |
|---|---|---|---|
| `write_ops` | story | Producer | Validate, gate, choose tier, encode, check size |
| `table_frames` | story | Producer | Split rows, number each frame, encode |
| `next` | entity | Producer | Returns the next `sequence_id` and increments |
| `write_error`, `write_caps`, `write_arrow_ipc`, `write_messagepack` | entity | Producer | Encode one body |

**Words.** "Gate" is the step that removes ops. "Filtered" means removed by the gate and
listed in the report. "Eligible" means that a form has a column layout in section 6.4.

## 4. Rules

### 4.1 Law worksheets

```
Entity: Frame bytes
Conserved value:         ops out = ops in minus filtered ops, in the same order
Bound:                   length <= WireLimits::max_frame_bytes
Inverse pairs:           write then read gives the kept ops (both tiers)
Idempotent verbs:        write_ops is a pure function of its arguments
Ordering pairs:          validate before gate; gate before tier choice; encode before size check
Monotone values:         none
Freeze points:           none
Links to other entities: sequence_id comes from the header that the caller filled
```

```
Entity: StreamSequencer
Conserved value:         stream_id
Bound:                   sequence_id >= 1
Inverse pairs:           none
Idempotent verbs:        none (each next() gives a new number)
Ordering pairs:          none
Monotone values:         sequence_id strictly increases
Freeze points:           stream_id is fixed at construction
Links to other entities: none
```

```
Entity: WriteReport
Conserved value:         filtered.len() + kept == input length (top-level rows)
Bound:                   none
Inverse pairs:           none
Idempotent verbs:        none
Ordering pairs:          none
Monotone values:         row_index values in `filtered` increase
Freeze points:           none
Links to other entities: none
```

### 4.2 Invariants

| # | Invariant |
|---|---|
| I1 | The bytes of `write_ops` for `mount: Native` never hold a `Web` form, at any depth of `ReplaceSubtree`. |
| I2 | The bytes of `write_ops` for `mount: Web` never hold a `Mobile` form, at any depth. |
| I3 | The kept ops keep their relative order. |
| I4 | Reading the frame gives the same ops for `Tier::Ops` and for `Tier::Columnar`. A columnar read maps an empty string to "absent". |
| I5 | The writer uses `Tier::Columnar` exactly when the kept count is greater than `columnar_threshold` and every kept op is eligible. |
| I6 | Every `[uint]`, `[int]`, `[long]` and `[double]` vector in a `ColumnarBatch`, `DataTable` or `ArrowIpcBlob` that the writer makes starts on an 8-byte boundary of the buffer. |
| I7 | A frame longer than `max_frame_bytes` is never returned. |
| I8 | A `SetProp` whose value type does not match its property is never written. |
| I9 | An `AttachState` whose state has an empty `tag` is never written, unless `allow_unsealed_state` is true. |
| I10 | `StreamSequencer::next` returns 1, then 2, then 3, and so on, with no repeat and no gap. |
| I11 | Each frame of `table_frames` has at most `max_table_rows` rows, and the rows of all frames, in order, are the rows of the input. |

### 4.3 Must have

- `WriterConfig` with `columnar_threshold` (default 64), `max_table_rows` (default 8192),
  `allow_unsealed_state` (default false) and a `WireLimits`.
- One gate function that every write path calls. No second copy of the rule.

### 4.4 Must do

- Report every filtered op with its row index and reason.
- Return an error, not a partial frame, when validation fails.
- Replace an unknown `NodeKind` with `UNKNOWN` for an old client, and report it (research 11
  section 3.3: the client shows a fallback view).

### 4.5 Must not

- Do not let the action author pick the tier (D16).
- Do not drop an op in silence. A removed op is in the report.
- Do not write an Arrow IPC body for UI ops. `write_arrow_ipc` is for data that goes to Arrow
  tools (D28).

## 5. Acceptance scenarios

```gherkin
Feature: Frame writer and tiers

  Scenario: A small update uses tier 1
    Given one SetText op for node 42 with text "1"
    And ClientCaps for a web mount with the current vocabulary
    When the producer calls write_ops with header stream 7, sequence 20, ack 5
    Then the call succeeds with report.tier == Tier::Ops and no filtered op
    And read_frame gives body Ops with that one SetText

  Scenario: A large eligible batch uses tier 2
    Given 65 SetText ops for nodes 1 to 65
    And the default WriterConfig
    When the producer calls write_ops for a web mount
    Then report.tier == Tier::Columnar
    And the frame body is Columnar with 65 rows

  Scenario: A batch at the threshold stays in tier 1
    Given 64 SetText ops
    When the producer calls write_ops for a web mount
    Then report.tier == Tier::Ops

  Scenario: A large batch with a subtree stays in tier 1
    Given 65 SetText ops and one ReplaceSubtree op
    When the producer calls write_ops for a web mount
    Then report.tier == Tier::Ops

  Scenario: A native client does not receive the HTML morph
    Given the ops [SetText(42), HtmlMorph(node 40, "<b>x</b>"), NativeCommand(HAPTIC)]
    And ClientCaps for a native mount
    When the producer calls write_ops
    Then the frame holds [SetText(42), NativeCommand(HAPTIC)]
    And report.filtered == [{ row_index: 1, op: HtmlMorph, reason: OtherPlatform }]

  Scenario: A web client does not receive the native command
    Given the same three ops
    And ClientCaps for a web mount
    When the producer calls write_ops
    Then the frame holds [SetText(42), HtmlMorph]
    And report.filtered == [{ row_index: 2, op: NativeCommand, reason: OtherPlatform }]

  Scenario: A web form inside a subtree is removed for a native client
    Given ReplaceSubtree(node 40, [CreateNode(41, TEXT), WebClass(41, "x")])
    And ClientCaps for a native mount
    When the producer calls write_ops
    Then the frame holds ReplaceSubtree(node 40, [CreateNode(41, TEXT)])

  Scenario: A wrong property value is rejected
    Given SetProp(node 1, PADDING, Text "8px")
    When the producer calls write_ops
    Then the call fails with WriteError::Vocab(PropKindMismatch { .. }) at row 0
    And no bytes are returned

  Scenario: An unsealed state is rejected
    Given AttachState(root 40) whose SignedState has an empty tag
    And the default WriterConfig
    When the producer calls write_ops
    Then the call fails with WriteError::UnsealedState { row_index: 0 }

  Scenario: A frame above the limit is rejected
    Given one SetText op with a text of 2000 bytes
    And WireLimits with max_frame_bytes 1024
    When the producer calls write_ops
    Then the call fails with WriteError::TooLarge { limit: 1024, .. }

  Scenario: An old client gets a fallback kind
    Given CreateNode(node 9, kind 11) where 11 is above max_node_kind of the client's version
    When the producer calls write_ops
    Then the frame holds CreateNode(node 9, UNKNOWN)
    And report.filtered == [{ row_index: 0, op: CreateNode, reason: UnknownVocabulary }]

  Scenario: A table of 20000 rows becomes three frames
    Given a DataTable with 20000 rows and one I64 column
    And max_table_rows 8192
    When the producer collects table_frames with a sequencer that is at 0
    Then there are 3 frames with 8192, 8192 and 3616 rows
    And their sequence ids are 1, 2 and 3
```

## 6. Technical breakdown

### 6.1 Public API (`backends/foundation_ui_wire/src/writer/`)

Files: `mod.rs`, `caps.rs`, `config.rs`, `gate.rs`, `tier.rs`, `columnar.rs`, `table.rs`,
`sequencer.rs`, `error.rs`.

```rust
/// What one connection declared in its ClientHello.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientCaps {
    pub platform: Platform,
    pub mount: MountSurface,
    pub vocabulary_version: u32,
    /// Action references ("origin:name") that the client has. `None` means "all".
    pub known_actions: Option<alloc::collections::BTreeSet<String>>,
}
impl ClientCaps {
    /// A web client with the current vocabulary and every action.
    #[must_use] pub fn web_current() -> Self;
    /// A native client with the current vocabulary and every action.
    #[must_use] pub fn native_current(platform: Platform) -> Self;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriterConfig { /* private fields */ }
impl Default for WriterConfig { /* 64, 8192, false, WireLimits::default() */ }
impl WriterConfig {
    #[must_use] pub fn columnar_threshold(self, ops: usize) -> Self;
    #[must_use] pub fn max_table_rows(self, rows: usize) -> Self;   // must be > 0
    #[must_use] pub fn allow_unsealed_state(self, allow: bool) -> Self;
    #[must_use] pub fn limits(self, limits: WireLimits) -> Self;
    // one getter for each field
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameHeader { pub stream_id: u64, pub sequence_id: u64, pub ack_action: u64 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Tier { Ops, Columnar }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterReason { OtherPlatform, UnknownVocabulary, UnknownAction }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FilteredOp { pub row_index: usize, pub op: Op, pub reason: FilterReason }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteReport { pub tier: Tier, pub filtered: Vec<FilteredOp> }

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WriteError {
    Vocab { row_index: usize, error: VocabError },
    UnsealedState { row_index: usize },
    /// An OpRow whose union is NONE.
    EmptyRow { row_index: usize },
    TooLarge { limit: usize, actual: usize },
    /// A table column whose length is not `rows`.
    ColumnLength { column: usize, expected: u64, actual: u64 },
    /// The client declared a vocabulary version that this build does not have.
    UnknownVocabularyVersion { version: u32 },
}

pub fn write_ops(ops: Vec<OpT>, caps: &ClientCaps, header: FrameHeader, config: &WriterConfig)
    -> Result<(Vec<u8>, WriteReport), WriteError>;

/// Applies only the gate. Feature 11 uses it to encode one batch per mount surface.
pub fn gate(ops: Vec<OpT>, caps: &ClientCaps) -> Result<(Vec<OpT>, Vec<FilteredOp>), WriteError>;

pub fn write_error(error: &ActionErrorT, header: FrameHeader, config: &WriterConfig)
    -> Result<Vec<u8>, WriteError>;
pub fn write_caps(caps: &ServerCapsT, header: FrameHeader, config: &WriterConfig)
    -> Result<Vec<u8>, WriteError>;
pub fn write_arrow_ipc(target_node: u32, ipc: &[u8], header: FrameHeader, config: &WriterConfig)
    -> Result<Vec<u8>, WriteError>;
pub fn write_messagepack(target_node: u32, bytes: &[u8], header: FrameHeader,
    config: &WriterConfig) -> Result<Vec<u8>, WriteError>;

/// Splits `table` by rows. Each item is one finished frame. Lazy: it encodes a frame when
/// the caller asks for it.
pub fn table_frames<'a>(table: &'a DataTableT, sequencer: &'a mut StreamSequencer,
    config: &'a WriterConfig) -> Result<TableFrames<'a>, WriteError>;
pub struct TableFrames<'a> { /* private */ }
impl Iterator for TableFrames<'_> { type Item = Result<Vec<u8>, WriteError>; }

#[derive(Debug)]
pub struct StreamSequencer { /* stream_id, last */ }
impl StreamSequencer {
    #[must_use] pub fn new(stream_id: u64) -> Self;            // last = 0
    #[must_use] pub fn resume(stream_id: u64, last: u64) -> Self;
    #[must_use] pub fn stream_id(&self) -> u64;
    #[must_use] pub fn last(&self) -> u64;
    pub fn next(&mut self) -> u64;                             // last + 1, saturating at u64::MAX
    /// A header for a push (ack_action 0) or an answer.
    pub fn header(&mut self, ack_action: u64) -> FrameHeader;
}
```

### 6.2 The steps of `write_ops`

```mermaid
flowchart TD
    A[ops, caps, header, config] --> V{validate each op}
    V -->|bad| E1[WriteError, no bytes]
    V --> G[gate: platform, vocabulary, action]
    G --> T{kept count > threshold<br/>and all eligible?}
    T -->|yes| C[encode ColumnarBatch<br/>with create_aligned_vector]
    T -->|no| O[encode OpBatch]
    C --> S{len <= max_frame_bytes?}
    O --> S
    S -->|no| E2[WriteError::TooLarge]
    S -->|yes| R[bytes + WriteReport]
```

_The diagram shows the fixed order of the steps of one write._

1. **Validate** each top-level op and each nested op of a `ReplaceSubtree`, in order:
   - a row with no op gives `EmptyRow`;
   - `SetProp`: `check_prop`; an error gives `Vocab`;
   - `AttachState`: read the nested bytes with `read_signed_state`; a read error or an empty
     `tag` gives `UnsealedState`, unless `allow_unsealed_state` is true (then only a read
     error fails).
   The `row_index` is the index of the top-level op.
2. **Gate** (section 6.3).
3. **Tier** (I5).
4. **Encode.** Tier 1: build `FrameT` and call `finish_frame`. If feature 01 recorded that
   `pack` does not align the numeric vectors, tier 1 is not affected, because `OpBatch` has
   no numeric vector. Tier 2: section 6.4.
5. **Size check** against `config.limits().frame_bytes()`.

### 6.3 The gate

`caps.vocabulary_version` must have a row in `VOCAB_VERSIONS`; if not, return
`UnknownVocabularyVersion`. For each op, in order, the first rule that matches decides:

| # | Condition | Effect | Reason |
|---|---|---|---|
| 1 | `!applies_on(op, caps.mount)` | Remove | `OtherPlatform` |
| 2 | `CreateNode.kind > max_node_kind` | Keep, with `kind = UNKNOWN` | `UnknownVocabulary` |
| 3 | `SetProp.prop > max_prop_id`; `BindAction.event` or `UnbindAction.event > max_event_id`; `WebElement.tag_id > max_tag_id`; `WebAttr.attr_id`, `WebProperty.name_id`, `WebStyle.name_id` or `WebListener.name_id > max_attr_id`; `NativeControl.control > max_native_control`; `NativeCommand.command > max_native_command`; the union number of the op `> max_op` | Remove | `UnknownVocabulary` |
| 4 | `BindAction` and `caps.known_actions` is `Some(set)` and the set does not hold `action` | Remove | `UnknownAction` |
| 5 | `ReplaceSubtree` | Apply rules 1 to 4 to each nested op. Nested removals are not listed one by one; if one or more nested ops were removed, list the `ReplaceSubtree` row once with the reason of the first removal | — |
| 6 | Otherwise | Keep | — |

### 6.4 Tier 2 column layout

`op_codes[i]` is the `Op` union number. A column that a form does not use holds `0`. A blob
that a form does not use is an empty range. `blob_a_off` and `blob_b_off` each have
`rows + 1` entries; the bytes of blob `x` of row `i` are
`blob_bytes[x_off[i] .. x_off[i + 1]]`. Write all `blob_a` bytes of all rows first, then all
`blob_b` bytes, so each offset column increases. `blob_b_off[0]` equals `blob_a_off[rows]`.

| Form | `node_ids` | `arg_a` | `arg_b` | `blob_a` | `blob_b` |
|---|---|---|---|---|---|
| `CreateNode` | `node_id` | `kind` | — | — | — |
| `SetText` | `node_id` | — | — | `text` | — |
| `SetProp` | `node_id` | `prop` + (`unit` << 16) | see below | text value | — |
| `AppendChild` | `parent_id` | `child_id` | — | — | — |
| `InsertBefore` | `parent_id` | `child_id` | `before_id` | — | — |
| `Remove` | `node_id` | — | — | — | — |
| `BindAction` | `node_id` | `event` | — | `action` | — |
| `UnbindAction` | `node_id` | `event` | — | — | — |
| `AttachState` | `root_node` | — | — | `state` bytes | — |
| `HtmlMorph` | `target.node_id` | `target.kind` | `action` | `target.text` | `content` |
| `ScriptNode` | `node_id` | `parent_id` | — | `body` | — |
| `WebElement` | `node_id` | `tag_id` | — | `custom_tag` | `class_name` |
| `WebText` | `node_id` | — | — | `content` | — |
| `WebAttr` | `node_id` | `attr_id` | `remove` (0 or 1) | `custom_attr` | `value` |
| `WebProperty` | `node_id` | `name_id` | — | `custom_name` | `value` |
| `WebStyle` | `node_id` | `name_id` | — | `custom_name` | `value` |
| `WebClass` | `node_id` | — | `remove` | `class_name` | — |
| `WebListener` | `node_id` | `name_id` | `remove` | `custom_name` | — |
| `WebRegistry` | `node_id` | — | `unregister` | — | — |
| `WebReplaceNode` | `old_id` | `new_id` | — | — | — |

`SetProp` `arg_b` by the `PropKind` of the property: `Length` gives `value.to_bits()` (and
the unit goes in `arg_a` bits 16 to 23); `ColorToken` and `FontToken` give the token id;
`Alignment` and `Axis` give the enum number; `Bool` gives 0 or 1; `Text` gives 0 and the
text goes in `blob_a`.

**Not eligible:** `ReplaceSubtree`, `LiveMount`, `NativeControl`, `NativeCommand`. A batch
with one of them uses tier 1.

Build the five `uint` columns with `create_aligned_vector`. Build the table with the
generated `ColumnarBatchBuilder`, not with `pack`.

`pub fn columnar_rows(batch: ColumnarBatch<'_>) -> Result<Vec<OpT>, ColumnarError>` in
`columnar.rs` is the inverse, for tests and for Rust readers. `ColumnarError` has
`LengthMismatch { column: &'static str, expected: usize, actual: usize }`,
`BadOffsets { row: usize }`, `UnknownOp { row: usize, code: u8 }` and
`InvalidUtf8 { row: usize }`. It checks that every column has `rows` entries (the offset
columns `rows + 1`) and that the offsets increase and stay inside `blob_bytes`.

### 6.5 Tables

`table_frames` first checks each column: the length of its data vector must equal
`table.rows`, else `ColumnLength`. Frame `k` holds rows `k * max .. min((k + 1) * max, rows)`
of every column, the matching slice of `validity` (bit `r` of the bitmap is row `r`; copy
bit by bit into a new bitmap that starts at bit 0), and the same `target_node`. A table with
0 rows gives one frame with 0 rows. Each frame takes `sequencer.header(0)`. Build `I32s`,
`I64s` and `F64s` vectors with `create_aligned_vector`.

### 6.6 Fault tables

**`write_ops`**

| Step that fails | What the system does | What the caller sees | What is left behind |
|---|---|---|---|
| Validate | Stops at the first bad op | `WriteError::{Vocab, UnsealedState, EmptyRow}` with the row | nothing |
| Gate: unknown vocabulary version | Stops | `WriteError::UnknownVocabularyVersion` | nothing |
| Size check | Drops the encoded bytes | `WriteError::TooLarge` | nothing. The caller's sequencer did advance if the caller took the header from it; the next frame then has a gap of one, which a client treats as a gap and resumes (feature 11). Callers that must not skip a number call `write_ops` with a header built from `sequencer.last() + 1` and call `next()` only on success |

**`table_frames`**

| Step that fails | What the system does | What the caller sees | What is left behind |
|---|---|---|---|
| Column length check | Fails before the first frame | `Err(ColumnLength)` from `table_frames` | nothing; the sequencer did not advance |
| A frame is above the limit | That item is `Err(TooLarge)`; the iterator then ends | The frames before it were already given | The frames already sent. The caller sends an `Error` frame with `write_error` so the client does not wait |

## 7. Tests

All in `backends/foundation_ui_wire/tests/`. Use `ClientCaps::web_current()` and
`ClientCaps::native_current(Platform::Ios)`.

| Law or scenario | Test | Oracle |
|---|---|---|
| I1, I2 | `writer_gate_tests.rs::no_other_platform_form_in_bytes`: for 200 seeded random batches (seed from a fixed `u64`, ops drawn from all 24 forms, nesting depth up to 3), read the bytes back and walk every op | property |
| I3 | `writer_gate_tests.rs::kept_ops_keep_order` (tag each op with an increasing node id) | property |
| I4 | `writer_tier_tests.rs::both_tiers_read_the_same`: write the same eligible batch with threshold 0 and with threshold `usize::MAX`; `columnar_rows` of one equals the unpacked ops of the other after "empty string = absent" | metamorphic |
| I5; Scenarios: tier 1, tier 2, threshold, subtree | `writer_tier_tests.rs::small_update_uses_tier_1`, `::large_eligible_batch_uses_tier_2`, `::batch_at_threshold_stays_in_tier_1`, `::subtree_forces_tier_1` | example |
| I6 | `writer_align_tests.rs::numeric_columns_are_8_aligned` for 1, 3, 65 and 1000 rows, and for a table frame | property |
| I7; Scenario: A frame above the limit | `writer_error_tests.rs::frame_above_limit_is_rejected` | expected outcome |
| I8; Scenario: A wrong property value | `writer_error_tests.rs::wrong_prop_value_is_rejected` | expected outcome |
| I9; Scenario: An unsealed state | `writer_error_tests.rs::unsealed_state_is_rejected`, `::unsealed_state_is_allowed_by_config` | expected outcome |
| I10 | `sequencer_tests.rs::next_is_strictly_increasing`, `::resume_continues_after_last` | property |
| I11; Scenario: A table of 20000 rows | `writer_table_tests.rs::table_splits_by_rows`, `::validity_bits_follow_rows`, `::empty_table_gives_one_frame` | example |
| Scenarios: native does not receive the morph; web does not receive the command; nested web form | `writer_gate_tests.rs::native_client_gets_no_html_morph`, `::web_client_gets_no_native_command`, `::nested_web_form_is_removed_for_native` | example |
| Scenario: An old client gets a fallback kind | `writer_gate_tests.rs::unknown_kind_becomes_fallback` (uses a test-only `ClientCaps` with `vocabulary_version` 1 and an op with kind 11) | example |
| Fault: column length | `writer_table_tests.rs::bad_column_length_fails_before_first_frame` (asserts `sequencer.last() == 0`) | fault |
| The fixture `frame_columnar_3rows.bin` of feature 01 equals `write_ops` output | `writer_tier_tests.rs::columnar_fixture_matches` (threshold 0, header 7/20/0) — update the fixture and the feature 01 test if the bytes differ, and record it | example |

## 8. Tasks

- [ ] 1. `caps.rs`, `config.rs`, `error.rs` and their constructors.
- [ ] 2. `sequencer.rs` and its tests.
- [ ] 3. Validation and its error tests.
- [ ] 4. `gate.rs` and its tests.
- [ ] 5. Tier 1 encoding and `write_ops` for tier 1.
- [ ] 6. `columnar.rs`: encode, `columnar_rows`, alignment tests.
- [ ] 7. `tier.rs` and the tier tests.
- [ ] 8. `table.rs` and its tests; `write_error`, `write_caps`, `write_arrow_ipc`, `write_messagepack`.
- [ ] 9. Bump `foundation_ui_wire` to `0.0.3`.
- [ ] 10. Update `progress.md` and `learnings.md`.

## 9. Verify

```bash
cargo fmt --all -- --check
cargo check  -p foundation_ui_wire --profile uat
cargo clippy -p foundation_ui_wire --profile uat --no-deps -- -D warnings
cargo test   -p foundation_ui_wire --profile uat
cargo check  -p foundation_ui_wire --profile uat --target wasm32-unknown-unknown
mise run ui-wire:test-ts
```

## 10. Done when

- [ ] All tasks are complete and every command of section 9 passes with no warning.
- [ ] Each invariant and each scenario has a passing test (section 7).
- [ ] The gate is one function, and every write path of ops calls it.
- [ ] No `TODO`, `FIXME`, `todo!()` or `unimplemented!()` is in the module.
- [ ] The check list of the `story-driven-specification` skill passes.

---

_Created: 2026-11-13_
