# Feature 09 — Status: COMPLETE (2026-06-12)

## What shipped

**Scoped styles (decision 019, compile-time):**
- `transform_scoped_css` in foundation_macros — the §3 five-rule table:
  `:parent` replacement, already-scoped pass-through (id or parent class),
  `&`-nesting pass-through, custom-property pass-through, unscoped prefixing;
  `@media` recursion, other at-rules untouched. Hand-rolled brace walker
  instead of `lightningcss` (dependency budget; swap point documented).
- `html!` integration: `<style primal:style>{"…css…"}</style>` children are
  extracted (CSS must be a quoted string literal — raw CSS does not tokenize
  as Rust), combined per parent (§4), transformed against the parent identity
  (`#id` wins over `.first-class`; neither → compile error with span), and
  REMOVED from the tree. Pure form: one combined `data-primal-scoped`
  `<style>` child. Reactive form: head-injection ops in the same batch
  (create/register/SetText/AppendChild to the reserved `<head>` ambient id 0).
- **Reserved-id fix surfaced by this work**: `allocate_id_block` now starts at
  16 — instance ids no longer collide with the JS registry's ambient seeds
  (0=head, 1=body, 2=html).

**Theme system (decision 020):**
- `#[derive(ThemeTokens)]` in foundation_macros. The spec's nested-literal
  sketch isn't valid Rust; the real syntax carries values in attributes —
  `#[token(category = "color", light = "…", dark = "…")]` — preserving the
  three dark-control levels (omit → auto, partial, full). Generates ONE
  `'static` CSS string: `:root` custom properties, a
  `prefers-color-scheme: dark` block (explicit verbatim; missing colors
  auto-derived at ~80% luminance), §7.1 utility classes per category
  (bg/text/border, p/m, rounded, shadow), §7.2 built-in utilities.
- `foundation_wasm_ui::theme::inject_theme_css` (+`THEME_STYLE_NODE_ID = 3`,
  `HEAD_NODE_ID = 0`): the §8 first-batch sequence.

**Scoped scripts**: the runtime side shipped with F06 (`Hydrator.createScope`,
script execution with per-script error isolation) — §5 needs no macro work.

## Verification

4 scoped-css unit tests in-macro (the §3.1 worked example rule-by-rule,
class parents, compound selectors, @media/@keyframes) + 5 integration tests
in wasm_ui: pure-form combine/transform, class fallback, reactive head ops in
the build batch with ids ≥16, the derive's full CSS contract (light vars,
explicit + auto dark `#10b981→#0d9467`, all §7.1 classes, built-ins, 'static
type), and the §8 injection sequence verbatim. Macros + wasm_ui + signals
suites all green; zero clippy `--all-targets`.

## Deviations (justified)

| Spec | Shipped | Why |
|------|---------|-----|
| `lightningcss` AST walk | dependency-free rule walker | classification needs rule boundaries + leading selector only; macro crate stays lean. Swap if real CSS outgrows it. |
| nested-struct ThemeTokens literal syntax | `#[token(...)]` attributes | the spec sketch is not valid Rust (field defaults/nested literals don't exist); same three dark-control levels. |
| auto-dark "invert" rules per token kind | uniform ~80%-luminance darken for colors | spec's own example is ≈0.8 darken; bg/text classification isn't expressible from a flat token. |
| CSS as bare child text | quoted string literal required | raw CSS (hex colors, `20px`) does not survive Rust tokenization. Compile error says so. |
