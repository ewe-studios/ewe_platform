# Feature 01: foundation_ui_traits Crate

**Crate path:** `crates/foundation_ui_traits/`
**Decisions:** 007 (IntoHtml), 010 (Arrow batch schema), 012 (crate split), 015 (protocol layers)
**Constraint:** No WASM dependency, no FFI, no MemoryAllocations. Must compile for any Rust target.

Shared type and encoding crate beneath `foundation_signals` and `foundation_wasm_ui`.
Owns: `IntoHtml` trait, `Html` struct, `Part` descriptors, the full 17-variant `DomOp`
enum, `ProtocolEncoder<T>` trait, three concrete encoders, and the `Envelope` format.

## 1. Types and Structs

### Html

```rust
pub struct Html {
    pub tag: Option<String>,         // None = text node, Some = element
    pub attributes: Vec<(String, String)>,
    pub children: Vec<Html>,
    pub text: Option<String>,
    pub parts: Vec<Part>,            // Reactive binding descriptors from html! macro
}
```

### Part Descriptors

```rust
pub enum Part { Text(TextPart), Attribute(AttrPart), Event(EventPart) }
// Note: ChildPart was removed — all child positions generate Part::Text.
// Vec<Html> expressions are handled at runtime via IntoHtml (wraps as tagless node).
pub struct TextPart   { pub node_id: u32 }
pub struct AttrPart   { pub node_id: u32, pub attr_name: String }
pub struct EventPart  { pub node_id: u32, pub event_name: String }
```

### DomOp Enum (19 variants)

Each variant maps 1:1 to an Arrow operation u8 discriminant (decision 010).

Two ops (17-18) manage the JS-side NodeRegistry explicitly — no implicit registration.
Every element that will be referenced by subsequent ops must be registered first.

// Target selector — compact wire representation
pub enum TargetSelector {
    NodeId(u32),                                    // op=0: direct registry lookup
    Id(Cow<'static, str>),                          // op=1: #id
    Class(Cow<'static, str>),                       // op=2: .class (first match)
    Query(Cow<'static, str>),                       // op=3: arbitrary CSS query
}

// Morph action — small enum, saves wire bytes vs string
pub enum MorphAction {
    ReplaceChildren,   // 0: morph target's children
    ReplaceElement,    // 1: replace target itself
    InsertBefore,      // 2: insert before target
    InsertAfter,       // 3: insert after target
    AppendSibling,     // 4: append as sibling
}

pub enum DomOp {
    CreateElement       { node_id: u32, tag: HtmlTag, class: Cow<'static, str> },  // op=0
    CreateTextNode      { node_id: u32, content: Cow<'static, str> },               // op=1
    SetText             { node_id: u32, text: Cow<'static, str> },                  // op=2
    SetAttribute        { node_id: u32, name: AttrName, value: Cow<'static, str> }, // op=3
    RemoveAttribute     { node_id: u32, name: AttrName },                           // op=4
    SetProperty         { node_id: u32, name: AttrName, value: Cow<'static, str> }, // op=5
    AddEventListener    { node_id: u32, event_name: AttrName },                     // op=6
    RemoveEventListener { node_id: u32, event_name: AttrName },                     // op=7
    AppendChild         { parent_id: u32, child_id: u32 },                          // op=8
    RemoveChild         { parent_id: u32, child_id: u32 },                          // op=9
    RemoveNode          { node_id: u32 },                                           // op=10
    InsertBefore        { parent_id: u32, child_id: u32, ref_id: u32 },             // op=11
    ReplaceNode         { old_id: u32, new_id: u32 },                               // op=12
    SetStyle            { node_id: u32, prop: AttrName, value: Cow<'static, str> }, // op=13
    AddClass            { node_id: u32, class: Cow<'static, str> },                 // op=14
    RemoveClass         { node_id: u32, class: Cow<'static, str> },                 // op=15
    MorphNode           { target: TargetSelector, action: MorphAction,              // op=16
                          content: Cow<'static, str> },
    RegisterNode        { node_id: u32 },                                           // op=17
    UnregisterNode      { node_id: u32 },                                           // op=18
}
```

**MORPH_NODE is self-contained in Arrow:** carries target selector, action enum, and content.
JS reads all three, resolves target, parses content, morphs.

**JSON morphing wrapper:**
```json
{ "morph": { "target": "#main", "action": "replace-children", "content": "<div>...</div>" } }
```

**HTML morphing:** Server sends HTML inside an `<island>` component that carries the target
and action as attributes:
```html
<island data-target="#main" data-action="replace-children">
  <div>...new content...</div>
</island>
```
The island web component reads the attributes, resolves the target, and morphs.

**RegisterNode / UnregisterNode semantics:**
- `RegisterNode { node_id }` — JS adds `node_id → DOM Element` to NodeRegistry. The element must already exist in the DOM (created by a prior `CreateElement`, `CreateTextNode`, or found via `querySelector` for existing elements like `<head>`). If `node_id` is already registered, this is a no-op.
- `UnregisterNode { node_id }` — JS removes `node_id` from NodeRegistry. If `node_id` is not registered, this is a no-op. The DOM element is NOT removed — this only clears the registry entry. Typically paired with `RemoveNode` (which removes from DOM) or used standalone for elements that are DOM-removed by browser navigation.
- `CreateElement` and `CreateTextNode` do NOT auto-register. The caller must emit `RegisterNode` explicitly after creation, before any op that references the node_id. This makes registry ownership explicit and testable.
- `ReplaceNode` implicitly unregisters `old_id` and registers `new_id` — no separate ops needed.
- `RemoveNode` implicitly unregisters `node_id` — no separate op needed.

**Arrow column mapping:** `HtmlTag` and `AttrName` values are encoded as: known IDs become
decimal strings `"id:1234"` (prefixed with `id:` to distinguish from string names). Unknown names
are stored as the raw string. The JS parser detects the `id:` prefix and resolves via lookup table.
Secondary node IDs (child_id, new_id, ref_id) are stored as plain decimal strings. This keeps the
Arrow schema uniform at six columns.

| op | Variant | node_id | attribute | value | text_val |
|----|---------|---------|-----------|-------|----------|
| 0 | CreateElement | new_id | tag | class | |
| 1 | CreateTextNode | new_id | | | content |
| 2 | SetText | target | | | text |
| 3 | SetAttribute | target | name | val | |
| 4 | RemoveAttribute | target | name | | |
| 5 | SetProperty | target | name | serialized_val | |
| 6 | AddEventListener | target | | event_name | |
| 7 | RemoveEventListener | target | | event_name | |
| 8 | AppendChild | parent | child_id | | |
| 9 | RemoveChild | parent | child_id | | |
| 10 | RemoveNode | target | | | |
| 11 | InsertBefore | parent | child_id | | ref_id |
| 12 | ReplaceNode | old_id | new_id | | |
| 13 | SetStyle | target | prop | val | |
| 14 | AddClass | target | | class | |
| 15 | RemoveClass | target | | class | |
| 16 | MorphNode | target_id(0 if selector) | action_enum + selector_str | content |
| 17 | RegisterNode | node_id | | | |
| 18 | UnregisterNode | node_id | | | |

## 2. IntoHtml Trait and Implementations

```rust
pub trait IntoHtml { fn into_html(self) -> Html; }
```

**G11 resolved — `ProtocolEncoder<T>` generic scope:** The trait is only implemented for concrete
types: `ProtocolEncoder<Vec<DomOp>>`, `ProtocolEncoder<SignalPatch>`, `ProtocolEncoder<Action>`.
No blanket implementations. Each type gets its own encoder impl. The trait's `T` parameter exists
for future extensibility but the current codebase only uses `Vec<DomOp>`.

**G12/G13/G14 resolved — Tag/Attribute wire optimization:**

Known tag names and attribute names are assigned numeric IDs (`u16`). Unknown names (custom elements, user-defined attributes) fall back to `Cow<'static, str>`. This saves wire bytes — most DOM ops use known tags/attrs.

```rust
// Known tags assigned u16 IDs at compile time
pub enum HtmlTag { Id(u16), Name(Cow<'static, str>) }
pub enum AttrName { Id(u16), Name(Cow<'static, str>) }

// Known tag IDs (partial list)
pub const TAG_DIV: u16 = 1;
pub const TAG_SPAN: u16 = 2;
pub const TAG_INPUT: u16 = 3;
pub const TAG_BUTTON: u16 = 4;
// ... all standard HTML elements

// Known attribute IDs
pub const ATTR_CLASS: u16 = 1;
pub const ATTR_ID: u16 = 2;
pub const ATTR_STYLE: u16 = 3;
pub const ATTR_VALUE: u16 = 4;
// ... common attributes
```

**Wire format:** Protocol encodes known tags/attrs as `[type: u8 = 0][id: u16 LE]` (3 bytes). Unknown as `[type: u8 = 1][len: u16 LE][utf8...]` (variable). JS side has matching lookup tables to resolve IDs to DOM method names.

```rust
pub struct Html {
    pub tag: Option<HtmlTag>,
    pub attributes: Vec<(AttrName, Cow<'static, str>)>,
    pub children: Vec<Html>,
    pub text: Option<Cow<'static, str>>,
    pub parts: Vec<Part>,
}
```

**18 implementations in this crate:**

| # | Type | Strategy |
|---|------|----------|
| 1 | `Html` | Identity (return self) |
| 2 | `Vec<Html>` | Wrap as children of a tagless node |
| 3 | `Option<Html>` | `Some` unwraps, `None` produces empty node |
| 4 | `&str` | Text node via `to_string()` |
| 5 | `String` | Text node (move) |
| 6-18 | `usize`, `isize`, `u8`, `u16`, `u32`, `u64`, `i8`, `i16`, `i32`, `i64`, `bool`, `f32`, `f64` | Display-based: `self.to_string()` into text node |

All 13 primitive impls use a single macro:

```rust
macro_rules! impl_into_html_display {
    ($($t:ty),*) => { $(
        impl IntoHtml for $t {
            fn into_html(self) -> Html {
                Html { tag: None, attributes: vec![], children: vec![],
                       text: Some(self.to_string()), parts: vec![] }
            }
        }
    )* };
}
impl_into_html_display!(usize, isize, u8, u16, u32, u64, i8, i16, i32, i64, bool, f32, f64);
```

**Not in this crate:** `&Signal<T: IntoHtml + Clone>` is implemented in `foundation_signals`.

## 3. ProtocolEncoder Trait

```rust
pub trait ProtocolEncoder<T> {
    fn protocol_byte(&self) -> u8;  // 0=CustomBinary, 1=Arrow, 2=JSON
    fn version(&self) -> u8;       // Starts at 1
    fn encode(&self, data: T) -> Vec<u8>;
    fn decode(&self, payload: &[u8]) -> DecodeResult<T>;
}
```

### DecodeResult and DecodeError

```rust
pub enum DecodeResult<T> { Success(T), Error(DecodeError) }

pub enum DecodeError {
    TruncatedBuffer   { expected_min: usize, actual: usize },
    UnknownOperation  { op_id: u8, row_index: usize },
    InvalidUtf8       { column_name: &'static str, row_index: usize },
    SchemaMismatch    { detail: String },
    JsonParseError    { detail: String },
    Other             { detail: String },
}
```

## 4. Encoder Internals

### 4.1 ArrowEncoder (`protocol_byte=1`)

Uses the `arrow` crate with `features = ["ipc"]` for encoding/decoding.

**Encoding algorithm (Vec<DomOp> to Arrow IPC bytes):**

1. Allocate six Arrow array builders: `UInt32Builder` (op_ids, node_ids),
   `UInt8Builder` (operations), `StringBuilder` (attributes, values, text_vals).
2. For each `DomOp`, push a monotonic `op_id`, extract node_id/parent_id/old_id into
   `node_ids`, push the operation discriminant (0-18), and map variant fields into the
   three string columns per the table in section 1. Unused columns get `append_null()`.
3. Build an Arrow `RecordBatch`: `op_id` UInt32, `node_id` UInt32, `operation` UInt8,
   `attribute` Utf8 nullable, `value` Utf8 nullable, `text_val` Utf8 nullable.
4. Serialize as Arrow IPC stream format (single batch) and return bytes.

**Decoding algorithm:**

1. Use `arrow::ipc::reader::StreamReader` over byte slice. Read single RecordBatch
   (zero rows = `Success(vec![])`).
2. Validate 6 columns with correct types, else `SchemaMismatch`.
3. For each row: read `operation` u8, read `node_id` u32, read nullable strings.
   Match operation 0-18 to reconstruct DomOp. Unknown u8 returns `UnknownOperation`.
   Invalid UTF-8 returns `InvalidUtf8`.

**Buffer layout:**
```
[Schema message][RecordBatch: 6 columns x N rows][EOS marker (0xFFFFFFFF)]
```

### 4.2 JsonEncoder (`protocol_byte=2`)

Serializes `Vec<DomOp>` as a JSON array of flat objects with the same field names as
the Arrow columns:

```json
[
  { "op_id": 0, "node_id": 5, "operation": 2,
    "attribute": null, "value": null, "text_val": "hello" },
  { "op_id": 1, "node_id": 10, "operation": 0,
    "attribute": "div", "value": "main", "text_val": null }
]
```

Uses `serde_json::to_vec` for encoding, `serde_json::from_slice` for decoding, then
the same u8-to-DomOp matching logic as ArrowEncoder.

### 4.3 CustomBinaryEncoder (`protocol_byte=0`)

Follows the existing `ops.rs` pattern in `backends/foundation_wasm/src/ops.rs` — compact
binary with tag bytes and length-prefixed strings:

```
Per DomOp: [operation: u8][node_id: u32 LE][...variant fields]
  Strings: [len: u32 LE][utf8 bytes]
  Secondary IDs: [id: u32 LE]
```

Same wire format as the existing foundation_wasm encoding but expressed as a pure
function over `DomOp` with no WASM or memory allocation dependencies.

## 5. Envelope

### Byte Layout (6 bytes)

```
Offset  Size  Field      Description
0       1     protocol   0=CustomBinary, 1=Arrow, 2=JSON
1       1     version    Format version (starts at 1)
2       4     length     Payload length, u32 little-endian

 0   1   2   3   4   5   6            6+N
 ┌───┬───┬───┬───┬───┬───┬────────────┐
 │ P │ V │ L0│ L1│ L2│ L3│ payload... │
 └───┴───┴───┴───┴───┴───┴────────────┘
```

**No memory_id in the base envelope.** That field is added by the WASM transport layer
(`foundation_wasm::protocol`) for the WASM-JS boundary only.

### Methods

```rust
pub const ENVELOPE_SIZE: usize = 6;

impl Envelope {
    /// Build complete message: 6-byte header + payload.
    pub fn write(protocol: u8, version: u8, payload: &[u8]) -> Vec<u8>;

    /// Parse buffer into Envelope + payload slice.
    /// Returns None if buffer < 6 bytes or declared length exceeds available bytes.
    pub fn parse(bytes: &[u8]) -> Option<(Envelope, &[u8])>;
}

/// Encode data and wrap in envelope in one step.
pub fn encode_with_envelope<T>(encoder: &impl ProtocolEncoder<T>, data: T) -> Vec<u8>;
```

## 6. Error Cases

| Scenario | Result |
|----------|--------|
| Buffer < 6 bytes | `Envelope::parse` returns `None` |
| Payload shorter than declared length | `Envelope::parse` returns `None` |
| Unknown operation u8 in payload | `DecodeError::UnknownOperation` with op_id and row_index |
| Invalid UTF-8 in string column | `DecodeError::InvalidUtf8` with column name and row index |
| Arrow column count or type wrong | `DecodeError::SchemaMismatch` |
| Malformed JSON | `DecodeError::JsonParseError` |
| Zero-row batch | Not an error: returns `Success(vec![])` |
| Unknown protocol byte in envelope | Dispatcher-level concern, not encoder-level |
| Encoding well-formed DomOps | Infallible (always succeeds) |

## 7. Integration Points

| Crate | Uses from this crate | Purpose |
|-------|---------------------|---------|
| `foundation_signals` | `IntoHtml`, `Html` | `impl IntoHtml for &Signal<T>` |
| `foundation_wasm_ui` | All types + encoders | Builds DomOp batches, encodes, sends via WASM transport |
| `foundation_wasm` | `DomOp`, encoders, `Envelope` | WASM transport wraps payloads with `memory_id` |
| HTTP/SSE/WS servers | Encoders, `DomOp`, `Envelope` | Server-side rendering and live patching without WASM |

**Dependency direction:**
```
foundation_ui_traits  (no dependencies)
        ^         ^
        |         |
foundation_signals  foundation_wasm
        ^               ^
        └──foundation_wasm_ui──┘
```

## 8. File Ownership

```
crates/foundation_ui_traits/src/
├── lib.rs              // Re-exports
├── html.rs             // Html struct, IntoHtml trait + all 18 impls
├── parts.rs            // Part, TextPart, AttrPart, EventPart, ChildPart
├── dom_op.rs           // DomOp enum (19 variants)
├── encoder.rs          // ProtocolEncoder<T>, DecodeResult, DecodeError
├── arrow_encoder.rs    // ArrowEncoder
├── json_encoder.rs     // JsonEncoder
├── binary_encoder.rs   // CustomBinaryEncoder
└── envelope.rs         // Envelope, ENVELOPE_SIZE, encode_with_envelope
```

## 9. Refactoring Strategy

1. Create the crate with all types/traits. Write and pass all tests.
2. Add as dependency of `foundation_signals` and `foundation_wasm`. Re-export types
   via `pub use` so existing downstream code compiles unchanged.
3. Migrate callers to import from `foundation_ui_traits` directly. Remove re-exports.
4. Delete duplicate definitions from old crates.
5. For `CustomBinaryEncoder`: verify byte-identical output against existing `ops.rs`
   encoding before replacing the old code with a delegating wrapper.

## 10. Testing

### IntoHtml (tests 1-10)

| # | Scenario | Verify |
|---|----------|--------|
| 1 | `Html` identity | `html.into_html() == html` |
| 2 | `"hello"` (&str) | `text == Some("hello"), tag == None` |
| 3 | `String::from("world")` | Same structure as &str |
| 4 | `42u32` | `text == Some("42")` |
| 5 | `true` | `text == Some("true")` |
| 6 | `3.14f64` | `text == Some("3.14")` |
| 7 | All 13 primitives | Each produces text node with Display output |
| 8 | `Some(html)` | Returns inner Html |
| 9 | `None::<Html>` | Empty Html node |
| 10 | `vec![a, b]` | Two children, no tag |

### ArrowEncoder round-trip (tests 11-17)

| # | Scenario | Verify |
|---|----------|--------|
| 11 | 5 mixed DomOps | Encode, decode, all 5 match field-by-field |
| 12 | Empty Vec | Valid zero-row Arrow buffer, decodes to `Success(vec![])` |
| 13 | Single CreateElement | Tag and class survive round-trip |
| 14 | All 19 variants | Batch of 19 (one per variant), all match after round-trip |
| 15 | 1000 SetText ops | Unique text per op, all 1000 texts match after round-trip |
| 16 | Unicode (CJK, emoji, RTL) | Byte-exact match after round-trip |
| 17 | Hand-crafted op=99 | Returns `UnknownOperation { op_id: 99, row_index: 0 }` |

### JsonEncoder round-trip (tests 18-21)

| # | Scenario | Verify |
|---|----------|--------|
| 18 | 5 mixed DomOps | Encode, decode, all match |
| 19 | Empty Vec | Decodes to `Success(vec![])` |
| 20 | All 19 variants | All match after round-trip |
| 21 | JSON shape | Parse output as serde_json::Value, verify array of objects with correct field names |

### CustomBinaryEncoder round-trip (tests 22-24)

| # | Scenario | Verify |
|---|----------|--------|
| 22 | 5 mixed DomOps | Encode, decode, all match |
| 23 | Empty Vec | Decodes to `Success(vec![])` |
| 24 | All 17 variants | All match after round-trip |

### Envelope (tests 25-29)

| # | Scenario | Verify |
|---|----------|--------|
| 25 | Write then parse | Round-trip preserves protocol, version, length, payload |
| 26 | Empty payload | 6 bytes total, length=0, empty payload slice |
| 27 | Truncated (< 6 bytes) | Returns `None` |
| 28 | Length exceeds buffer | Returns `None` |
| 29 | `encode_with_envelope` | First byte is protocol, parse extracts payload that decodes correctly |

### Error paths (tests 30-33)

| # | Scenario | Verify |
|---|----------|--------|
| 30 | Invalid UTF-8 in Arrow string column | `InvalidUtf8 { column_name: "text_val", ... }` |
| 31 | Arrow buffer with 4 columns | `SchemaMismatch` |
| 32 | `JsonEncoder.decode(b"not json")` | `JsonParseError` |
| 33 | `ArrowEncoder.decode(&[])` | `TruncatedBuffer` or `SchemaMismatch` |

### Cross-encoder consistency (tests 34-35)

| # | Scenario | Verify |
|---|----------|--------|
| 34 | Arrow vs JSON on same 10 ops | Both decode to identical `Vec<DomOp>` |
| 35 | All 3 encoders on same batch | All decode to identical `Vec<DomOp>` |
