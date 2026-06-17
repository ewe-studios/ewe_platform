# F3 — Disclosure: collapsible, accordion, tabs

Source: base-ui `types.md` references, harvested in full 2026-06-13. This
family is where the **platform-alternative** question pays off most —
`<details>` and modern CSS cover a large share of it.

---

## collapsible

Parts: Root (div), Trigger (button), Panel (div).

| base-ui prop | ours | static/signal |
|--------------|------|---------------|
| `open`/`defaultOpen`/`onOpenChange` | `(open, set_open)` | signal |
| `disabled = false` | config | static |
| Panel `keepMounted = false` | config | static — see mounted-ness note below |
| Panel `hiddenUntilFound = false` | config | static — uses `hidden="until-found"` so browser find-in-page can EXPAND the panel (captured behavior: overrides keepMounted; the browser fires `beforematch` → component must flip `open` to true) |

Data attributes: Root + Trigger + Panel all carry `data-open`/`data-closed`
(`data-panel-open` on trigger); Panel adds `data-starting-style`/
`data-ending-style` (M7).

CSS variables (adopt — they're how height animation works headlessly):
`--collapsible-panel-height` / `--collapsible-panel-width` — set from the
panel's measured scroll size so CSS can `transition: height` between 0 and
`var(--collapsible-panel-height)`. Measurement = JS-side (M7 measures on
open/close and sets the custom property via SetStyle).

Mounted-ness note (applies to accordion + tabs panels too): base-ui
unmounts closed panels unless `keepMounted`. For US, v1 keeps panels
mounted with `hidden`/`data-closed` (visibility model — no `<Show>`
dependency); a `keep_mounted: false` mode arrives with feature 01 and uses
scope disposal. The `hiddenUntilFound` path REQUIRES the mounted model —
another argument for mounted-by-default.

Keyboard: Trigger is a native button (Enter/Space). `aria-expanded` +
`aria-controls` on trigger.

**Platform alternative — `<details>/<summary>` (offer as the DEFAULT
simple form):**
- `<details>` gives open state, toggle behavior, a11y, and find-in-page
  expansion natively, ZERO JS.
- `name="group"` attribute (Baseline 2024) gives EXCLUSIVE accordions
  natively — open one, others close.
- `::details-content` pseudo-element + `interpolate-size: allow-keywords`
  (or `calc-size()`) animate open/close height in pure CSS (Chrome/Edge;
  graceful no-animation elsewhere).
- Limits that justify the headless version: `<details>` can't do
  controlled-from-signals semantics cleanly across morphs, multiple-open
  coordination with arbitrary markup shapes, or panel placement away from
  the trigger.
- DECISION for the catalog: ship `collapsible` (headless, signal-driven)
  AND document the `<details>` recipe as the no-runtime alternative;
  `accordion` likewise with `<details name>`.

---

## accordion

Parts: Root (div) → Item (div) → Header (h3) → Trigger (button) +
Panel (div). Multiple collapsibles sharing one value model.

| base-ui prop | ours | static/signal |
|--------------|------|---------------|
| `value: Value[]` (+triple) | `(open_items, set_open_items)` (`Vec<String>`) | signal |
| `multiple = false` | config | static |
| `disabled = false` | config (root) + per-item | static |
| `keepMounted` / `hiddenUntilFound` | config | static (panel model as collapsible) |
| Item `value` (auto-id if omitted) | item config — REQUIRED in ours (we don't auto-generate identity; explicit beats magic) | static |
| Item `onOpenChange` | optional per-item callback | — |
| ~~`loopFocus`~~ / ~~`orientation`~~ | NOT PORTED | base-ui deprecated both following the APG update REMOVING roving focus from accordions — triggers are plain tab stops now. We adopt the post-APG behavior: NO M5 in accordion (correcting feature-05 overview which listed it). |

Data attributes: items/panels/triggers carry `data-open`/`data-closed`,
`data-disabled`, `data-index`; panel CSS vars
`--accordion-panel-height/width` (same M7 mechanism).

Keyboard (post-APG): Tab/Shift+Tab between triggers, Enter/Space toggles.
ARIA: trigger `aria-expanded` + `aria-controls`; panel
`role="region"` + `aria-labelledby` (trigger id); Header is a real heading
element wrapping the trigger.

Single mode: opening one closes the open one (write both in one setter —
one stabilize, glitch-free by construction).

**Platform alternative:** `<details name="x">` group (above). Recipe doc.

---

## tabs

Parts: Root (div) → List (div, `role="tablist"`) → Tab (button,
`role="tab"`) + Indicator (span, decorative) ; Panel (div,
`role="tabpanel"`).

| base-ui prop | ours | static/signal |
|--------------|------|---------------|
| Root `value: Tab.Value` (+triple; `null` = none active; default `0`) | `(selected, set_selected)` (`Option<String>` — we use explicit string values, not indexes) | signal |
| Root `orientation = horizontal` | config | static |
| List `activateOnFocus = false` | config | static — false: arrows move focus, Enter/Space activates (manual); true: arrow focus activates (automatic). M5 parameter. |
| List `loopFocus = true` | config | static (M5) |
| Tab `value` (required) | static per tab | |
| Tab `disabled` | static per tab — CAPTURED SUBTLETY: if the initially-selected tab is disabled, selection falls back to the next enabled tab; base-ui documents this can't be known during SSR — our `to_markup` story has the same constraint, so the spec requires an explicitly enabled initial `selected` value in pure-form renders |
| Panel ties to tab by matching `value` | static per panel | |

Automatic fallback behaviors (captured from the Root reason taxonomy —
`'initial' | 'disabled' | 'missing' | 'none'`): when the selected tab
becomes disabled or is removed, selection falls back to the next enabled
tab (or none). These automatic changes are not cancelable. Ours: the
fallback runs inside the group's setter logic; the optional `on_change`
callback receives the reason in its `EventData` payload.

Data attributes (full set):
- Tab: `data-active`, `data-disabled`, `data-orientation`,
  `data-activation-direction: left|right|up|down|none` (which way
  selection moved — for slide animations),
- Panel: `data-hidden` AND the native `hidden` attribute together (the
  reference stylesheet selects `&[hidden]`), `data-index`,
  `data-orientation`,
  `data-activation-direction`, `data-starting-style`/`data-ending-style`,
- List: `data-orientation` (+ `data-activation-direction`).

Indicator (the animated underline): positioned via CSS variables set from
the active tab's measured box: `--active-tab-left/right/top/bottom/
width/height`. JS-side measurement on activation + resize (same M7/M5
measuring seam as collapsible height). base-ui's
`renderBeforeHydration` is a React hydration concern — not ported (our
runtime sets the vars on mount; no hydration gap by construction).

Keyboard (M5): ←/→ (or ↑/↓ vertical) move through enabled tabs, loop per
config; Home/End jump; manual mode Enter/Space activates. Roving tabindex
— one tab stop. Panels: `tabindex="0"` when they contain no focusable
content (a11y capture).

ARIA: `aria-selected` on tabs, `aria-controls` tab→panel,
`aria-labelledby` panel→tab, `aria-orientation` on list.

**Platform alternatives:**
- No native tabs element exists (the old `<tabs>` proposals died; CSS
  Toggles died). Headless stays justified.
- The INDICATOR however: CSS anchor positioning (`anchor-name` on the
  active tab + `position-anchor`/`anchor()` on the indicator) replaces the
  measured CSS variables in supporting browsers — no resize observer, no
  JS measurement. Spec: emit BOTH (anchor attributes + measured vars);
  document anchor-positioning as the preferred styling path; the measured
  vars remain the portable fallback.
- View Transitions API (`document.startViewTransition`) is the modern
  panel-switch animation path — `data-activation-direction` still serves
  CSS-only slides.

---

## Shapes

```rust
pub fn collapsible(ctx, rcv, cfg, open, set_open, slots{trigger_label, children}) -> Html;
pub fn accordion(ctx, rcv, cfg, open_items, set_open_items, items: Vec<AccordionItem>) -> Html;
pub struct AccordionItem { pub value: &'static str, pub cfg: ItemConfig, pub header: Slot, pub panel: Slot }
pub fn tabs(ctx, rcv, cfg, selected, set_selected, tabs: Vec<TabDef>) -> Html;
pub struct TabDef { pub value: &'static str, pub disabled: bool, pub label: Slot, pub panel: Slot }
```

Machinery: M5 (tabs only — accordion is post-APG plain tabbing), M7
(panel height vars, starting/ending styles, indicator vars).

Tests: open/close data-attribute + hidden flips on the op stream; single
vs multiple accordion exclusivity in ONE stabilize; tabs keyboard contract
(manual + automatic modes, disabled skip, loop); disabled-initial-tab
fallback incl. the documented pure-form constraint; hidden-until-found
panel responds to `beforematch` (JS suite); indicator var updates on
activation; `<details>`/`<details name>` recipes rendered via `to_markup`
and verified as valid alternatives.

## Reference CSS (vendored — the styling acceptance criteria)

[collapsible](../styling/collapsible.css) · [accordion](../styling/accordion.css) · [tabs](../styling/tabs.css)

Per [styling/README.md](../styling/README.md): each implementation must
satisfy its reference stylesheet's selectors (data-attributes, CSS vars)
with only the mechanical adaptations listed there; the adapted file becomes
the component's opt-in default stylesheet.

