# Machinery algorithms — the exact semantics (cold-review fixes, 2026-06-13)

The fresh-eyes review (agent a039cb3c) verdicted YES-WITH-FIXES: the family
docs named behaviors whose ALGORITHMS lived only in base-ui source. This
file pins them, with source-of-truth citations, so no implementer ever
needs the React code.

## M1 — Collision handling (the full Positioner semantics)

Ported verbatim from base-ui `Positioner.collisionAvoidance`
(popover/types.md): three independent knobs.

- **`side`** — overflow on the preferred placement axis:
  - `flip` (default): keep the requested side when it fits; otherwise try
    the opposite side (top↔bottom, or left↔right).
  - `shift`: never change side; move the popup along that axis within the
    clipping boundary so it stays visible.
  - `none`: no side-axis correction.
- **`align`** — overflow on the alignment axis:
  - `flip` (default): keep side, swap `start`↔`end` when the requested
    alignment overflows.
  - `shift`: keep side and alignment, nudge along the alignment axis to
    fit.
  - `none`: no alignment-axis correction.
- **`fallbackAxisSide`** — when the preferred axis cannot fit at all:
  - `start`: allow perpendicular fallback, trying the logical start side
    first (top before bottom; left before right in LTR).
  - `end`: logical end side first.
  - `none`: never fall back to the perpendicular axis.
- Constraint rule: when `side: 'shift'`, an explicit `align` may only be
  `'shift'` or `'none'`; omitted `align` then defaults to `'flip'`.

Defaults (CORRECTING F4's earlier "(fixed default)" note):
`positionMethod = 'absolute'`, `collisionBoundary = 'clipping-ancestors'`,
`collisionPadding = 5`, `sticky = false`, `arrowPadding = 5`.
`data-side`/`data-align` always reflect the FINAL post-collision placement.
`data-anchor-hidden`: present when the anchor is fully clipped out of every
collision boundary (popup hides with it unless `sticky`).

CSS vars: positioner emits `--anchor-width/height`,
`--available-width/height`, `--positioner-width/height`,
`--transform-origin`; the POPUP additionally emits
`--popup-width/--popup-height` (its own measured size — consumed by
popover.css and the navigation-menu resize transition).

## M5 — Typeahead (source: floating-ui-react useTypeahead.ts)

- Keystrokes append to a buffer; the buffer RESETS after **750 ms**
  (`resetMs`) of inactivity.
- Match: `label.toLocaleLowerCase().startsWith(buffer)` over VISIBLE,
  enabled items, scanning forward from the item after the current
  highlight (wrap-around).
- Repeated same letter ("a", "a", …) cycles through items starting with
  that letter instead of accumulating "aa".
- Space is part of the buffer once typing has started (single leading
  Space activates instead).

## M2/M3 — Scroll lock for non-dialog modals (source: useScrollLock.ts)

select/menu/popover with `modal: true` lock page scroll WITHOUT a native
`<dialog>`:
- If `CSS.supports('scrollbar-gutter','stable')`: set the scroll
  container's `overflow-y: hidden` with `scrollbar-gutter: stable` so
  layout doesn't shift.
- Otherwise: `overflow-y: scroll` is preserved on the locked container to
  keep the scrollbar gutter (no width jump).
- Lock target: `<html>` unless `<body>` carries its own overflow styles
  (then lock body) — both originals restored on unlock.
- Inertness for `modal: true` (non-dialog): pointer interactions outside
  are disabled by the M3 outside-click guard consuming the event
  (capture-phase preventDefault), NOT by the `inert` attribute (which
  would break screen-reader access to the live region pattern base-ui
  uses).

## M3 — Hover intent + safe polygon (source: safePolygon.ts)

- Timer model: trigger pointerenter starts an OPEN timer (`delay`);
  pointerleave starts a CLOSE timer (`closeDelay`); entering the POPUP
  cancels the close timer (unless `disableHoverablePopup`); re-entering
  the trigger cancels close.
- **Safe polygon** (submenus + hoverable popups): on trigger pointerleave
  toward the popup, a triangle/polygon between the cursor's exit point and
  the popup's near corners keeps the popup open while the cursor moves
  inside it; leaving the polygon (or stalling beyond the close delay)
  closes. Option `blockPointerEvents` additionally suppresses pointer
  events on other elements while crossing.
- Per-component defaults (CORRECTIONS included): popover trigger
  `delay = 300`, `closeDelay = 0`; **tooltip `delay = 600`**,
  `closeDelay = 0`, provider `timeout = 400` (grouping: moving between
  grouped triggers within 400 ms skips the open delay); preview-card
  `delay = 600`, `closeDelay = 300`; **submenu trigger `delay = 100`**;
  navigation-menu `delay = 50`, `closeDelay = 50`;
  **context-menu long-press = 500 ms** (touch).

## M7 — `data-instant` (transition suppression)

Emitted on popups alongside open-state changes when the change should NOT
animate; value names the cause: `click` (toggled by pointer on the
trigger), `dismiss` (Escape/outside), `focus` (focus-driven open/close),
`trigger-change` (popup re-anchored to a different trigger — payload
switches). Consumer CSS (tooltip.css, menubar.css, navigation-menu.css)
disables transitions under `[data-instant]`. Implementations MUST emit it
for those four causes; M7's transitionend bookkeeping skips
starting/ending styles when present.

## M8 — Hold-repeat + scrub constants (source: number-field constants.ts)

- Increment/decrement hold: first repeat after **400 ms**
  (`START_AUTO_CHANGE_DELAY`), then every **60 ms**
  (`CHANGE_VALUE_TICK_DELAY`).
- Scrub: `pixelSensitivity = 2` (pointer px per step tick),
  `SCROLLING_POINTER_MOVE_DISTANCE = 8` (movement before a wheel/touch
  gesture counts as scrolling, not scrubbing), virtual cursor teleports
  across the viewport edge (`teleportDistance`).
- v1 stylesheet compatibility rule: movement CSS vars consumed
  unconditionally by vendored CSS (`--toast-swipe-movement-x/y`,
  `--drawer-swipe-progress`, `--drawer-swipe-strength`) MUST be
  initialized to `0px`/`0` at mount even while gestures are deferred.

## Event reason taxonomy (cross-cutting)

base-ui's `ChangeEventDetails.reason` unions ride along in our `EventData`
payload (`reason` field) for every component callback, not just tabs.
Reasons worth implementing exactly: number-field
(`input-change/input-clear/input-blur/input-paste/keyboard/
increment-press/decrement-press/wheel/scrub`), overlay dismiss reasons
(`trigger-press/outside-press/escape-key/focus-out/hover-leave/
list-navigation/item-press` etc. per component), tabs
(`none/initial/disabled/missing`). `cancel()` ≙ the handler not writing
the signal; automatic (non-user) changes are not cancelable.

## Machinery delivery: `register_function` JS handles (decided 2026-06-13)

The JS-runtime modules (M1/M3/M5/M7) are NOT a monolithic hand-maintained
`foundation-wasm-ui.js`. Each behavior's JS lives **colocated beside its Rust
contract** in a central `machinery` module and is delivered through the host
function-registry ABI (`foundation_wasm::host_runtime::web`):

- **Backbone.** `register_function(code: &str) -> HostFunction{ handler: u64 }`
  compiles a JS **function expression** (`Function("…return("+src+")")()`);
  `invoke_as_*(handler, &[Params])` calls it. The fn runs with `this` = the
  FunctionRegistry `context`. (host_runtime.rs §web; foundation-wasm.js
  `FunctionRegistry`.)
- **Delivery = SCOPED SCRIPTS (FINAL, 2026-06-13 — supersedes both the
  `this.node` and the raw-`<script>`+`querySelector` plans).** Reuse the
  EXISTING scoped-script hydrator (spec-39 feature 09 §5): each behavior emits
  a `<script scoped primal:script>function(scope){…}</script>` node inside the
  component root. The hydrator runs it with a `scope` object:
  - `scope.parent()` / `scope.targets()` → the component element(s) directly —
    no `primal-id` plumbing, no `querySelector` guesswork.
  - `scope.addEvent(target, type, handler)` → listener with AUTO-CLEANUP on
    disconnect, so idempotency-on-re-attach is handled for free.
  - `scope.querySelector(sel)` (parent-scoped) + `primal.on/onclick/onchange`
    globals. Bubbled events: one listener on `scope.parent()`, read
    `event.target` (M5 key handling, M3 outside-click).
  Per-behavior OPTIONS ride the parent's data-attributes (which the component
  already sets — `data-orientation`, `data-loop`, …); the body reads them off
  `scope.parent()`. No `register_function`, no `cfg(wasm32)` gating, no
  mount-timing dance, no new JS infra. `register_function`+`invoke` remains a
  possible later optimization but is NOT needed.
- **One integration point to verify (§5.4):** scoped scripts OUTSIDE an
  `<island>` are compile-time only — the MutationObserver does not hydrate
  runtime-inserted ones. Reactive (runtime-mounted) components must therefore
  hydrate within an island's `connectedCallback`, OR the observer is extended
  to run scoped scripts on insertion. Confirm/choose before building M5.
- **Per behavior.** A `&str` JS-source const (function expression) colocated
  with the Rust contract + a typed Rust `attach(node_id, …opts) -> Html` that
  returns the self-invoking `<script>` node the component embeds (no `cfg`
  gating — it's just markup).
- **Timing.** Solved by construction: the script runs after its target element
  exists (parse-time for SSR; insertion-time for the `createElement`-built
  wasm node). No `requestAnimationFrame` dance needed, though a body MAY still
  rAF for layout reads.
- **Attribute contract still holds.** Bodies WRITE the documented outputs
  (`data-side`/`data-align`, `--anchor-*` vars, roving `tabindex`, etc.) so CSS
  + morph see the same contract regardless of delivery.

Build order: backbone `JsBehavior` helper → M5 (roving/typeahead, self-contained)
→ M3 (dismiss) → M1 (positioning) → M7 (transitions/measurement); M2 is mostly
native (`<dialog>.showModal()` / `popover`), needing only open/close + scroll-lock
helpers.

## RTL policy (cross-cutting)

`dir` is static config, but its consequences are spec'd here once:
- M1 `inline-start/inline-end` sides and `fallbackAxisSide` logical order
  resolve against the anchor's computed direction.
- M5 arrow mirroring: ←/→ swap meaning in RTL composites (menus, tabs
  horizontal, sliders); ↑/↓ never mirror.
- `data-activation-direction` reports LOGICAL direction post-resolution.
- F6 chips: Backspace removes the LAST chip in logical order; arrow
  traversal follows logical order.
- F8 progress/slider indicators use inset-inline/logical properties.
