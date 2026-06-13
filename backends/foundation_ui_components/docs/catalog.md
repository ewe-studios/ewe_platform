# The catalog

All eight families ship (spec-42 feature 05). Each component is a function
returning `Html`; reactive ones take `&Context` + `&SharedInstructionReceiver`.

| Family | Components |
|--------|-----------|
| **F1 primitives** | `button` · `toggle` · `toggle_group` · `separator` · `avatar` |
| **F2 selection** | `switch` · `checkbox` (+ `checkbox_group`) · `radio_group` |
| **F3 disclosure** | `collapsible` · `accordion` · `tabs` |
| **F4 overlays** | `dialog` · `alert_dialog` · `drawer` · `popover` · `tooltip` · `preview_card` · `toast` (+ `ToastManager`) |
| **F5 menus** | `menu` (+ `MenuEntry`) · `context_menu` · `menubar` · `navigation_menu` · `toolbar` |
| **F6 pickers** | `select` · `combobox` · `autocomplete` (over `PickItem`) |
| **F7 form** | `field` · `input` · `fieldset` · `form` · `number_field` · `otp_field` |
| **F8 indicators/surfaces** | `progress` · `meter` · `slider` · `scroll_area` · `skeleton` |

Every reactive component follows the same call shape and the data-attribute
contract — see **[conventions](./conventions.md)**. F4/F5/F6 compose with
**[machinery](./machinery.md)** behaviors (positioning, dismiss, focus trap,
transitions). F7 builds on `field` — see **[forms & fields](./forms-and-fields.md)**.

---

## F1 primitives (detailed)

### `button` / `button_with_click`
`<button type="button">` with content + optional `loading` signal (→ `data-loading`
+ `aria-busy`). `button_with_click` adds a `Callback`. `focusable_when_disabled`
keeps it in tab order via `aria-disabled`. Data attrs: `data-disabled`,
`data-loading`.

```rust
let go = ctx.callback(move |_| { /* … */ });
button_with_click(&ctx, &rcv, ButtonConfig::default(), ButtonSlots::single("Save"), None, go);
```

### `toggle`
A two-state `<button aria-pressed>`; clicking flips the `pressed` signal (no `role`
— native button + `aria-pressed` IS the contract). Data attr: `data-pressed`.

### `toggle_group`
`<div role="group">` over toggles sharing a `Vec<String>` selection (single or
`multiple`). Items render via `<For>` (correct ids + live `data-pressed`); clicks
update the Vec. Carries `data-composite` so the roving-focus machinery can attach.
Data attrs: `data-orientation`, `data-disabled`, `data-multiple`.

### `separator`
Pure `<div role="separator">` with `aria-orientation`/`data-orientation`;
`aria-hidden` present only when decorative. No `ctx` needed.

### `avatar`
Image with fallback. `load`/`error` events (via `ctx.callback`) drive an internal
status signal; the fallback hides once `loaded` (`data-hidden`). `fallback_delay_ms`
→ the `--avatar-fallback-delay` CSS var (CSS owns the timing). Data attr:
`data-loading-status` (idle/loading/loaded/error).

---

## Notes on the other families

- **F2 selection** (`switch`, `checkbox`, `radio_group`) — value-carrying controls;
  bind with a `SignalSetter`. `radio_group` uses the move-and-select roving
  machinery; groups manage a shared selection.
- **F3 disclosure** (`collapsible`, `accordion`, `tabs`) — open/active state in
  signals; reflected as `data-open`/`data-active`. `tabs` wires roving focus across
  the tablist.
- **F4 overlays** — `dialog`/`alert_dialog`/`drawer` use scroll-lock + focus-trap +
  dismiss; `popover`/`tooltip`/`preview_card` use anchored positioning + dismiss +
  transitions; `toast` + `ToastManager` manage a queue/region.
- **F5 menus** — `menu`/`context_menu`/`menubar`/`navigation_menu`/`toolbar` use the
  composite roving machinery (+ positioning for floating menus).
- **F6 pickers** — `select`/`combobox`/`autocomplete` are generic over `PickItem<T>`;
  `SelectConfig<T>.to_form_value` bridges to form submission.
- **F8 indicators/surfaces** — `progress`/`meter`/`slider`/`scroll_area`/`skeleton`;
  `slider` uses pointer-gesture machinery for drag/scrub.

Per-component build notes + gotchas live in
`specifications/42-ui-component/features/05-headless-components/` and
`specifications/42-ui-component/LEARNINGS.md`.
