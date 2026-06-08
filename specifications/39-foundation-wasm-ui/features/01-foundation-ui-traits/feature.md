# Feature 01: foundation_ui_traits Crate

## Description

Create `foundation_ui_traits` — a shared crate that both `foundation_signals` and `foundation_wasm_ui` depend on. Holds the common types: `IntoHtml` trait, `Html` struct, `Part` descriptors, and `DomOp` enum. This breaks the circular dependency: signals need `IntoHtml` on `Signal<T>`, but signals can't depend on the UI crate.

**Decisions:** 007, 012

## Crate

`crates/foundation_ui_traits/` (lightweight, no runtime dependencies)

## Types

### IntoHtml trait

```rust
/// Converts a value into an `Html` fragment.
pub trait IntoHtml {
    fn into_html(self) -> Html;
}
```

Implementations:
- `Html` — identity
- `Vec<Html>` — flattened into children
- `impl Iterator<Item = Html>` — collected into children
- `Option<Html>` — `Some` = child, `None` = nothing
- `&str`, `String`, `&Html` — text content
- Primitives: `usize`, `isize`, `u8`-`u64`, `i8`-`i64`, `bool`, `f32`, `f64` — via `Display`
- `&Signal<T: IntoHtml + Clone>` — implemented in `foundation_signals` (depends on this crate)

### Html struct

```rust
pub struct Html {
    pub tag: Option<String>,
    pub attributes: Vec<(String, String)>,
    pub children: Vec<Html>,
    pub text: Option<String>,
    pub parts: Vec<Part>,
}
```

Simple tree — not arena-allocated like `foundation_html`'s `Fragment`.

### Part descriptors

```rust
pub enum Part {
    Text(TextPart),
    Attribute(AttrPart),
    Event(EventPart),
    Children(ChildPart),
}

pub struct TextPart {
    pub node_id: u32,     // primal-id of the element
}

pub struct AttrPart {
    pub node_id: u32,
    pub attr_name: String,
}

pub struct EventPart {
    pub node_id: u32,
    pub event_name: String,
}

pub struct ChildPart {
    pub parent_id: u32,
}
```

Each `Part` describes a dynamic slot. The mount step creates one effect per Part.

### DomOp enum

```rust
pub enum DomOp {
    SetText { node_id: u32, text: String },
    SetAttr { node_id: u32, name: String, value: String },
    SetClass { node_id: u32, class: String },
    SetStyle { node_id: u32, prop: String, value: String },
    CreateEl { node_id: u32, tag: String, class: String },
    AppendChild { parent_id: u32, child_id: u32 },
    Remove { node_id: u32 },
    InsertBefore { parent_id: u32, child_id: u32, ref_id: u32 },
    Replace { old_id: u32, new_id: u32 },
    Morph { node_id: u32, html: String },
}
```

## Dependencies

- None

## Testing

- IntoHtml for all primitives → produces correct text Html
- IntoHtml for Option/Vec/Iterator → correct child structure
- Html struct serializes to expected HTML string
- DomOp variants have correct field names
