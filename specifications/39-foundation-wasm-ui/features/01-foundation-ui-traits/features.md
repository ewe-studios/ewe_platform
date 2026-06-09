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
pub enum Part { Text(TextPart), Attribute(AttrPart), Event(EventPart), Children(ChildPart) }
pub struct TextPart   { pub node_id: u32 }
pub struct AttrPart   { pub node_id: u32, pub attr_name: String }
pub struct EventPart  { pub node_id: u32, pub event_name: String }
pub struct ChildPart  { pub parent_id: u32 }
```

### DomOp Enum (17 variants)

Each variant maps 1:1 to an Arrow operation u8 discriminant (decision 010).

```rust
pub enum DomOp {
    CreateElement       { node_id: u32, tag: String, class: String },       // op=0
    CreateTextNode      { node_id: u32, content: String },                  // op=1
    SetText             { node_id: u32, text: String },                     // op=2
    SetAttribute        { node_id: u32, name: String, value: String },      // op=3
    RemoveAttribute     { node_id: u32, name: String },                     // op=4
    SetProperty         { node_id: u32, name: String, value: String },      // op=5
    AddEventListener    { node_id: u32, event_name: String },               // op=6
    RemoveEventListener { node_id: u32, event_name: String },               // op=7
    AppendChild         { parent_id: u32, child_id: u32 },                  // op=8
    RemoveChild         { parent_id: u32, child_id: u32 },                  // op=9
    RemoveNode          { node_id: u32 },                                   // op=10
    InsertBefore        { parent_id: u32, child_id: u32, ref_id: u32 },     // op=11
    ReplaceNode         { old_id: u32, new_id: u32 },                       // op=12
    SetStyle            { node_id: u32, prop: String, value: String },      // op=13
    AddClass            { node_id: u32, class: String },                    // op=14
    RemoveClass         { node_id: u32, class: String },                    // op=15
    MorphNode           { node_id: u32, html: String },                     // op=16
}
```

**Arrow column mapping:** Secondary node IDs (child_id, new_id, ref_id) are stored as
decimal strings in the `attribute`, `value`, or `text_val` columns. The JS parser
converts back to integers. This keeps the Arrow schema uniform at six columns.

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
| 16 | MorphNode | target | | | html |

## 2. IntoHtml Trait and Implementations

```rust
pub trait IntoHtml { fn into_html(self) -> Html; }
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

**Encoding algorithm (Vec<DomOp> to Arrow IPC bytes):**

1. Allocate six column builders: `op_ids: Vec<u32>`, `node_ids: Vec<u32>`,
   `operations: Vec<u8>`, `attributes: Vec<Option<String>>`,
   `values: Vec<Option<String>>`, `text_vals: Vec<Option<String>>`.
2. For each `DomOp`, push a monotonic `op_id`, extract node_id/parent_id/old_id into
   `node_ids`, push the operation discriminant (0-16), and map variant fields into the
   three string columns per the table in section 1. Unused columns get `None`.
3. Build an Arrow RecordBatch: `op_id` UInt32, `node_id` UInt32, `operation` UInt8,
   `attribute` Utf8 nullable, `value` Utf8 nullable, `text_val` Utf8 nullable.
4. Serialize as Arrow IPC stream format (single batch) and return bytes.

**Decoding algorithm:**

1. Parse Arrow IPC stream, extract single RecordBatch.
2. Validate 6 columns with correct types, else `SchemaMismatch`.
3. For each row: read `operation` u8, read `node_id` u32, read nullable strings.
   Match operation 0-16 to reconstruct DomOp. Unknown u8 returns `UnknownOperation`.
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
├── dom_op.rs           // DomOp enum (17 variants)
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
| 14 | All 17 variants | Batch of 17 (one per variant), all match after round-trip |
| 15 | 1000 SetText ops | Unique text per op, all 1000 texts match after round-trip |
| 16 | Unicode (CJK, emoji, RTL) | Byte-exact match after round-trip |
| 17 | Hand-crafted op=99 | Returns `UnknownOperation { op_id: 99, row_index: 0 }` |

### JsonEncoder round-trip (tests 18-21)

| # | Scenario | Verify |
|---|----------|--------|
| 18 | 5 mixed DomOps | Encode, decode, all match |
| 19 | Empty Vec | Decodes to `Success(vec![])` |
| 20 | All 17 variants | All match after round-trip |
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
