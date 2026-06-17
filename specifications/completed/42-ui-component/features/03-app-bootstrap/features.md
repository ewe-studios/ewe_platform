# Feature 03: App Bootstrap — one-line wiring with protocol presets

## 1. Motivation

Every program (and every test) repeats the same five-object ritual:

```rust
let signals = Rc::new(SignalsRuntime::new());
let ctx = Context::new(Rc::clone(&signals));
let runtime = Runtime::builder()
    .protocol(ColumnarV1::new())
    .memory(MemoryAllocations::new())
    .build();
runtime.attach(&signals);
let receiver = runtime.receiver();
```

Nothing in it varies except the protocol. Wrap it once; keep every piece
reachable from the wrapper.

## 2. Design: one `App` struct, protocol-named constructors

One struct with named constructors rather than distinct `ColumnarApp` /
`ArrowApp` / `JsonApp` types: the protocol is a VALUE choice behind the
`ProtocolMethods` seam, not a type-level property of the app — distinct
types would force every downstream signature to be generic (or duplicated)
over the app flavor for zero benefit. (If a use case ever needs the
protocol type statically, `App::with_protocol` keeps that door open.)

```rust
pub struct App {
    signals: Rc<SignalsRuntime>,
    ctx: Context,
    runtime: Runtime,
    receiver: SharedInstructionReceiver,
    theme: Option<GeneratedTheme>,   // §5 — owned theme, injected on install
}

impl App {
    /// THE DEFAULT — the arrow-family wire (protocol byte 1), VERSION 1:
    /// compact columnar, zero-copy on the JS side. We always default to
    /// arrow unless otherwise stated.
    pub fn new() -> App;                      // = arrow family, v1 columnar
    /// Explicit alias of the default (wire VERSION 1).
    pub fn columnar() -> App;
    /// Arrow family VERSION 2: real Apache Arrow IPC RecordBatches, via a
    /// NEW `ArrowIpcV2` ProtocolMethods impl wrapping the EXISTING
    /// `foundation_arrow::ArrowIpcEncoder` (already implements
    /// `ProtocolEncoder<Vec<DomOp>>`). Behind an `arrow` cargo feature
    /// (optional foundation_arrow dep); JS side reads it with the embedded
    /// apache-arrow.js (`APACHE_ARROW_JS`).
    pub fn arrow() -> App;
    /// Human-readable JSON DomOps (debugging).
    pub fn json() -> App;
    /// MockProtocol — tests; pairs with `sent_batches()` access.
    pub fn mock() -> (App, SentBatches);
    /// SERVER preset (review addition): on native, host_apply is a no-op
    /// stub — these capture every flushed batch as envelope-framed Vec<u8>
    /// in a shared queue (FrameSink<E>), ready for WS binary frames / SSE
    /// toward a client mount-stream. server() = arrow v1; server_with takes
    /// ANY Layer-1 encoder (JsonEncoder, foundation_arrow::ArrowIpcEncoder —
    /// no cargo feature needed server-side).
    pub fn server() -> (App, CollectedFrames);
    pub fn server_with<E: ProtocolEncoder<Vec<DomOp>> + 'static>(e: E) -> (App, CollectedFrames);
    /// Escape hatch: any ProtocolMethods impl.
    pub fn with_protocol(p: impl ProtocolMethods<Vec<DomOp>> + 'static) -> App;

    /// The pair every reactive html!/component call needs.
    pub fn context(&self) -> (Context, SharedInstructionReceiver);

    pub fn ctx(&self) -> &Context;
    pub fn receiver(&self) -> SharedInstructionReceiver;
    pub fn signals(&self) -> &Rc<SignalsRuntime>;
    pub fn stabilize(&self);                 // signals().stabilize()
    pub fn scope(&self) -> Context;          // ctx().child() — component scopes

    /// Install a theme (§5): App takes ownership and IMMEDIATELY queues the
    /// stylesheet's head-injection ops. Pairs with `theme!{}` or the runtime
    /// `Theme` builder — both yield a `GeneratedTheme`.
    pub fn theme(self, theme: GeneratedTheme) -> App;   // builder-style, #[must_use]
    pub fn get_theme(&self) -> Option<&GeneratedTheme>;
}
```

Usage:

```rust
let app = App::new();        // arrow family by default
let (ctx, receiver) = app.context();
let tree = html! { ctx, receiver, <main>…</main> };
app.stabilize();
```

Notes (settled):
- **Naming (review correction, 2026-06-13): the v1 wire is the COMPACT
  COLUMNAR — never "arrow v1".** Calling our owned format "arrow" was
  confusing (it triggered exactly the feature-flag confusion it was bound
  to). Taxonomy: protocol byte 1 = the COLUMNAR family; VERSION demuxes
  our compact columnar (v1) from Apache Arrow IPC (v2). `App::new()` =
  compact columnar; `arrow()` = Apache Arrow IPC. Nothing defaults to
  json/mock — those are opt-in debugging/testing presets.
- **The `arrow` feature is ON by default** (review decision: arrow-native
  support is a project goal; arrow-rs rides default-features=false and
  builds for wasm32 — verified). Size-sensitive wasm artifacts opt out
  with `default-features = false`. Measured (wire_bench, release): v1 vs
  v2 — 10 ops: 674ns/5.3µs encode, 344B/2378B; 1000 ops: 28µs/45µs,
  ~1.08x size; v1 dominates small UI deltas, v2 converges on bulk and
  buys ecosystem interop.
- `context()` returns CLONES (both are cheap Rc-backed handles) — the tuple
  the user asked for, directly usable by `html!` and component functions.
- "PlainHtmlApp" from the plan: there is no html-string protocol on the
  DomOp loop — the html-as-text story is `Html::to_markup()` (feature 00)
  on the server side, plus the existing `MorphNode` op for html patches. So
  the preset set is new/columnar/arrow_ipc/json/mock (+`with_protocol`); a
  markup "app" wrapper is not needed — servers call `to_markup` on plain
  `Html` values with no runtime at all.
- `App` owns all five pieces → drop order handled in one place; dropping the
  `App` disposes the root context (tears down effects) with the runtime
  still alive, then the rest in field order.
- Memory defaults to `MemoryAllocations::new()`; an `App::builder()` is NOT
  added until a real need appears (don't speculate API).

## 3. Server rendering (review addition)

First-paint HTML needs NO App (`html!{…}.to_markup()` — feature 00). Live
server-driven UI = `App::server()` + ship the frames; one App per
connection/session; drop on disconnect. README §10 is the user-facing
walkthrough. `Context` gains handle-counted `Clone` (clone = another handle
to the SAME scope; disposal at the last handle — `Rc::strong_count` can't
decide it because parents hold child inners) so `app.context()` can hand
out an owned pair.

## 4. Testing

Construction per preset; `context()` pair drives a reactive template
end-to-end (mock preset asserts the op stream); `stabilize` flushes;
dropping `App` disposes effects (signal set after drop runs nothing);
`scope()` child teardown independent of root.

Theme (§5): `theme!{}` → `GeneratedTheme`; `App::theme(t)` queues the
head-injection sequence (assert the `SetText` op carries the generated CSS);
`get_theme()` reads tokens/CSS back; the runtime `Theme` builder and the
`theme!{}` macro produce identical CSS for the same tokens.

---

## 5. Theme ownership: `App.theme(theme!{…})` (spec-39 decision 021)

The `App` is where a theme lives, because the theme's CSS must reach `<head>`
as the FIRST DOM batch — before any content renders (feature 09 §8) — and the
`App` is the one object that owns the receiver from construction.

```rust
let app = App::new().theme(theme! {
    colors  { primary: { light: "#3b82f6", dark: "#60a5fa" }, bg: "#ffffff" }
    spacing { sm: 8px, md: 16px, lg: 24px }
    radius  { sm: 4px }
    shadow  { soft: "0 1px 2px rgba(0,0,0,.1)" }
});
let theme = app.get_theme().unwrap();      // tokens + generated CSS, inspectable
```

`theme(self) -> App` is builder-style and `#[must_use]`; it queues
`inject_theme_css(receiver, theme.to_css())` immediately, then stores the
`GeneratedTheme`. `get_theme()` returns `Option<&GeneratedTheme>`.

### 5.1 Why a macro at all — the constraint that drove 020 → 021

Decision 020 sketched a theme as a struct with nested literal VALUES
(`colors: Colors { primary: "#3b82f6" }`). That is **not valid Rust** as the
input to a `#[derive]`: a derive receives a TYPE DEFINITION (`field: Type`),
and values cannot appear in a type position. The shipped workaround put values
in `#[token(...)]` attributes on unit fields — correct, but it forces users to
learn attribute/derive mechanics for something they just want to *declare*.

The root fact, stated once: **a macro only ever sees tokens, never a value.**
A `#[derive]` sees the definition; a function-like macro sees whatever literal
token stream sits between its braces. So values can be read at COMPILE time
only if written **inline** in a function-like macro (`theme!{…}`) — not via a
separately-defined const it merely references. Reading them at RUNTIME is the
other option: instantiate a normal struct/builder and hand the VALUE to a
function. The two front-ends below are exactly those two readers.

### 5.2 `theme!{}` — the headline API (compile time)

A single function-like macro, struct-like to read, expanding to a
`const`-capable `GeneratedTheme` whose CSS is generated AT COMPILE TIME and
embedded as a `&'static str`.

- **Blocks are a fixed, enforced set** — `colors`, `spacing`, `padding`,
  `margin`, `radius`, `shadow`, `font_size`, `animation`. An unknown block
  name is a COMPILE ERROR. Each block maps to a token *category*.
- **Value grammar** per `name: <value>` entry:
  - a string literal → verbatim CSS (`primary: "#3b82f6"`, or any multi-part
    value like a shadow — `#` and spaces aren't single Rust tokens, so colors
    and compound values stay quoted);
  - a bare suffixed literal → stringified with NO quotes (`md: 16px`, `fast:
    250ms`, `base: 1.5rem`, `weight: 2`);
  - the brace form `name: { light: <v>, dark: <v> }` → explicit dark. A plain
    value auto-derives the dark color (§5.5).
- **Output**: `GeneratedTheme` — holds the parsed `ThemeToken` table AND the
  CSS string; `to_css()`, `tokens()`, `value(category, name)` for inspection.
  Const-capable: the macro emits a `const` token slice (an inline `&[…]`
  won't rvalue-promote because `ThemeToken` carries a `Cow`, whose `Owned`
  variant needs `Drop`).

### 5.3 Runtime `Theme` builder — no macro

For themes assembled from config/values at runtime (the macro can't read
runtime values):

```rust
let theme = Theme::new()
    .color("primary", "#3b82f6", Some("#60a5fa"))
    .color("secondary", "#10b981", None)   // None → dark auto-derived
    .spacing("md", "16px")
    .radius("sm", "4px")
    .build();                               // -> GeneratedTheme (Cow::Owned CSS)
```

Same output type, so `App::theme` and everything downstream are uniform.

### 5.4 `#[derive(ThemeTokens)]` — legacy, retained

The decision-020 derive still works (values in `#[token(...)]` attributes) and
now routes through the SAME generator. No longer the headline API, but kept for
back-compat.

### 5.5 The shared generator: `foundation_theme`

All three front-ends call ONE function — `foundation_theme::theme_css(&[
ThemeToken]) -> String` — so output is identical regardless of how tokens were
declared. New `no_std + alloc` crate, depended on by `foundation_macros` (the
macro/derive call it at expansion time and embed the result as a literal) and
re-exported by `foundation_wasm_ui`.

| front-end | reads values | when | CSS type |
|-----------|--------------|------|----------|
| `theme!{}` macro | inline literals | compile | `&'static str` |
| `#[derive(ThemeTokens)]` | `#[token]` attrs | compile | `&'static str` |
| `Theme` builder | struct fields | runtime | `String` |

`GeneratedTheme { css: Cow<'static,str>, tokens: Cow<'static,[ThemeToken]> }`
unifies both lifetimes — `from_static` (`const fn`) for the macro/derive,
`from_tokens` for the builder.

`theme_css` emits, in order:
1. `:root { --{category}-{name}: {light}; … }` custom properties;
2. a `@media (prefers-color-scheme: dark)` block — colors only, explicit dark
   verbatim, missing dark colors auto-derived at ~80% luminance (integer
   round-half-up; `#10b981 → #0d9467`; non-hex values are left alone);
3. per-category utility classes — `color`→`.bg-/.text-/.border-`,
   `spacing`→`.p-/.m-`, `padding`→`.p-`, `margin`→`.m-`, `radius`→`.rounded-`,
   `shadow`→`.shadow-`, `font_size`→`.text-`, `animation`→`.duration-`;
4. the built-in token-independent utilities (layout/display/sizing/borders)
   plus a GENERATED `.opacity-0 … .opacity-100` scale in steps of 5
   (`.opacity-5 { opacity: 0.05; }` … `.opacity-100 { opacity: 1; }`).

### 5.6 Injection

`inject_theme_css(receiver, css: &str)` queues the five-op head sequence
(create `<style id="primal-theme">`, register, set text = CSS, append to the
reserved `<head>` ambient node). Widened from `&'static str` to `&str` so the
runtime builder's owned `String` works alongside the macro/derive's static
CSS. `App::theme` calls it on install; flushing it IS the first batch.

Cross-references: decision [021](../../../39-foundation-wasm-ui/decisions/021-theme-macro-and-runtime.md)
(full rationale), decision 020 (original system), feature 09
(scoped-styles-theme: utility tables, first-batch injection).
