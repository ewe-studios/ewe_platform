# Machinery — JS behaviors as scoped scripts

Some behavior is genuinely the browser's job: anchored positioning, light dismiss,
focus trapping, enter/leave transitions, roving focus, pointer gestures. Rather
than a monolithic runtime, each behavior's JS is a `function(scope){…}` colocated
beside its Rust contract and delivered as a `<script scoped primal:script>` node
the component embeds in its root (`machinery::scoped_script`).

## How a scoped behavior runs

The existing scoped-script hydrator runs it once with `scope`:

- `scope.parent()` = the component root,
- `scope.addEvent(...)` auto-cleans on disconnect,

then removes the script — idempotent across morph/re-insert, no `register_function`,
no `cfg(wasm32)` gating. **Options** ride the component's own `data-*` attributes;
**outputs** WRITE the documented `data-*`/CSS-var contract, so CSS + morph see the
same thing regardless of delivery. Embed with `<Fragment>{behavior()}</Fragment>`
(it's element content — see [conventions](./conventions.md)).

A recurring pattern: when JS must change Rust state, it `.click()`s a hidden
element whose `primal:onclick` the component already wired (e.g.
`set_open(false)`) — the same mechanism roving-select uses. No JS→signal bridge.

## The behaviors

| Behavior | Builder | Container contract | Writes |
|----------|---------|--------------------|--------|
| **M5 composite** (roving focus) | `composite::composite_behavior()` | `data-composite` + `data-orientation`/`data-loop`; items `data-composite-item`; optional `data-composite-select` / per-item `data-composite-active` | roving `tabindex`; focuses/clicks members on arrows/Home/End |
| **M3 dismiss** (light dismiss) | `dismiss::dismiss_behavior()` | `data-dismiss` + optional `primal:anchor` / `data-dismiss-escape\|outside="false"`; a hidden `[data-dismiss-action]` wired to `set_open(false)` | clicks the action on Escape / outside-pointer, stamping `data-dismiss-reason` |
| **M1 position** (anchored) | `position::position_behavior()` | `primal:anchor` (or prev sibling) + `primal:side`/`primal:align`/`data-offset`/`data-collision-*` | `data-side`/`data-align`; `--anchor-*`/`--available-*`/`--positioner-*`/`--popup-*`/`--transform-origin`; re-runs on scroll/resize |
| **M7 transition** (enter/leave) | `transition::transition_behavior()` | open signal toggles `data-open`/`data-closed`; optional `[data-transition-complete]` + `data-instant` | `data-starting-style`/`data-ending-style`; fires complete on `transitionend` |

Other machinery (`machinery::*`): M2 `scroll_lock` + `dialog`, M3 `hover`
(hover-intent + safe polygon), M4 `focus_trap`, M5 `listbox` (virtual highlight +
typeahead) + `composite`, M7 `measure`, M8 pointer-gestures (slider drag, scrub,
swipe).

## Who embeds what

`toggle_group` (M5) and `radio_group` (M5 move-and-select) embed theirs today; the
M3/M1/M7 behaviors back the F4 overlay family. Build notes + gotchas:
`specifications/42-ui-component/LEARNINGS.md`.

See also: **[catalog](./catalog.md)** · **[conventions](./conventions.md)**.
