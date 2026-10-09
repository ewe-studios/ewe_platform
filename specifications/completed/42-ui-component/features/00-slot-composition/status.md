# Feature 00 — Status: COMPLETE (2026-06-13)

## What shipped

- **`foundation_ui_traits`**: `Html.runtime_id: Option<u32>` (additive;
  pure trees carry None) + `Html::to_markup()` (markup.rs — WHATWG-rule
  escaping, void elements, tagless flattening, visible `unknown-tag`
  marker for out-of-table ids).
- **`foundation_wasm_ui::slots`**: `Render` (`&self`, object-safe,
  re-invokable, provided `into_slot`), `Slot` (`Box<dyn Render>` +
  `Slot::lazy` closure entry + `From<Html>` + `Render` delegation),
  `mount_fragment` (AppendChild-by-reference for mounted fragments at ANY
  depth — incl. inside grouping wrappers, a bug the acceptance test caught;
  document-order build with a fresh id block for pure fragments, primal-id
  re-stamped with runtime ids), `mount_into` convenience.
- **`html!` macro**: `<Fragment>{expr}…</Fragment>` (compile-time
  directive, no DOM node, exprs evaluate ONCE during build ops; pure form =
  transparent grouping wrapper); capitalized tags reserved (unknown →
  compile error with the custom-elements-need-a-dash hint; not valid in
  scoped styles); text slots render as the dedicated `<span>` in BOTH
  forms (reactive: + RegisterNode + primal-id, SetText targets it; pure:
  text-shaped values wrap, element values inline as before); reactive root
  stamps `runtime_id = __base`.
- **JS runtime**: morph identity extended — `morphIdentity(el)` = `id` ??
  `primal-id` — REQUIRED for §4's whole point (review of the acceptance
  test showed F07 matched on `id` only, so slot spans were NOT preserved;
  now they anchor like user ids; server re-renders of one template have
  stable primal-ids).

## Deviations from the spec (documented decisions)

| spec said | shipped | why |
|-----------|---------|-----|
| pure text-slot spans carry `primal-id` | pure slot spans carry NO id attribute | pure element-id numbering is compile-time and assigning ids to slots would renumber every existing template contract; parity that morph needs is STRUCTURAL (span at the slot position) + reactive spans keep ids. Revisit if a real morph case demands it. |
| `to_markup` escapes `'` | escapes `& < >` text / `& "` attributes | WHATWG serialization rules; values are always double-quoted so `'` needs no escape. |

## Verification

- Rust: 3 failure-mode acceptance tests (owned-Html compiles+builds; pure
  fragment splices with fresh re-stamped ids; reactive child spliced by
  reference, built exactly once, interior effects live after splice) +
  span-on-the-wire + typed slot struct (required/optional/static+lazy
  children) + recipe semantics (drop after render, two renders = two
  instances both updating) + struct components + grouping-node
  mount_fragment + to_markup parity/escaping. 12/12
  (slots_tests.rs) + 6 markup unit tests + full suites of all three crates
  green; zero clippy `--all-targets`; workspace check green.
- JS: morph-survival test (slot span keeps node identity through a
  reorder+content morph); full suite 88/88.
