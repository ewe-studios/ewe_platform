# Token-scale design guide — how we generate calculable utility tokens

**Status:** Active design guide · **Owner:** theme system (`foundation_theme`)
· **Related:** decision 020 (theme system), decision 021 (`theme!{}` + runtime
codegen), feature 09 §7 (utility tables).

This guide is the *thinking* behind every generated utility class. The goal:
tokens that are **calculable** — every class's value is a pure function of an
index, so the system is predictable, uniform, and complete within its range
(no "we forgot `opacity-35`").

---

## 1. Two kinds of token

1. **Named (semantic) tokens** — author intent: `primary`, `secondary`, `md`,
   `soft`. Declared in `theme!{}` / `Theme` builder; values are arbitrary
   (`#3b82f6`, `0 1px 2px …`). They generate `.bg-primary`, `.p-md`,
   `.shadow-soft`. Use when the *meaning* matters and the value is bespoke.

2. **Scale (numeric) tokens** — generated, calculable: `.opacity-50`, `.w-75`,
   `.p-16`, `.text-20`, `.font-700`. The class index IS the value's input.
   Use for the regular, repeating dimensions of a design (sizes, spacing,
   opacity, weight).

Named and scale tokens **coexist by namespace**: a class suffix is either a
*name* (`md`, `primary`) or a *number* (`16`, `50`). They never collide, so
`.p-md` (token) and `.p-16` (scale) both exist and mean different things.

---

## 2. The calculable principle

A scale is `(.prefix-{n}, property, n ∈ start..=end step Δ, value = f(n))`.
Because `value` is a pure function of `n`, three things follow:

- **Predictable** — to know `.opacity-65`, compute `65/100`; never look it up.
- **Complete** — every `n` in range exists; no gaps, no surprises.
- **Encoded** — the generator stores `(prefix, property, start, end, step,
  kind)`, not a hand-written list. Adding a property is one table row.

This is why generation lives in code (`foundation_theme::SCALES` +
`scale_value`), not in a static CSS blob.

## 3. The units (`Unit`) — and the suffix convention

A scale value is rendered through a **unit**. Pick units by what the number
*physically means*:

| unit | `value(n)` | suffix | for |
|------|------------|--------|-----|
| **Ratio** | `n / 100` | *(bare)* | proportions 0–1: `opacity` |
| **Pct** | `n%` | *(bare)* | sizing against the container: `width`/`height` default |
| **Px** | `n px` | `px` | the spatial grid: spacing/border/font-size |
| **Rem** | `n rem` | `rem` | root-relative sizing |
| **Em** | `n em` | `em` | element-relative sizing |
| **Vh** / **Vw** | `n vh` / `n vw` | `vh` / `vw` | viewport-relative sizing |
| **Raw** | `n` | *(bare)* | unitless CSS enumerations: `font-weight` |

**The suffix convention.** A scale lists one or more units. Each step emits:
- a **bare** class `.{prefix}-{n}` using the *primary* (first) unit — the
  common default (`.text-16` = `16px`, `.w-50` = `50%`, `.opacity-50`);
- a **unit-tagged** class `.{prefix}-{n}-{suffix}` for every unit with a
  non-empty suffix (`.text-16-px`, `.text-16-rem`, `.text-16-em`,
  `.text-16-vh`; `.h-50-vh`, `.w-50-vw`).

So a user picks the *measured property* by suffix and never has to remember
which unit a bare class implies. `Ratio`/`Pct`/`Raw` are bare-only (a percent
or a weight has no competing unit), so they emit no `-suffix` twin.

**Why 5 for ratios/percent and the grid in pixels** is deliberate:
- *Ratios/percent* read as human percentages (`opacity-50` = "half"). Step 5
  gives 21 round stops, no ugly decimals.
- *Spatial* values follow a 4px base grid (8px rhythm = 2 steps, 16px = 4) —
  the "8-based design" convention expressed as a 4px atomic unit so half-steps
  (4, 12, 20) stay available. The same index is offered in `rem/em/vh` so the
  author selects the right relative unit without a new number.

## 4. The current scale table

(`foundation_theme::SCALES` — change here, regenerate everywhere.)

| prefix | property | range / step | units (primary first) |
|--------|----------|--------------|------------------------|
| `opacity` | `opacity` | 0–100 / 5 | Ratio |
| `w` | `width` | 0–100 / 5 | Pct, Vw, Vh |
| `h` | `height` | 0–100 / 5 | Pct, Vh, Vw |
| `p` | `padding` | 0–64 / 4 | Px, Rem |
| `m` | `margin` | 0–64 / 4 | Px, Rem |
| `gap` | `gap` | 0–64 / 4 | Px, Rem |
| `text` | `font-size` | 8–72 / 2 | Px, Rem, Em, Vh |
| `border` | `border-width` | 0–8 / 1 | Px, Rem, Em |
| `font` | `font-weight` | 100–900 / 100 | Raw |

Endpoints inclusive. `opacity-0 = 0`, `opacity-100 = 1` (special-cased so they
aren't `0.00`/`1.00`). The index is the literal magnitude in each unit
(`.text-56-rem` = `56rem`) — the author picks the sensible unit per use.

## 5. Rules for adding a scale

1. **Choose the family by physical meaning** (§3), not by the property name.
2. **Pick a step that yields round, human values** — 5 for proportions, the
   grid base for spatial, the CSS enumeration for enumerated.
3. **Keep ranges honest** — only emit what a real design uses (e.g. font-weight
   stops at 900 because CSS does; width stops at 100% ). Avoid vanity ranges
   that bloat the stylesheet.
4. **Mind the namespace** — a numeric scale never collides with a named token,
   but two *numeric* scales sharing a prefix would. One prefix, one property.
5. **Add a table row, not strings** — `Scale { prefix, property, start, end,
   step, units }`. If a property needs a unit the set doesn't cover, add a
   `Unit` variant with its `suffix()` + `value()` arms, and document it here.

## 6. Non-goals

- **No arbitrary-value escape** (`p-[13px]`) — that's the named-token job
  (declare it in `theme!{}`) or inline `style`. Scales are the *regular* grid.
- **No responsive/state variants in the scale** (`md:`, `hover:`) — those are a
  separate concern (selectors), not value generation.
- **No runtime computation** — scales are generated once (compile-time via the
  macro/derive, or once at `Theme::build()`), never per-render.
