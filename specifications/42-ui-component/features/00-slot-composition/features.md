# Feature 00: Slot Composition — `Render`, `Slot`, `<Fragment>`, span text slots

**Decision record + implementation spec.** Designed in conversation
2026-06-12/13; every decision below was argued and settled — deviations need
a new decision, not a silent change.

## 1. Motivation: reactive composition is broken today (verified)

The pure form composes perfectly (slots accept `Html` / `Option<Html>` /
`Vec<Html>` / text via `IntoHtml`). The REACTIVE form cannot compose
fragments at all — three failure modes confirmed against the real macro
(scratch test, 2026-06-12):

1. **Owned `Html` slot value → compile error (E0507).** The slot effect is
   an `FnMut` re-run on signal changes; `into_html(self)` consumes the value
   on the first run.
2. **`&fragment` → silently dropped.** Compiles; wire shows the slot's
   dedicated text node receiving `SetText("")` — the effect keeps only
   `.text` of the converted value, and an element fragment has none. No
   `CreateElement` for the fragment is ever queued.
3. **Reactive child as slot → orphaned twice.** The child mounts itself
   DETACHED (created + registered, never appended), and the parent's slot
   again renders `SetText("")`. The child lives in the registry, never in
   the DOM.

Additionally there is **no `Html` → markup-string serializer**, so a server
cannot emit first-paint HTML from the same `Html` values that drive the
DomOp channel — which the morph contract (§5) requires.

## 2. The `Render` trait (decision: `&self`, no `Box<Self>`, no consuming)

```rust
/// "Given a context and receiver, I present my html; what happens to me
/// afterwards is my owner's business."
pub trait Render {
    fn render(&self, ctx: &Context, rcv: &SharedInstructionReceiver) -> Html;
}
```

Lives in `foundation_wasm_ui` (needs `Context` + `SharedInstructionReceiver`;
`foundation_ui_traits` sits below both).

**Why `&self`** (argued against `self` and `self: Box<Self>`):
- Object-safe with zero ceremony → `Box<dyn Render>` / `Rc<dyn Render>` work
  directly; shareable.
- Re-invokable: each `render()` call mounts a FRESH instance (own id block,
  own effects) — a slot is a reusable template, same semantics as calling a
  component function twice. Mount-once is therefore a documented convention,
  not compiler-enforced; this is the accepted trade.
- Implementations hand out clones instead of moves (`Html`'s impl is
  `self.clone()`); UI-sized data mounted once — negligible, accepted.

**The three-lifetime model** (the contract that makes recipes safely
droppable/shareable):
1. **Recipe** (the `Render` value): holds cheap handles (signal getters,
   config). Effects created during `render` capture their own clones — the
   mounted output never references the recipe. Drop/keep/share it freely.
2. **Mounted instance**: born at `render(ctx, rcv)`, owned by THAT `ctx`
   (effects + id block); dies when the placing component's scope is
   disposed. The component supplies ITS OWN ctx at the call site, so slot
   content's lifecycle follows PLACEMENT (this is why `render` takes ctx as
   a parameter instead of capturing the caller's — argued and settled).
3. **Data** (signals): owned by the creating scope (usually the caller's).
   Constraint to document: slot content must not outlive the signals it
   reads — in practice, don't dispose a parent scope while a child holding
   its slots is mounted (parent-outlives-child is the natural nesting).

## 3. `Slot` and the typed slot-struct convention

```rust
pub struct Slot(Box<dyn Render>);

impl Slot {
    /// Closure entry — closures do NOT implement Render (coherence: a
    /// blanket Fn impl + `impl Render for Html` overlap, E0119; argued).
    pub fn lazy(f: impl Fn(&Context, &SharedInstructionReceiver) -> Html + 'static) -> Self;
    pub fn render(&self, ctx: &Context, rcv: &SharedInstructionReceiver) -> Html;
}
impl From<Html> for Slot { … }          // statics lift free (via Render for Html)
impl Render for Slot { … }              // delegation, so Slot is itself a Render
```

Per-component slots are a TYPED STRUCT — never stringly lookups
(`named("menu")` was argued and rejected: runtime panic where the compiler
can enforce; no IDE support; no refactor safety):

```rust
pub struct ShoeShelfSlots {
    pub menu: Slot,            // required = field (can't construct without it)
    pub badge: Option<Slot>,   // optional  = Option (component skips on None)
    pub children: Vec<Slot>,   // ordered, unkeyed children
}
```

- **Nesting**: a slot group nests another component's slot group as an
  explicit field (`pub drawer: DrawerSlots`). Dynamic slot-path traversal
  (`slot.slot("drawer")`) is REJECTED — it makes component internals public
  API, fails at runtime, and breaks refactoring. A component that wants an
  inner piece customizable PROMOTES it to its own slot deliberately.
- **Lower components needing their own slots** require no mechanism: the
  parent constructs the child's slot struct at the call site (plain function
  composition).

Three entry points, no trait gymnastics:
`menu: fragment.into()` (static `Html`) · `menu: Slot::lazy(|c, r| nav(c, r))`
(function component) · `menu: drawer_config.into_slot()` (struct component —
provided method on `Render: Sized`).

## 4. Text slots render as `<span primal-id=…>` (changed from bare text node)

**Why (morph addressability, argued from F07 mechanics):** MorphDom matches
and preserves nodes by ELEMENT identity; text nodes cannot carry
`primal-id` (no attributes), so a server morph silently replaces the slot's
registered text node → the registry points at a detached node → every later
`SetText` writes into the void. An id-bearing `<span>` is preserved by the
morph's id-scans, so slot wiring SURVIVES server patches.

- `<span>`, not `<p>`: `<p>` is block-level AND auto-closed by the HTML
  parser (`<p>` in `<p>` restructures the tree). `<span>` is the neutral
  inline container.
- Accepted cost, now a documented contract: a text slot IS an element —
  CSS (`.parent > *`, `:first-child`, flex/grid gap) sees it; slots become
  deliberately styleable/inspectable.
- Reactive form: `CreateElement(span)` + `RegisterNode` + `SetAttribute
  (primal-id)` + `AppendChild`; the slot effect's `SetText` targets the span
  (textContent). Pure form: same span shape for text-shaped slot values
  (PARITY, §5); element-fragment values keep inlining as today.

## 5. One rendering contract, any target (parity is by construction)

Verified: the macro expansion has no target-specific code, and the entire
reactive test suite runs natively against `MockProtocol`. Pure-vs-reactive
is "value only" vs "value + mounted live instance"; the protocol seam
decides where flushed DomOps go (wasm arena → JS, server → SSE/WS toward
`mount-stream`, mock → assertions). Since we own EVERY producer of this DOM
shape, both forms emit the same shape — that is what makes MorphDom diffs
clean (server markup vs live DOM come from the same renderer).

**Therefore feature 00 includes `Html::to_markup()`** (in
`foundation_ui_traits`, no_std + alloc): serialize an `Html` tree to an HTML
string — text escaping (`&<>"'`), void elements, attribute quoting, and the
slot-span shape EXACTLY matching the reactive build ops. This is the missing
leg of server-side rendering: first-paint HTML and the DomOp channel from
the same values.

## 6. `<Fragment>` and the capitalized-built-in namespace

```rust
html! { ctx, receiver,
    <section>
        <h1>{title.get()}</h1>                                    // text slot (span), reactive
        <Fragment>{slots.menu.render(ctx, receiver)}</Fragment>   // mount once, splice
        <ul><Fragment>{items}</Fragment></ul>                     // Vec<Html> appended in order
    </section>
}
```

- `{expr}` KEEPS today's semantics (reactive text slot — now span-shaped).
  Two different machine codes exist (re-running text effect vs once-at-mount
  splice) and the macro must choose from syntax alone; overloading `{expr}`
  was rejected (type-based guessing is fragile magic).
- `<Fragment>` is a COMPILE-TIME DIRECTIVE, no DOM node of its own: each
  `{expr}` child evaluates ONCE during the build-ops phase and splices into
  the PARENT element. Expression type: anything `IntoHtml`. The user calls
  `.render(ctx, receiver)` explicitly (argued: no hidden ctx-threading; this
  also lets `<Fragment>` splice any fragment expression, not just slots).
- Mechanics per fragment value:
  - already-mounted reactive fragment (`runtime_id` is `Some`) → single
    `AppendChild(parent, root)`;
  - pure fragment → `mount_fragment` walks it, allocates an id block, queues
    create/register/append ops.
- Reactivity INSIDE spliced content keeps working through its own effects;
  the splice itself NEVER re-runs. Signal-driven placement (conditional,
  keyed lists) is OUT OF SCOPE → feature 05 (`<Show>`/`<For>`), which will
  reuse `mount_fragment` + scope disposal as building blocks. Until then,
  visibility toggling via reactive class/style covers dialog/accordion/
  popover patterns.
- **Capitalized tags are reserved built-ins** (`Fragment` now; `Show`/`For`
  later). Real HTML tags are lowercase and custom elements require a dash,
  so the namespace is free. Unknown capitalized tag → compile error with a
  "custom elements need a dash" hint.
- Pure form: `<Fragment>{expr}</Fragment>` ≡ today's `{expr}` inlining.

## 7. Supporting changes

1. `Html.runtime_id: Option<u32>` (`foundation_ui_traits`, additive,
   default `None`): the reactive `html!` stamps its root's block id; how a
   spliced child is recognized as already-mounted. (Parsing the `primal-id`
   attribute string was rejected as a hack.)
2. `mount_fragment(ctx, rcv, html, parent_id) -> u32` (`foundation_wasm_ui`):
   the one helper behind `<Fragment>` — `AppendChild` for mounted fragments,
   build-ops walk (fresh id block via `ctx.allocate_id_block`) for pure
   ones; returns the root id. Also finally gives apps a non-magic way to
   attach a top-level tree to `body` (node 1).
3. Macro: `Fragment`/capitalized-tag handling in the parser; span-shaped
   text slots in BOTH forms' codegen; `runtime_id` stamping on the reactive
   root.

## 8. Testing (tests in `{crate}/tests/`, uat profile)

- The three §1 failure modes become the acceptance tests: owned-`Html`-in-
  `<Fragment>` compiles and mounts; pure fragment splices (CreateElement ops
  present); reactive child splices via `AppendChild` to the parent (no
  orphans), interior effects still firing after splice.
- Slot struct end-to-end: required/optional/children; a component rendering
  `Vec<Slot>` in order; nested slot groups; recipe dropped after render →
  mounted instance unaffected; same recipe rendered twice → two disjoint id
  blocks, independent updates.
- Span text slots: wire shape (span + SetText) in reactive form; identical
  shape from pure form + `to_markup`; morph survival (JS suite: morph a
  subtree containing a slot span, assert the span node is preserved and a
  subsequent SetText still lands).
- `to_markup`: escaping, void elements, attribute order/quoting, slot-span
  parity against the reactive op stream.
- JS suite additions where DOM behavior is asserted (morph survival,
  fragment append into live tree).
