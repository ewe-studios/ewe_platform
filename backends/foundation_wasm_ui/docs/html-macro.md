# The `html!` macro

`html!` compiles HTML-shaped markup to a typed `Html` value and (in reactive
mode) a batch of `DomOp` builds. There is **zero runtime HTML parsing** — the
parser walks Rust tokens at compile time.

## Two forms

**Pure form** — `html! { <markup> }` → an `Html` value. No context, no receiver,
no reactivity. For server rendering, static fragments, and composition:

```rust
use foundation_wasm_ui::html;

let h = html! { <div class="card"><h2>"Title"</h2></div> };
let markup: String = h.to_markup();   // serialize for first-paint HTML
```

**Reactive form** — `html! { ctx, receiver, <markup> }`. The first two arguments
are a `Context` and a `SharedInstructionReceiver` (split at the first two
top-level commas). This form *mounts*: allocates a contiguous block of wire ids,
queues build ops on the receiver, and creates **one effect per dynamic slot**.

```rust
let (count, set_count) = ctx.signal(7i64);
let tree = html! { ctx, receiver, <div><span>{count.get()}</span></div> };
app.stabilize();          // CreateElement(div), CreateElement(span), SetText("7")
set_count.set(8);
app.stabilize();          // exactly one SetText("8") — nothing else re-runs
```

Both forms return the same `Html` (with `parts` and a `runtime_id`), so you can
inspect or splice what was mounted.

## Child slots (`{expr}`)

In the **pure** form, `{expr}` evaluates **once** through `IntoHtml`:

| slot expression type        | becomes                                        |
|-----------------------------|------------------------------------------------|
| `Html` / `&Html`            | inlined as a child element                     |
| `Option<Html>`              | the element, or nothing (`None`)               |
| `Vec<Html>`                 | a tagless wrapper whose children are the items |
| `&str` / `String`           | a text node                                    |
| integers / floats / `bool`  | a text node via `Display`                      |

In the **reactive** form, a bare `{expr}` child slot is **TEXT-ONLY** — it
becomes a `<span>` + an effect emitting `SetText`. To mount *element* content
(an `Html`, a built child, a machinery `<script>`) wrap it in `<Fragment>`:

```rust
html! { ctx, rcv,
    <button>
        <Fragment>{children}</Fragment>   // ✅ mounts the element(s)
        // {children}                     // ❌ would text-ify to empty
    </button>
}
```

Rule: **text → `{…}`, element content → `<Fragment>{…}</Fragment>`.**

## Attribute kinds (precise)

| syntax | kind | behaviour |
|--------|------|-----------|
| `name="lit"` | **static** | string literal, set once; `class` rides the `CreateElement` op |
| `name` (bare) | **static boolean** | value `"true"` |
| `name={expr}` | **reactive** | wrapped in an effect; re-runs on signal change. Read signals with `.get()`. `Some` sets, `None` removes (presence contract) |
| `name=[expr]` | **static-once** | evaluated once at build, **no effect, no clone** — for owned config values |
| `primal:onX={handler}` | **event** | one-time `AddEventListener` (see [reactivity & events](./reactivity-and-events.md)) |

```rust
let class: std::borrow::Cow<'static, str> = compute_class();
html! { ctx, receiver,
    <input
        class="field"                          // static
        value={text.get()}                     // reactive — effect, re-runs
        data-mode=[class]                       // static-once — moved, no clone
        data-checked={on.get().then_some("")}  // presence: Some("") sets, None removes
        primal:onchange={set_on}               // event
    />
}
```

> The `primal:` prefix is reserved for machinery (`primal:on*`, `primal:style`,
> `primal:script`, `primal:setter`). For machinery *inputs* use `data-*`.

## How ids work

Each macro invocation gets a **disjoint id block** (`ctx.allocate_id_block(total)`),
so instantiating the same component in a loop can never collide. Text slots become
a dedicated `<span primal-id>` in *both* forms (the morph contract), so `SetText`
targets only that span and server markup lines up with the live DOM.

Each dynamic slot lives in exactly one closure — a handle used in several slots
needs a per-slot `.clone()` (plain Rust move rules).

## What it is NOT for

- **No template directives.** `@if`/`@for` don't exist — use plain Rust (pure
  form) or `<Show>`/`<For>` (reactive form).
- Pure-form `{slot}` values are evaluated **once** — values, not bindings.
- `<Show>`/`<For>` are **reactive-only** (compile error in a pure tree).
- Exactly **one root element** per `html!` (wrap multiples in a parent or
  `<Fragment>`).
- Capitalised tags are reserved built-ins (`Fragment`, `Show`, `For`); a custom
  element must be lowercase-with-a-dash (`<my-widget>`).
- The macro never resolves types — whether a handler is a setter or a closure is
  `MaybeCallback`'s job at compile time.

See also: **[reactivity & events](./reactivity-and-events.md)** ·
**[composition](./composition.md)**.
