# F4 — Overlays: dialog, alert-dialog, drawer, popover, tooltip, preview-card, toast

Source: base-ui `types.md` references + page.mdx guides (2026-06-13). The
family rides M1 (positioning), M2 (layering), M3 (dismiss), M4 (focus),
M7 (transitions) — and is where "**use new CSS/platform standards as
alternative when sensible**" is most decisive, so each section opens with
the platform verdict.

## The shared overlay model (capture once)

Every overlay in base-ui shares:
- `open`/`defaultOpen`/`onOpenChange` → ours ONE `(open, set_open)` signal.
- `onOpenChangeComplete(open)` → fires AFTER animations finish → ours: M7
  transitionend hook, optional callback.
- `actionsRef.unmount` (manual unmount for external animation libs) → NOT
  PORTED — v1 keeps overlays mounted (visibility model, consistent with
  F3); M7 + `@starting-style` cover animations.
- Trigger `payload` + `handle`/`triggerId` (one overlay, many triggers,
  payload tells content who opened it) → ours: `(active_trigger,
  set_active_trigger)` optional `Option<String>` signal set by triggers
  before opening; content reads it like any signal. No handle type needed
  — signals ARE the handle.
- Popup `data-open/closed`, `data-starting-style`/`data-ending-style`;
  dialog adds `data-nested` / `data-nested-dialog-open` (style stacked
  dialogs — capture: nested dialog pushes parent back via these attrs +
  `--nested-dialogs` count variable).
- Focus: Popup `initialFocus` / `finalFocus` (bool | element | fn of
  interaction type `mouse|touch|pen|keyboard`) → ours M4 config:
  `InitialFocus::{Default, None, Target(id)}` + restore-to-trigger default
  on close; the interaction-type refinement is an M4 JS concern (keyboard
  opens focus first tabbable; pointer opens focus popup container).

## dialog — platform verdict: **native `<dialog>`, fully**

`showModal()` gives top-layer, focus trap, Escape (`cancel` event),
`::backdrop`, inertness of the page — i.e. M2+M4+half of M3 for free.

| base-ui | ours |
|---------|------|
| `modal: true \| false \| 'trap-focus'` | `true` → `showModal()`; `false` → `show()` (non-modal); `'trap-focus'` (trap without scroll-lock/inertness) → `show()` + M4 trap. Scroll-lock when modal: native. |
| `disablePointerDismissal = false` | config; outside-click close implemented in M3 (native dialog has no light dismiss; Escape is native via `cancel`) |
| Backdrop part | `::backdrop` pseudo-element — a styling recipe, NOT a DOM part in ours; `forceRender`-style nested handling via `data-nested` attrs |
| Portal part | NOT PORTED — top layer makes portaling moot (the whole reason base-ui portals is stacking contexts; the platform solved it) |
| Trigger | `<button commandfor="dialog-id" command="show-modal">` (Invoker Commands, F1) with JS fallback wiring the open signal; trigger sets `active_trigger`/payload first |
| Title/Description parts | slots; auto-wire `aria-labelledby`/`aria-describedby` to their generated ids |
| Close part | slot/button writing `set_open(false)`; REQUIRED inside modal popups (captured a11y note: touch screen readers need an explicit escape) |
| Viewport part | not ported v1 (base-ui uses it for scrollable-dialog layout; CSS handles it) |

Captured behaviors to preserve: open/close via signal must stay in sync
with native open/close (cancel event → `set_open(false)`; signal effect →
`showModal()/close()` via SetProperty-style op — M2 exposes
`primal:dialog="modal|nonmodal"` attribute contract the JS runtime wires).
Nested dialogs: count + attrs. Keyboard: Escape native; focus trap native.

## alert-dialog

Dialog variant, captured deltas: `role="alertdialog"`, ALWAYS modal, NO
light dismiss ever (Escape only), initial focus defaults to the
least-destructive action (config target). Same parts minus hover anything.
Ours: `alert_dialog(...)` thin wrapper over dialog with these flags fixed.

## popover — platform verdict: **`popover` attribute + CSS anchor positioning, with M1/M3 fallback**

The `popover="auto"` attribute gives top-layer + light dismiss (outside
click AND Escape) natively; `popovertarget` wires the trigger
declaratively. CSS Anchor Positioning (`anchor-name` / `position-anchor` /
`position-area` / `position-try-fallbacks`) covers side/align/flip. Where
anchor positioning is unavailable, M1 (our floating module) positions the
same markup — the spec REQUIRES both paths behind one attribute contract:
`primal:anchor="<trigger-id>"`, `primal:side`, `primal:align`.

Full positioning API captured from Positioner (this table IS the M1 spec —
referenced by tooltip/preview-card/menus/select):

| positioner prop | ours (M1 config / attrs) |
|------------------|---------------------------|
| `side: top\|bottom\|left\|right\|inline-start\|inline-end` (+`align: start\|center\|end`) | static config → `data-side`/`data-align` REFLECT the FINAL placement after collision handling (CSS hooks for arrow + origin) |
| `sideOffset`/`alignOffset: number \| fn(anchor,popup,side)` | static numbers v1 (offset functions deferred until needed) |
| `anchor` (element/virtual/ref) | trigger by default; any `primal-id` or a point (context-menu) |
| `collisionAvoidance` (flip/shift modes), `collisionBoundary` (clipping ancestors default), `collisionPadding = 5`, `sticky` | M1: flip-then-shift default, boundary = viewport/clipping ancestors, padding config; `sticky` keeps popup attached when anchor scrolls out |
| `positionMethod: absolute\|fixed` | M1 detail (fixed default) |
| `arrowPadding = 5`, Arrow part with `data-side`/`data-uncentered` | arrow = slot positioned via `--transform-origin`-style vars |
| `disableAnchorTracking` | static — skip scroll/resize observers |
| CSS vars: `--anchor-width/height`, `--available-width/height`, `--positioner-width/height`, `--transform-origin` | M1 emits ALL (they're the headless styling contract: match-trigger-width selects, max-height clamping, scale-from-origin animations) |
| `data-anchor-hidden` | present when anchor scrolled out of view |

Trigger hover mode (popover/preview-card/tooltip share it): `openOnHover =
false`, `delay = 300`, `closeDelay = 0` — M3-adjacent hover-intent JS
(pointer enter/leave with delay timers, popup itself hoverable keeps it
open). `modal: false` default for popovers (true traps focus — rare).

## tooltip

Popover specialization, hover/focus-only (NEVER click), non-focusable
content, `role="tooltip"`:
- Provider: `delay`, `closeDelay`, `timeout = 400` — GROUPING: moving
  between grouped triggers within `timeout` skips the open delay (the
  "tooltip walk" UX). Ours: `TooltipGroup` config value shared by tooltip
  instances (plain Rc'd struct, no provider component).
- `trackCursorAxis: none\|x\|y\|both` — popup follows the cursor on an
  axis (M1 virtual-anchor mode).
- `disableHoverablePopup` — popup not hoverable (closes on leave).
- `disabled` — render trigger only.
- Opens on focus-visible too; Escape closes; never traps.
- **Platform**: `popover="hint"` (Chrome 133+) is EXACTLY tooltip
  semantics (doesn't close auto popovers); use when available; M1/M3
  fallback otherwise. `interestfor` (interest invokers) is the eventual
  zero-JS path — watch, don't depend.

## preview-card

Popover with `openOnHover` fixed true, link-preview semantics
(`delay=600/closeDelay=300` defaults), no modal, no focus management
(content is supplementary). Thin wrapper over the popover machinery.

## drawer

Dialog variant + gestures; captured distinctives:
- `swipeDirection = 'down'` (up/down/left/right) — swipe-to-dismiss.
- `snapPoints: (number|string)[]` + `(snap_point, set_snap_point)` signal
  + `snapToSequentialPoints` (don't skip points on fast swipes) — partial
  heights the drawer rests at.
- Parts: dialog parts + SwipeArea, Indent/IndentBackground (iOS-style
  page-scale effect).
- CSS vars for swipe translation (toast-style movement vars).
- **v1 scope decision**: ship `drawer` as side-anchored dialog (open/close
  signal, M7 slide transitions) WITHOUT swipe/snap; gestures live in a
  shared `pointer-gestures` JS module (with slider drag + number-field
  scrub) in a follow-up iteration. snapPoints API reserved in config now so
  signatures don't break.

## toast

The one overlay where the LIST is the component.
- Manager (base-ui `createToastManager` / `useToastManager`): `add`,
  `close`, `update`, `promise(promise, {loading, success, error})` —
  ours: `ToastManager` Rust struct over a `(toasts, set_toasts)` Vec
  signal; `promise` becomes a valtron-friendly variant (update on task
  completion). Toast value: `{ id, type, title, description, timeout,
  priority, action }`.
- Behaviors captured: auto-dismiss timeout (pause on hover/focus of the
  viewport — captured!), `limit` with `data-limited` for overflowed
  toasts, stacking with `--toast-index`/`--toast-height`/
  `--toast-offset-y` vars + `data-expanded` (viewport hover expands the
  stack), swipe-to-dismiss with `--toast-swipe-movement-x/y` +
  `data-swiping` + `data-swipe-direction` (gesture module — same deferral
  as drawer; timeout/limit/stack ship in v1), `data-type` for styling per
  toast type.
- A11y: viewport is `role="region"` + aria-live (polite; assertive for
  `priority: 'high'`); F6 keyboard: hotkey to jump to viewport (config).
- Rendering the list NEEDS feature 01 `<For>` — toast is the catalog's
  `<For>` proving component. Viewport = `popover="manual"` (top layer, no
  light dismiss).

## Shapes

```rust
pub fn dialog(ctx, rcv, cfg: DialogConfig, open, set_open, slots: DialogSlots{title, description, children, close_label}) -> Html;
pub fn alert_dialog(...same, cfg fixed: modal, no pointer dismiss...) -> Html;
pub fn popover(ctx, rcv, cfg: PopoverConfig{position: PositionConfig, hover: Option<HoverConfig>, modal}, open, set_open, slots{trigger, children, arrow}) -> Html;
pub fn tooltip(ctx, rcv, cfg: TooltipConfig{group, track_cursor_axis, ...}, slots{trigger, content}) -> Html;  // open signal internal
pub fn preview_card(...) -> Html;
pub fn drawer(ctx, rcv, cfg: DrawerConfig{side, snap_points reserved}, open, set_open, slots) -> Html;
pub struct ToastManager { /* add/close/update/promise over a Vec signal */ }
pub fn toast_viewport(ctx, rcv, cfg, manager: &ToastManager) -> Html;     // needs <For>
```

Machinery: M1 (popover/tooltip/preview-card/menus later), M2 (`<dialog>`,
`popover` attr), M3 (dismiss incl. hover intent), M4 (focus config), M7
(all). Tests: open/close signal⇄native sync (cancel event round-trip),
light-dismiss matrix (modal/non-modal × pointer-dismissal flag), nested
dialog attrs, positioning data-attrs + CSS vars on the op stream (JS suite
with a stub layout), tooltip group delay-skip timing, toast manager
add/limit/timeout-pause semantics, `commandfor`/`popovertarget` attributes
present in `to_markup` output (the zero-JS server-rendered path).
