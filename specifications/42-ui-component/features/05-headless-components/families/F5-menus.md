# F5 — Menus: menu, context-menu, menubar, navigation-menu, toolbar

Source: base-ui `types.md` references (2026-06-13). Builds on F4's overlay
model (open signal, positioning table, hover config, transitions) — only
deltas are captured here.

---

## menu

Parts: Root / Trigger / Portal / Backdrop / Positioner / Popup / Arrow /
Item / Group / GroupLabel / CheckboxItem (+Indicator) / RadioGroup /
RadioItem (+Indicator) / LinkItem / Separator / SubmenuRoot /
SubmenuTrigger.

Root captured (beyond the F4 overlay model):

| base-ui prop | ours | notes |
|--------------|------|-------|
| `highlightItemOnHover = true` | config | hover moves the M5 highlight (vs keyboard-only) |
| `loopFocus = true` | config | M5 |
| `modal = true` | config | menus trap-ish by default (pointer outside dismisses; page inert while open) |
| `closeParentOnEsc = false` (submenu) | config | Escape in a submenu: close just it, or the whole tree |
| `orientation = vertical` | config | arrow axis |
| `disabled` | config | trigger inert |
| trigger `openOnHover`/`delay` (submenu trigger: hover default) | config | F4 hover model |

Items captured:

| part | props | ours |
|------|-------|------|
| Item | `label` (typeahead text override), `onClick`, `closeOnClick = true`, `disabled` | item config + callback; `data-highlighted`/`data-disabled` |
| CheckboxItem | checked triple, `closeOnClick = false` | `(checked, set_checked)` per item; `data-checked/unchecked` + Indicator slot; `role="menuitemcheckbox"`, `aria-checked` |
| RadioGroup/RadioItem | group `value` triple | `(value, set_value)` on the group config; `role="menuitemradio"` |
| LinkItem | anchor item | renders `<a>`; closes menu on activate |
| Group/GroupLabel | — | `role="group"` + `aria-labelledby` |
| Separator | — | F1 separator with `role="separator"` |
| SubmenuRoot/SubmenuTrigger | nested menu; trigger is an Item that opens on hover/→ | recursion of the menu component itself — slots make this natural (a submenu is an item whose slot is another menu) |

Keyboard contract (M5 menu variant — captured fully):
↑/↓ move highlight (loop per config); → opens submenu / ← closes it
(mirrored in RTL); Enter/Space activate; typeahead by `label` or text
content; Escape closes (tree per `closeParentOnEsc`); Tab closes and moves
on. Highlight ≠ focus: base-ui keeps DOM focus on the popup and moves
`aria-activedescendant`-style highlight — ours: `data-highlighted` +
`aria-activedescendant` on the popup (one focus stop, M5 "virtual
highlight" mode — also what select/combobox need).

Open-state niceties captured: opening via KEYBOARD highlights the first
item; via pointer highlights none; ↑ opens highlighting the LAST item.

**Platform verdict:** no native menu (the `<menu>` element is just a list;
"menu bar" proposals stalled). `popover` attr + anchor positioning carry
the popup layer (F4); behavior is ours via M1/M3/M5.

---

## context-menu

Root/Trigger wrap a menu: Trigger is the right-clickable SURFACE
(`contextmenu` event → open at pointer coords via M1 virtual point
anchor; long-press on touch). No hover, no `openOnHover`. Repeated
right-click inside moves the menu. All menu parts/keyboard identical.

---

## menubar

One part wrapping multiple menu roots horizontally.

| prop | ours |
|------|------|
| `orientation = horizontal`, `loopFocus = true`, `modal = true`, `disabled` | config |

Captured behaviors: ←/→ roving focus across top-level triggers (M5);
hovering an ADJACENT trigger while one menu is open opens it without a
click (shared "open intent" across the bar); ↓/Enter from a trigger opens
its menu with first item highlighted; Escape returns focus to the trigger.
`role="menubar"`.

---

## navigation-menu

Website-nav variant (links, mega-panels) — distinct semantics from menu
(it's NOT role=menu; it's nav + disclosure).

| prop | ours |
|------|------|
| `value: any \| null` (+triple) | `(active, set_active)` `Option<String>` — which item's panel is open |
| `delay = 50` / `closeDelay = 50` | hover-intent config |
| `orientation` | config |

Parts: Root (nav) / List / Item / Trigger / Icon / Content / Link /
Backdrop / Portal / Positioner / Popup / Viewport / Arrow.

Captured behaviors: ONE shared popup (Viewport) morphs between items'
Content — `data-activation-direction: left|right` for slide animations,
popup resizes between contents (`--popup-width/height` vars
transitioning); content swaps need mounted panels (visibility model) or
feature 01; triggers `aria-expanded` + panels linked; keyboard:
arrows along the list (M5), Enter/Space/↓ opens, Escape closes, Tab
moves THROUGH panel links. Links: `Link` part = real `<a>` with
`data-active` for current page.

v1 scope: spec'd, implemented AFTER menu/popover prove M1/M5 (most complex
composition, least novel machinery).

---

## toolbar

Pure M5 composite, no popup: Root (`role="toolbar"`) / Button / Group /
Input / Link / Separator.

| prop | ours |
|------|------|
| `orientation = horizontal`, `loopFocus = true`, `disabled` | config |

Captured: arrows rove across ALL interactive children (buttons, inputs,
links — mixed widgets); Tab is ONE stop; Home/End; toolbar buttons can be
toggles (F1 toggle inside toolbar participates in roving). Separator gets
`aria-orientation` flipped vs toolbar orientation.

---

## Shapes

```rust
pub fn menu(ctx, rcv, cfg: MenuConfig, open, set_open, slots{trigger}, items: Vec<MenuEntry>) -> Html;
pub enum MenuEntry {
    Item(ItemConfig, Slot, Callback),
    Checkbox(ItemConfig, Slot, SignalGetter<bool>, SignalSetter<bool>),
    RadioGroup(SignalGetter<String>, SignalSetter<String>, Vec<(ItemConfig, Slot)>),
    Link(ItemConfig, Slot, &'static str),
    Group(Slot /*label*/, Vec<MenuEntry>),
    Submenu(ItemConfig, Slot /*trigger*/, Vec<MenuEntry>),
    Separator,
}
pub fn context_menu(ctx, rcv, cfg, slots{surface}, items: Vec<MenuEntry>) -> Html;
pub fn menubar(ctx, rcv, cfg, menus: Vec<(Slot /*trigger*/, Vec<MenuEntry>)>) -> Html;
pub fn navigation_menu(ctx, rcv, cfg, active, set_active, items: Vec<NavItem>) -> Html;
pub fn toolbar(ctx, rcv, cfg, children: Vec<Slot>) -> Html;
```

`MenuEntry` is the catalog's first STRUCTURED slot enum — items are data,
not markup, because the component owns highlight/typeahead/roles (the
"don't make the caller wire ARIA" principle). Static item sets in v1;
dynamic sets (filtering) arrive with `<For>` and are F6's problem.

Machinery: M1 (point anchors for context-menu), M3 (modal/outside/Escape
tree), M5 (virtual-highlight variant + roving for menubar/toolbar), M7.
Tests: full keyboard matrix per component (incl. RTL arrow mirroring,
typeahead, submenu tree Escape), open-intent menubar hover, checkbox/radio
item state ops, context-menu point positioning, toolbar mixed-widget
roving, aria role/linkage assertions on the op stream.
