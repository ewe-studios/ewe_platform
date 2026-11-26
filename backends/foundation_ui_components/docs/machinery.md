# Component behaviours

A behaviour installs an interaction on the element that owns it.
Use a helper inside the component root.

```rust
use foundation_ui_components::machinery::dismiss::dismiss_behavior;
let popup = html! { ctx, receiver,
    <div data-dismiss="true">
        <button data-dismiss-action="true">"Close"</button>
        <Fragment>{dismiss_behavior()}</Fragment>
    </div>
};
```

## How a behaviour runs

A pure template writes an empty span with the `data-ewe-behaviour` attribute.
The first-page browser reader finds the marker and installs the binding on its parent.
The reader removes the marker after installation.
A reactive template or action render sends a typed operation that names the same parent.
The browser installs bindings after it creates all children in the frame.

Dialog, dismiss, scroll lock and transition have shared identifiers.
Web, iOS and Android each provide their own implementation for the shared identifier.
The remaining seven behaviours have web bindings.
Gesture and measurement helpers send a text argument named `mode`.
A drawer can attach both swipe and snap to one element. Each mode installs once.

Use `scoped_script(body)` for application-specific browser code.
Its scope supplies `parent()`, `addEvent(target, event, handler)` and `onDispose(cleanup)`.
Removing the owning component releases its event handlers and cleanup callbacks.

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
