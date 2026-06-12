# F7 — Form: field, fieldset, form, input, number-field, otp-field

Source: base-ui `types.md` references (2026-06-13). This family IS M6 —
the field state machine — plus the input widgets that ride it.

## field (the M6 carrier)

Parts: Root / Label / Control / Description / Error / Validity / Item.

| base-ui prop | ours | static/signal |
|--------------|------|---------------|
| `name` | config | static — inherited by the control inside |
| `disabled = false` | config | static, inherited |
| `invalid` (external override) | optional `SignalGetter<bool>` | for server-driven errors |
| `dirty`/`touched` (external overrides) | not config in ours — they're OUTPUTS (see FieldState) |
| `validate: fn(value, form_values) -> errors \| Promise` | `Validator` config: `fn(&FieldValue, &FormValues) -> Vec<String>`; async via valtron task variant (does not block submit in onSubmit mode — captured caveat) |
| `validationMode: onSubmit \| onBlur \| onChange` (field overrides form) | static config |
| `validationDebounceTime = 0` | static (onChange mode) |

**FieldState (the signal bundle, M6's core):**
```rust
pub struct FieldState {
    pub touched: SignalGetter<bool>,        // control lost focus once
    pub dirty: SignalGetter<bool>,          // value ≠ initial
    pub filled: SignalGetter<bool>,         // has a value
    pub focused: SignalGetter<bool>,        // control focused now
    pub valid: SignalGetter<Option<bool>>,  // None = not yet validated
    pub errors: SignalGetter<Vec<String>>,  // current messages
}
```
Every F2/F6/F7 control accepts `Option<&FieldState>` in config and emits
the six data-attributes from it. The field component OWNS the state and
wires its control slot.

Parts captured:
- **Label**: auto-id label wired to the control (`for`/`id`); clicking
  focuses/activates; works with span-rooted controls (enclosing form).
- **Description**: `aria-describedby` auto-wiring.
- **Error**: shows messages; `match` prop = WHICH ValidityState flag it
  renders for (`valueMissing`, `patternMismatch`, `tooShort`,
  `rangeOverflow`, …, `customError`, or `true` = always) — ours mirrors:
  `ErrorMatch::{Always, Custom, Native(ValidityKind)}`; multiple Error
  parts with different matches per field. Visibility: `data-hidden` until
  feature 01, real unmount after.
- **Validity**: render-prop exposing raw ValidityState → ours: a callback
  slot `fn(&ValiditySnapshot) -> Html` evaluated on validity change. LOW
  priority (escape hatch).
- Native-validity capture: control's ValidityState feeds `valid`/`errors`
  (JS reads `input.validity` flags + `validationMessage` and ships them in
  the event payload — M6 JS-side detail).

## fieldset

Root (`<fieldset>`) + Legend (auto-wired). Pure markup + disabled
inheritance (native fieldset disables descendants — adopt native).

## form

| base-ui prop | ours |
|--------------|------|
| `errors: Record<name, string\|string[]>` (server errors map) | `(errors, set_errors)` signal — THE server-roundtrip seam: submit → server responds field errors (JSON protocol, feature 04) → one setter call paints every field |
| `onFormSubmit(values, details)` | submit callback receiving collected `FormValues`; default prevented when invalid |
| `validationMode = onSubmit` | static, field-overridable |
| `actionsRef` | not ported |

Captured behaviors: submit validates all fields (mode-dependent), focuses
the FIRST invalid control, re-validates invalid fields on change after
submit; native form submission allowed when valid (progressive
enhancement — works with plain HTTP posts, which fits our server story
better than React's).

## input

Single part over `<input>`; `value` signal via the G21 `primal:onchange`
setter path (ALREADY BUILT); all native attributes static config;
field-aware. Nothing else — deliberately thin.

**Platform notes (form-wide):**
- `:user-valid` / `:user-invalid` pseudo-classes (Baseline 2023) replicate
  much of touched+invalid styling with ZERO runtime — document as the
  CSS-first recipe; data-attributes remain for uniformity + non-native
  controls.
- `field-sizing: content` (Chrome 123+) = auto-growing inputs/textareas,
  pure CSS.

## number-field

Parts: Root / Group / Decrement / Input / Increment / ScrubArea /
ScrubAreaCursor.

| base-ui prop | ours | notes |
|--------------|------|-------|
| `value: number \| null` triple + `onValueCommitted` | `(value, set_value)` + optional `on_commit` callback (fires on blur/arrow-release vs every keystroke) | signal |
| `min`/`max`/`step = 'any'`/`smallStep = 0.1`/`largeStep = 10` | static | arrows step; Shift+arrows largeStep; Alt+arrows smallStep (captured modifier map) |
| `snapOnStep = false` | static | snap to step grid on change |
| `allowOutOfRange = false` | static | clamp vs allow-and-invalidate |
| `format: Intl.NumberFormatOptions` + `locale` | static — display formatting (currency, percent, grouping); input parses localized text back (captured: parsing strips group separators, handles locale decimal) |
| `allowWheelScrub = false` | static | wheel over input scrubs value |
| ScrubArea (+Cursor) | pointer-drag scrubbing with virtual cursor (pointer lock) | DEFERRED to the shared pointer-gestures module (with slider drag, drawer swipe) — parts spec'd now |

Behaviors: input is `inputmode="decimal"` text (NOT `type=number` — the
native spinner UX is why this component exists); `role="spinbutton"` ARIA
not used (text input + buttons pattern per base-ui); buttons repeat on
hold (initial delay + acceleration — captured); Home/End → min/max.
Data attributes: scrubbing state on ScrubArea, `data-disabled` etc.

**Platform:** `<input type=number>` documented as the simple-case recipe;
headless justified by formatting/scrub/step-modifiers.

## otp-field

Parts: Root + per-cell Inputs (length static).

| base-ui prop | ours |
|--------------|------|
| `length` (required) | static |
| `value` triple | `(code, set_code)` String signal |
| `autoComplete = 'one-time-code'` | static default — SMS autofill (captured: a single hidden input receives the autofill and distributes) |
| `autoSubmit = false` | static — submits owning form when filled |
| `inputMode = numeric \| text` | static |
| `mask = false` | static — password-style cells |
| `normalizeValue` | optional `fn(&str) -> String` (paste cleanup: strips spaces/dashes) |

Behaviors captured: typing advances focus; Backspace clears + retreats;
←/→ move; paste distributes across cells from the focused cell;
selection-on-focus; only the active cell is tabbable (M5 micro-variant).

## Shapes

```rust
pub fn field(ctx, rcv, cfg: FieldConfig, slots{label, control: Slot, description, errors: Vec<(ErrorMatch, Slot)>}) -> (Html, FieldState);
pub fn form(ctx, rcv, cfg: FormConfig, errors, set_errors, on_submit, children: Vec<Slot>) -> Html;
pub fn input(cfg: InputConfig, value, set_value, field: Option<&FieldState>) -> Html;
pub fn number_field(ctx, rcv, cfg: NumberFieldConfig, value, set_value, slots{decrement, increment}) -> Html;
pub fn otp_field(ctx, rcv, cfg: OtpConfig, code, set_code) -> Html;
```

Note `field` returns `(Html, FieldState)` — the state bundle is an output
the caller threads into the control and any custom UI; this is the
clearest "signals are the API" moment in the catalog.

Machinery: M6 (defined here), M5 micro (otp), pointer-gestures (deferred:
scrub). Tests: validation mode matrix (submit/blur/change + debounce),
server errors map painting fields, first-invalid focus on submit,
ValidityState mapping into errors, number parse/format round-trips per
locale + modifier stepping, otp typing/paste/backspace choreography,
autofill distribution, `to_markup` renders working NATIVE form posts
(progressive enhancement proof).
