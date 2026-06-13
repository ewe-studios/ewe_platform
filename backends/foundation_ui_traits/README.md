# foundation_ui_traits

The shared vocabulary of the Ewe UI stack: the `Html` tree, the conversion
traits the `html!` macro lowers into, the compact wire identities for known tag
and attribute names, the 19-variant `DomOp` enum, the reactive `Part`
descriptors, and the protocol encoders that turn `DomOp` batches into bytes.

**`no_std + alloc`, zero dependencies, compiles for any Rust target.** It is the
middle layer that lets `foundation_signals` implement `IntoHtml` and
`foundation_wasm_ui` speak `DomOp` without either depending on the other
(decision 012). Pure types and pure codecs — nothing else.

> Spec: `specifications/39-foundation-wasm-ui/features/01-shared-traits/`
> (decisions 007/010/012/014/022).

## Contents
- [Why this crate](#why-this-crate)
- [`Html` + `IntoHtml`](#html--intohtml)
- [`IntoAttrValue` — the `{}`/`[]` attribute trait](#intoattrvalue--the---attribute-trait)
- [`HtmlTag` / `AttrName` — known-id wire tables](#htmltag--attrname--known-id-wire-tables)
- [`DomOp` — the wire op set](#domop--the-wire-op-set)
- [`Part` descriptors](#part-descriptors)
- [Encoders & the envelope](#encoders--the-envelope)
- [The `__macro` re-export module](#the-__macro-re-export-module)
- [What this crate is NOT for](#what-this-crate-is-not-for)

---

## Why this crate

Three layers of the stack need to agree on the same types, but none may depend
on the others:

- `foundation_signals` needs `IntoHtml` to implement on its getters;
- the `html!` macro (in `foundation_macros`) needs `Html`, `Part`,
  `IntoAttrValue`, and the wire-id tables to generate code against;
- `foundation_wasm_ui` and every server transport need the `DomOp` vocabulary
  and the encoders.

This crate is that single shared middle. It is `no_std`, zero-dependency, and
compiles everywhere — so the same `DomOp` and the same wire format are used by
the WASM runtime, an HTTP/SSE/WS server, a CLI, and the test suite alike.

---

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

// The decision-010 string-column form used by the columnar/JSON wires:
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

## `DomOp` — the wire op set

`DomOp` is the one canonical vocabulary of DOM mutations — 19 variants, ops
0–18 — that every transport (columnar, Arrow, JSON, the byte-0 batch stream) and
every consumer (WASM runtime, servers, tests) shares. The variants map 1:1 to the
decision-010 `u8` operation codes the JS applicator switches on.

```rust
use std::borrow::Cow;
use foundation_ui_traits::{DomOp, HtmlTag, AttrName};

let create = DomOp::CreateElement {
    node_id: 16,
    tag: HtmlTag::from_static("div"),
    class: Cow::from("card"),
};
let set = DomOp::SetAttribute {
    node_id: 16,
    name: AttrName::from_static("title"),
    value: Cow::from("hi"),
};
let append = DomOp::AppendChild { parent_id: 1, child_id: 16 };
```

The variants (op code in parentheses): `CreateElement` (0), `CreateTextNode` (1),
`SetText` (2), `SetAttribute` (3), `RemoveAttribute` (4), `SetProperty` (5),
`AddEventListener` (6), `RemoveEventListener` (7), `AppendChild` (8),
`RemoveChild` (9), `RemoveNode` (10), `InsertBefore` (11), `ReplaceNode` (12),
`SetStyle` (13), `AddClass` (14), `RemoveClass` (15), `MorphNode` (16, decision
027), `RegisterNode` (17), `UnregisterNode` (18).

`MorphNode` carries a `TargetSelector` (`NodeId` / `Id` / `Class` / `Query`) and a
`MorphAction` (`ReplaceChildren` / `ReplaceElement` / `InsertBefore` /
`InsertAfter` / `AppendSibling`, with `as_u8`/`from_u8` wire discriminants).

Tag/attribute names ride as compact ids via `HtmlTag`/`AttrName`; free-form values
are `Cow<'static, str>` so static templates encode without allocation. The
`NodeRegistry` ops (17/18) are an explicit `node_id -> element` mapping — creates
do **not** auto-register; `ReplaceNode`/`RemoveNode` implicitly unregister.

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

## Encoders & the envelope

A `ProtocolEncoder<T>` is a bidirectional, stateless codec for a payload type:

```rust
pub trait ProtocolEncoder<T> {
    fn protocol_byte(&self) -> u8;
    fn version(&self) -> u8;
    fn encode(&self, data: T) -> Vec<u8>;                 // payload bytes, no envelope
    fn decode(&self, payload: &[u8]) -> DecodeResult<T>;  // typed DecodeError on failure
}
```

```rust
use foundation_ui_traits::{ColumnarEncoder, JsonEncoder, ProtocolEncoder, DomOp};

let ops: Vec<DomOp> = vec![/* … */];
let bytes = ColumnarEncoder.encode(ops.clone());     // compact columnar (protocol byte 1, v1)
let back  = ColumnarEncoder.decode(&bytes).unwrap();
let json  = JsonEncoder.encode(ops);                 // readable JSON (protocol byte 2)
```

This crate ships two encoders:

- **`ColumnarEncoder`** / `ColumnarBatch` — the compact-columnar wire v1.1:
  fixed-width `op_id`/`node_id` columns plus three string columns, 8-byte aligned
  so the JS runtime can build zero-copy `TypedArray` views over the buffer.
- **`JsonEncoder`** — readable JSON `DomOp`s, for debugging the loop.

Every encoder serialises a `DomOp` through one shared mapping — `row_view` (a
borrowing visitor) and `Row` (its owned form) — keeping all formats bug-for-bug
consistent with each other and with JS. The decision-010 protocol bytes
(`PROTOCOL_CUSTOM_BINARY` = 0, `PROTOCOL_ARROW` = 1, `PROTOCOL_JSON` = 2),
`PROTOCOL_VERSION`, `OP_COUNT`, and `DecodeError`/`DecodeResult` are all exported
here.

The **`Envelope`** is the tiny 6-byte self-describing header
`[protocol:1][version:1][length:4 LE]` that precedes a payload so a receiver
(HTTP/SSE/WS) can demux the format without out-of-band metadata.
`encode_with_envelope` encodes-and-wraps in one step; `ENVELOPE_SIZE` is the
header length.

```rust
use foundation_ui_traits::{Envelope, encode_with_envelope, ColumnarEncoder, DomOp};

let framed = encode_with_envelope(&ColumnarEncoder, Vec::<DomOp>::new());
let (env, payload) = Envelope::parse(&framed).unwrap();  // (Envelope { protocol, version, length }, &[u8])
```

> Protocol byte 0 (Custom Binary) is the `foundation_wasm` batch-instructions
> stream (decision 022); its `ProtocolHandler` lives in `foundation_wasm_ui`,
> **not** here. The real Apache Arrow IPC encoder (wire v2) lives in
> `foundation_arrow`.

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

## What this crate is NOT for

- **No runtime, no DOM, no signals.** There is no reactive graph, no effect
  scheduler, no `host_apply`, no FFI here — only types and codecs. The graph is
  `foundation_signals`; the DOM application is JS; the WASM transport is
  `foundation_wasm`.
- **Not the HTML parser.** `html!` parsing/codegen lives in `foundation_macros`;
  this crate only defines the types that codegen targets.
- **Not where you add wire ids freely.** `TAG_NAMES`/`ATTR_NAMES` are ABI —
  append only, never reorder, because the JS runtime mirrors them.
- **Not the Custom-Binary or Arrow encoders.** Byte-0 batch instructions live in
  `foundation_wasm_ui`; real Arrow IPC lives in `foundation_arrow`. This crate
  ships only the compact-columnar and JSON encoders.
