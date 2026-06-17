# F6 — Pickers: select, combobox, autocomplete

Source: base-ui `types.md` references (2026-06-13). The family REQUIRES
feature 01 (`<For>` — dynamic option lists) and composes F4 positioning +
F5 virtual-highlight keyboard + M6 field state. Scheduled last for that
reason.

## The shared value model (capture once)

| base-ui | ours |
|---------|------|
| `value`/`defaultValue`/`onValueChange` (single: `Value\|null`; `multiple: true`: `Value[]`) | `(value, set_value)` — `Option<T>` or `Vec<T>` per static `multiple` |
| `items: {label, value}[] \| Group[]` | `Vec<PickItem<T>>` / `Vec<PickGroup<T>>` (label + value + disabled) — data-driven like F5 `MenuEntry` |
| `isItemEqualToValue` | `T: PartialEq` (the trait IS the prop) |
| `itemToStringLabel` / `itemToStringValue` (typeahead, form serialization, input display) | `PickItem.label: String` + `to_form_value: fn(&T) -> String` config — explicit, no Display magic |
| `name`/`form`/`required`/`readOnly`/`disabled` + hidden input | M6 — hidden `<input>` (single) / multiple inputs (multi) carry the serialized value |
| `autoComplete` | forwarded to the hidden/real input |
| `modal` (select `true`, combobox `false`) | config |

## select

Parts: Root / Trigger (button showing Value) / Value / Icon / Portal /
Backdrop / Positioner / Popup / Arrow / List / Item / ItemText /
ItemIndicator / Group / GroupLabel / ScrollUpArrow / ScrollDownArrow /
Separator.

Captured behaviors:
- Trigger shows the selected item's label via the Value part (placeholder
  when none); `data-placeholder`-style styling via empty state.
- Scroll arrows (ScrollUpArrow/ScrollDownArrow) carry
  `data-direction="up|down"` + visibility state (select.css selects
  `[data-direction]`).
- Popup ALIGNS THE SELECTED ITEM OVER THE TRIGGER (macOS-style; base-ui's
  `alignItemWithTrigger` positioning mode) with inner scroll + Scroll
  Up/Down arrow parts — v1 ships the simpler anchored-below mode (M1
  default); item-alignment mode is a documented M1 extension.
- Keyboard: closed trigger — ↑/↓/Enter/Space open (↑ highlights selected);
  typeahead SELECTS without opening (native select parity!). Open — F5
  virtual highlight: arrows, Home/End, typeahead by label, Enter selects,
  Escape closes; `multiple` keeps the popup open on select.
- Item: `data-selected`/`data-highlighted`/`data-disabled`; ItemIndicator
  (check mark slot) visible when selected; `aria-selected`,
  `role="option"`/listbox.
- Multiple: Vec value; trigger label = config-joined labels.

**Platform verdict — significant:** Customizable `<select>`
(`appearance: base-select` + styleable `<selectedcontent>`/`<option>`,
Chrome 135+) is the platform's answer and matches our server-rendered
philosophy perfectly (a REAL select, options in markup, CSS-styled). The
spec REQUIRES the select component docs to present the native-first
recipe; the headless component exists for: multi-select, item alignment,
rich option content beyond what customizable-select allows, and uniform
cross-browser behavior TODAY. Re-evaluate the default when
customizable-select reaches Baseline.

## combobox

Data-attribute surface (cold-review fix — previously undocumented):
- **Input**: `data-popup-open`, `data-popup-side` (mirror of the popup's
  final side), `data-list-empty`, `data-pressed`, + the field six.
- Trigger/Clear/Chips: `data-popup-open`, `data-pressed`,
  `data-disabled`; Chip: `data-highlighted`.
- Popup/List: `data-empty` when zero results (consumed by combobox.css
  and autocomplete.css), `data-side`/`data-align` per M1.
- Items: `data-selected`/`data-highlighted`/`data-disabled` as select.

Select + a text input filtering the option list. Parts add: Input,
InputGroup, Clear, Chips/Chip/ChipRemove (multi as tokens), Status
(live-region result count), Empty (no-results), Row (grid mode),
Collection, Trigger (optional separate arrow-button).

Captured beyond select:

| prop | ours |
|------|------|
| `inputValue` triple | `(query, set_query)` signal |
| `filter: fn(item, query) \| null` | DEFAULT: `filtered = ctx.computed(items × query)` using base-ui's named filter set: `contains` (default) / `starts_with` / `ends_with`, locale-lowercase folded with base-letter (diacritic-folding) sensitivity per the static `locale`; `null` = server-filtered (see below); custom fn supported |
| `filteredItems` (externally controlled list) | pass a `SignalGetter<Vec<PickItem>>` instead of static items — THE SERVER-DRIVEN PATH: query signal → mount-stream/fetch (feature 04) → items signal. First-class in ours, not an afterthought. |
| `autoHighlight = false` | highlight first result as you type |
| `openOnInputClick = true` | config |
| `limit = -1` | cap rendered results |
| `locale` | filter collation config |
| `grid = false` | 2D results (emoji-picker style) — M5 grid arrows; Row part |
| `inline = false` | inline autocomplete (input completes as you highlight) |
| `virtualized` | NOT PORTED v1 — revisit with `<For>` windowing later |
| `onItemHighlighted` | optional callback |
| Clear part | button clearing query+value, `data-disabled` when empty |
| Chips | multi-select as removable tokens IN the input group; Backspace removes last; `data-highlighted` chip navigation by arrows |
| Status/Empty | `role="status"` live region ("5 results"), empty-state slot |

Keyboard: input keeps DOM focus ALWAYS (virtual highlight in the list);
↑/↓ move highlight (open on ↓ when closed), Enter selects (multi: keeps
open + clears query per config), Escape clears highlight then closes,
Backspace in empty input removes last chip (multi).
A11y: `role="combobox"` + `aria-expanded` + `aria-controls` on input,
`aria-activedescendant` tracks highlight, `aria-autocomplete="list"`
(`"both"` when inline).

**Platform verdict:** `<datalist>` covers only trivial string suggestions
(no styling, no multi, inconsistent) — headless justified. The Status
part's live-region pattern is mandatory a11y, easy to forget: spec'd as a
built-in, not a slot.

## autocomplete

base-ui ships it as a SEPARATE component with the same anatomy where the
VALUE IS THE INPUT TEXT itself (items are suggestions). Captured API
(cold-review fix — the mode taxonomy was lost): **`mode: 'list' | 'both'
| 'inline' | 'none'`** — `list`: filter the list only; `both`: filter AND
inline-complete the input with the highlighted item's remainder (selected
range over the completed span); `inline`: inline-complete without
filtering; `none`: neither (external control). Plus `keepHighlight`
(preserve highlight across re-filters) and `submitOnItemClick`.
`aria-autocomplete` mirrors the mode. Ours: `autocomplete()` = combobox
specialization (`value ≡ query`, no chips, no hidden value input — the
input IS the form field) + the mode config. All filtering/server options
identical.

## Shapes

```rust
pub struct PickItem<T> { pub value: T, pub label: String, pub disabled: bool }
pub enum PickSource<T> {
    Static(Vec<PickGroup<T>>),                    // client filter via computed
    Reactive(SignalGetter<Vec<PickGroup<T>>>),    // server/driven lists
}
pub fn select<T: Clone + PartialEq + 'static>(ctx, rcv, cfg: SelectConfig<T>, value, set_value, source: PickSource<T>, slots{trigger_value, item_indicator, empty}) -> Html;
pub fn combobox<T>(ctx, rcv, cfg: ComboboxConfig<T>, value, set_value, query, set_query, source, slots{...}) -> Html;
pub fn autocomplete(ctx, rcv, cfg, query, set_query, source: PickSource<String>, slots) -> Html;
```

Machinery: feature 01 `<For>` (lists), M1 (+item-alignment extension,
documented), M3, M5 (virtual highlight + grid), M6, M7. Tests: full
keyboard matrices (closed-trigger typeahead!, chip editing), filter
computed correctness incl. locale + limit, server-source swap (items
signal replaces list — `<For>` keyed correctness), live-region status
text, hidden-input serialization single/multi, native-select recipe in
docs verified via `to_markup`.

## Reference CSS (vendored — the styling acceptance criteria)

[select](../styling/select.css) · [combobox](../styling/combobox.css) · [autocomplete](../styling/autocomplete.css)

Per [styling/README.md](../styling/README.md): each implementation must
satisfy its reference stylesheet's selectors (data-attributes, CSS vars)
with only the mechanical adaptations listed there; the adapted file becomes
the component's opt-in default stylesheet.

