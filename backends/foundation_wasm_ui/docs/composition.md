# Composition — components, slots, mounting

There is **no `Component` trait**. A component is a function returning `Html`;
composition is functions plus the `Render`/`Slot` pair and the `mount_*` helpers.

## Components are functions

```rust
use foundation_signals::Context;
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, SharedInstructionReceiver};

fn counter(ctx: &Context, rcv: &SharedInstructionReceiver) -> Html {
    let (count, set_count) = ctx.signal(0i64);
    let inc = ctx.callback(move |_| set_count.set(count.get() + 1));
    html! { ctx, rcv, <button primal:onclick={inc}>"Count: " {count.get()}</button> }
}

let _ui = html! { ctx, receiver, <div>{counter(&ctx, &receiver)}</div> };
```

Pure components that need no reactivity take no `ctx`/receiver.

## Accepting content: `Slot`

A component accepts content via typed slot fields — `Slot` (required),
`Option<Slot>` (optional), `Vec<Slot>` (repeated):

```rust
use foundation_wasm_ui::{html, Render, Slot};

struct Card { header: Slot, footer: Option<Slot>, items: Vec<Slot> }

fn card(ctx: &Context, rcv: &SharedInstructionReceiver, slots: Card) -> Html {
    let header = slots.header.render(ctx, rcv);
    let items: Vec<Html> = slots.items.iter().map(|s| s.render(ctx, rcv)).collect();
    let footer = slots.footer.map(|f| f.render(ctx, rcv));
    html! { ctx, rcv,
        <section class="card">
            <header>{header}</header>
            <ul>{items}</ul>
            {footer}
        </section>
    }
}
```

Build slots from an `Html` value or lazily from a closure:

```rust
let _from_html: Slot = html! { <b>"hi"</b> }.into_slot();
let _lazy: Slot = Slot::lazy(|ctx, rcv| html! { ctx, rcv, <span>"deferred"</span> });
```

When a slot renders, the effects it creates register under the **placing**
component's `ctx` — content lifecycle follows placement, not the recipe.

> In templates, mount element content with `<Fragment>{slot.render(ctx, rcv)}</Fragment>`,
> not a bare `{…}` (which is text-only in the reactive form).

## The `Render` trait

```rust
pub trait Render {
    fn render(&self, ctx: &Context, rcv: &SharedInstructionReceiver) -> Html;
    fn into_slot(self) -> Slot where Self: Sized + 'static;
}
```

`render` takes `&self` (object-safe, re-invokable — each call mounts a FRESH
instance). `Html` implements `Render` directly. `Slot` erases any `Render` through
`Box<dyn Render>`; closures enter via `Slot::lazy` (a blanket `Fn` impl would
overlap `impl Render for Html`).

## Mounting helpers

`<Fragment>` is the template form; the runtime behind it is `mount_*`:

```rust
use foundation_wasm_ui::{html, mount_fragment, mount_into};
let parent_id: u32 = 16;
let _root = mount_fragment(ctx, rcv, html! { <p>"static"</p> }, parent_id);
let _root = mount_into(ctx, rcv, "plain text", parent_id);   // anything IntoHtml
```

- An already-mounted reactive fragment (it has a `runtime_id`) splices with a
  single `AppendChild` — never rebuilt.
- A pure fragment is built here from a fresh `allocate_id_block` (same op sequence
  the macro emits; its compile-time `primal-id`s become runtime ids).
- A tagless grouping node (`Vec<Html>`/`<Fragment>`) contributes its children.
- `mount_before` inserts before a reference sibling and returns **every**
  top-level spliced id (`<Show>`/`<For>` use this to unmount multi-root content).

`App::mount(root)` is the same machinery against the reserved `<body>` node — the
body counterpart of `App::theme` (which injects into `<head>`). It appends at the
bottom of `<body>`, so a late `<script>` mounted after content runs after the body
is in place.

## Gotchas

- `Slot::lazy` takes `Fn`, not `FnOnce` — slots are re-invokable; a closure that
  consumes its captures won't compile. Clone from captures into each output.
- An already-mounted fragment must not be spliced twice (`mount_fragment` takes the
  `Html` by value to consume its identity).
- A pure fragment mounts inert nodes (no effects); reactivity needs the reactive
  `html!` form.
- Rendering a `Slot` twice yields two independent instances — not idempotent.

See also: **[html! macro](./html-macro.md)** ·
**[`foundation_ui_components`](../../foundation_ui_components/README.md)** for
ready-made headless components built this way.
