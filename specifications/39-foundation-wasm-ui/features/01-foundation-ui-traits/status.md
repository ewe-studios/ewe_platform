# Feature 01 — Status: COMPLETE (2026-06-11)

## What shipped

`backends/foundation_ui_traits` upgraded to the full feature-01 surface, split
into the spec's file layout (section 8): `html.rs`, `parts.rs`, `dom_op.rs`,
`encoder.rs`, `arrow_encoder.rs`, `json_encoder.rs`, `envelope.rs`, re-exported
flat from `lib.rs`. Still `no_std`, still zero dependencies.

- **`DomOp` — 19 variants (ops 0-18)** with `TargetSelector` (NodeId/Id/Class/
  Query) and `MorphAction` (5 actions) for `MorphNode`, plus the explicit
  registry ops `RegisterNode`/`UnregisterNode` and their implicit-registration
  rules (`ReplaceNode`, `RemoveNode`) documented on the enum.
- **`HtmlTag` / `AttrName`** (G12/G13/G14): known names ride the wire as
  `"id:<n>"` strings; tables `TAG_NAMES` (114 entries) / `ATTR_NAMES` (81)
  where index+1 == id, pinned ids `div/span/input/button` = 1-4 and
  `class/id/style/value` = 1-4. **Order is ABI** — the JS runtime mirrors both
  tables (generated from html.rs, not hand-copied).
- **`Html`** upgraded to typed `tag: Option<HtmlTag>`, `attributes:
  Vec<(AttrName, Cow)>`, `text: Option<Cow>`; `IntoHtml` with the 18 spec
  impls (+ `&Html` convenience).
- **`Part`**: `ChildPart` removed per spec section 1 — child positions are
  `Part::Text`; `Vec<Html>` handled by `IntoHtml` wrapping.
- **`DecodeError`** in the spec shape: `TruncatedBuffer{expected_min,actual}`,
  `UnknownOperation{op_id,row_index}`, `InvalidUtf8{column_name,row_index}`,
  `SchemaMismatch{detail}`, `JsonParseError{detail}`, `Other{detail}`.
- **`Row`** — ONE canonical decision-010 mapping (now `Option<String>` cells =
  nullable columns) shared by Arrow, JSON, and the byte-0 stream. Morph packing:
  `attribute = "<action>:<kind>:<selector>"` (split first two colons only — CSS
  queries contain `:`), node-id targets ride the `node_id` column.
- **`JsonEncoder`** rewritten to the spec's FLAT row objects
  (`{"op_id":..,"node_id":..,"operation":..,"attribute":null,...}`) with real
  `null`s; the `no_std` JSON parser grew `null` + surrogate-pair support.
- **`Envelope`** unchanged 6-byte layout + `ENVELOPE_SIZE` +
  `encode_with_envelope`. `PROTOCOL_VERSION` bumped 0 → 1 (spec: starts at 1;
  JS reads but never enforced the byte).

## Downstream updated in lockstep (kept green)

- `foundation_wasm_ui`: `batch_instructions.rs` (byte-0) re-mapped to the new
  `Row`/`DecodeError` with row-indexed errors; tests extended with the spec's
  byte-0 legs (tests 22-24: mixed/empty/all-19 round-trips).
- JS runtime `foundation-wasm-ui.js`: all 19 `Op` codes; mirrored
  `TAG_NAMES`/`ATTR_NAMES` + `resolveWireName` (`id:` prefix); `NodeRegistry`
  staging (`CreateElement`/`CreateTextNode` stage, `REGISTER_NODE` promotes or
  finds `[primal-id]`, re-register no-op); `REPLACE_NODE` pulls staged
  replacements and re-registers; `REMOVE_CHILD`/`SET_PROPERTY` (JSON-parse with
  raw fallback)/`ADD|REMOVE_EVENT_LISTENER` (keyed handler map + `onEvent` hook
  for feature 08); `MORPH_NODE` selector resolution + 5 actions (minimal
  application — full Datastar morphing is feature 07).
- Both e2e wasm fixtures rebuilt; `mock-dom.js` grew `removeChild`,
  `querySelector` (primal-id/#id/.class/tag), `insertAdjacentHTML` recorder,
  `parentNode`.

## Verification

- `foundation_ui_traits`: 39 tests (spec tests 1-21, 25-35 + wire-id contract
  + morph packing). Test 13 additionally proves the tag NAME never rides the
  wire for known tags.
- `foundation_wasm_ui`: 12 Rust tests (incl. all-19 byte-0 round-trip);
  19 JS tests incl. 2 e2e against freshly rebuilt Rust-encoded fixtures;
  foundation_wasm's 61 JS tests green with its rebuilt fixture.
- Zero clippy warnings on both crates (`--all-targets`, uat); full workspace
  `cargo check` green.

## Spec deviations (all justified)

| Spec says | Shipped | Why |
|-----------|---------|-----|
| `crates/foundation_ui_traits/` | `backends/foundation_ui_traits/` | `crates/*` is excluded from the workspace; every foundation crate lives in `backends/`. |
| `DecodeResult<T>` enum `Success/Error` | `type DecodeResult<T> = Result<T, DecodeError>` | Same two states, keeps `?`; spec tests' `Success(vec![])` ⇒ `Ok(vec![])`. |
| `Envelope::parse -> Option` | `Result<_, DecodeError>` | Strictly more diagnostic; existing consumers already use it. |
| `CustomBinaryEncoder` in this crate | `BatchInstructionsV1` in foundation_wasm_ui | Decision 022 (later) re-based byte-0 on the batch-instructions stream; it needs the WASM arena, which this crate must not depend on. Spec tests 22-24 live with it. |
| ArrowEncoder uses the `arrow` crate (IPC) | `no_std` Arrow-style columnar layout (F17 lineage) | The crate's own constraint ("no dependencies, any Rust target") and the shipped JS `ArrowParser` both pin this layout. Feature 05 owns the swap to real Arrow IPC behind the same `ProtocolEncoder` face. |
