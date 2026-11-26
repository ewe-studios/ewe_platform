# foundation_ui_traits

This crate defines the markup tree, names and reactive parts for the Ewe user interface (UI).
It uses Rust allocation types and works without the standard library.
The template macro creates these values. The renderer lowers them to generated wire operations.
The wire package guide explains the binary format, action messages, signed fixtures and hand-written frames. [Wire package guide](../foundation_ui_wire/README.md)

## `Html` + `IntoHtml`

`Html` is a lightweight, dependency-free DOM tree the `html!` macro produces.

```rust
use foundation_ui_traits::{Html, IntoHtml};

let el = Html::text("hello");          // a text node
let el = Html::new();                  // an empty tagless / grouping node
let _ = (el.is_text(), el.is_element());
```

```rust
pub struct Html {
    pub tag: Option<HtmlTag>,                          // None = text or grouping node
    pub attributes: Vec<(AttrName, Cow<'static, str>)>,
    pub children: Vec<Html>,
    pub text: Option<Cow<'static, str>>,
    pub parts: Vec<Part>,                              // dynamic-slot descriptors (below)
    pub runtime_id: Option<u32>,                       // Some once mounted reactively
}
```

`IntoHtml` is what `{expr}` child slots in `html!` lower to — implemented for all
the common types (decision 007) so conversion is automatic:

```rust
pub trait IntoHtml {
    fn into_html(self) -> Html;
}
```

| type | becomes |
|------|---------|
| `Html` / `&Html` | itself (clone for `&Html`) |
| `Vec<Html>` | a tagless node whose children are the items |
| `Option<Html>` | the element, or an empty node (`None`) |
| `&str` / `String` | a text node |
| `usize`/`isize`/`u8`…`u64`/`i8`…`i64`/`bool`/`f32`/`f64` | a text node via `Display` |

`foundation_signals` adds impls for `SignalGetter<T>` / `ComputedGetter<T>` (they
read `.get()`), which is why a signal can appear directly in a slot.

---

## `IntoAttrValue` — the `{}`/`[]` attribute trait

This is the trait the `html!` macro's `attr={expr}` / `attr=[expr]` attributes
lower to. It maps an attribute expression to `Option<Cow<'static, str>>`:

```rust
pub trait IntoAttrValue {
    /// `None` removes/omits the attribute; `Some` sets it to the value.
    fn into_attr_value(self) -> Option<Cow<'static, str>>;
}
```

The contract:

- **`Some(v)` ⇒ set the attribute** to `v` (`SetAttribute`);
- **`None` ⇒ remove it** (`RemoveAttribute`), or omit it in the pure/SSR form.

This is the **presence data-attribute contract** (decision feature 05 §1/§8.2):
a `data-checked` that is *present* when on and *absent* when off — so
`[data-checked]` in CSS matches only when truly on. A value that always
stringifies to `"false"` cannot express absence.

```rust
use std::borrow::Cow;
use foundation_ui_traits::IntoAttrValue;

assert_eq!("card".into_attr_value(), Some(Cow::from("card")));        // &str  → always Some
assert_eq!(true.into_attr_value(),   Some(Cow::from("true")));        // bool  → always "true"/"false"
assert_eq!(Option::<&str>::None.into_attr_value(), None);             // None  → remove/omit
assert_eq!(true.then_some("").into_attr_value(), Some(Cow::from(""))); // presence: Some("") when on
```

**Leaf impls** (`&str`, `String`, `Cow<'static, str>`, every integer/float,
`bool`) always return `Some(..)`, so plain `attr={value}` usage is unchanged. One
**blanket impl over `Option<T: IntoAttrValue>`** is what opts into presence
semantics — and there is no overlap, because bare `&str` and `Option<&str>` are
distinct types. Note that bare `bool` is **not** presence (it stringifies to
`"true"`/`"false"`); use `flag.then_some("")` for true presence.

---

## `HtmlTag` / `AttrName` — known-id wire tables

The overwhelmingly common tag and attribute names are assigned `u16` ids so the
wire carries ~3 bytes instead of a string (decisions G12/G13/G14). Each is an
enum: a known id, or an owned name for everything else.

```rust
pub enum HtmlTag  { Id(u16), Name(Cow<'static, str>) }
pub enum AttrName { Id(u16), Name(Cow<'static, str>) }
```

```rust
use foundation_ui_traits::{HtmlTag, AttrName, TAG_DIV, ATTR_CLASS};

let div = HtmlTag::from_static("div");      // -> HtmlTag::Id(TAG_DIV)
assert_eq!(div, HtmlTag::Id(TAG_DIV));
assert_eq!(div.name(), Some("div"));

let custom = HtmlTag::from_name("my-widget"); // unknown -> owned Name
let cls = AttrName::from_static("class");    // -> AttrName::Id(ATTR_CLASS)

// Convert a compact name to its string representation:
assert_eq!(cls.to_wire_string(), "id:1");    // known ids as "id:<n>"
let back = AttrName::from_wire_str("id:1");   // parses back to Id
```

Each type provides `from_name` / `from_static` (prefer the known-id form),
`name() -> Option<&str>`, `to_wire_string()` / `from_wire_str()`, `Display`, and
`From<&'static str>` / `From<String>`.

Known names live in two static tables, `TAG_NAMES` and `ATTR_NAMES`, where
**index + 1 == id** (id 0 is reserved, never assigned). The first four of each
are pinned by spec: `TAG_DIV`/`TAG_SPAN`/`TAG_INPUT`/`TAG_BUTTON` (1–4) and
`ATTR_CLASS`/`ATTR_ID`/`ATTR_STYLE`/`ATTR_VALUE` (1–4).

> **ABI rule:** the JS runtime mirrors these tables. **Order is ABI — append
> only, never reorder.** Inserting or moving an entry silently corrupts every
> already-shipped client.

---

## `Part` descriptors

A `Part` tells the signal runtime *where* a dynamic expression lives in the
rendered tree, so each one can become an effect that re-renders just that slot
(decision 005). They live in `Html::parts`.

```rust
pub enum Part {
    Text(TextPart),        // {signal} text content inside an element
    Attribute(AttrPart),   // class={expr}, id={expr}, …
    Event(EventPart),      // primal:onclick={handler}, …
}

pub struct TextPart  { pub node_id: u32 }
pub struct AttrPart  { pub node_id: u32, pub attr_name: String }
pub struct EventPart { pub node_id: u32, pub event_name: String }
```

`node_id` is the `primal-id` the macro assigned. There is no `ChildPart` — every
child position generates a `Part::Text`, and `Vec<Html>` expressions are handled
at runtime through `IntoHtml`.

---

## The `__macro` re-export module

```rust
#[doc(hidden)]
pub mod __macro {
    pub use alloc::borrow::Cow;
    pub use alloc::string::{String, ToString};
    pub use alloc::vec;
    pub use alloc::vec::Vec;
}
```

The `html!` / `theme!` macros (in `foundation_macros`) generate code that names
alloc types like `Vec`, `String`, and `Cow`. But the **calling** crate might be
`std` *or* `no_std` — a generated `::std::vec::Vec` path would not resolve in a
`no_std` crate. Naming those types through
`::foundation_ui_traits::__macro::Vec` instead makes the generated code neutral:
it works the same whether the consumer is `std` or `no_std`. It is hidden because
it exists only for codegen, not for hand-written use.

---

## Verify the public types

```sh
cargo test -p foundation_ui_traits --profile uat
cargo run -p foundation_wasm_ui --profile uat --example protocol_migration
```

The first command checks markup, names and the removal of the old operation type.
The second shows a host that writes and reads a frame through the current protocol.
