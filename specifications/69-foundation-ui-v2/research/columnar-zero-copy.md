# Idea 1 and 4: no virtual DOM, columnar zero-copy typed arrays

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** user interface (UI); Document Object Model (DOM);
WebAssembly (WASM); JavaScript (JS); foreign function interface (FFI); inter-process
communication (IPC); central processing unit (CPU); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. What the document proposes

Source: `foundation_ui_rearchitecture.md` §1 lines 9-13, §3 lines 142-151.

- No virtual DOM and no client-side diff. Each update targets an element that has a
  compile-time id.
- Custom 8-byte aligned columnar blocks.
- In the browser, a small JS applicator reads the envelope, then makes a zero-copy
  `Uint32Array` view over the columnar bytes and applies the rows to the DOM. The
  document says this path "bypasses the WebAssembly runtime execution thread".
- Apache Arrow IPC for large tabular pushes.

## 2. What we have now

Most of this idea is already built and tested (spec 39, features 05, 07, 19).

| Item | Location |
|---|---|
| Compile-time element ids (`primal-id`) from the `html!` macro | `backends/foundation_macros/src/html_macro/codegen.rs:465-481`; decision 005 in `specifications/completed/39-foundation-wasm-ui/decisions/005-element-id-and-part-granularity.md` |
| `DomOp`, 19 variants, `u32` node ids | `backends/foundation_ui_traits/src/dom_op.rs:91` |
| Explicit `NodeRegistry` semantics (ops 17, 18) | `backends/foundation_ui_traits/src/dom_op.rs:13-27` |
| Columnar v1.1 layout with 8-byte header alignment | `backends/foundation_ui_traits/src/columnar_encoder.rs:13-32`, `:49` (`RELATIVE_ALIGN_PAD = 7`) |
| Columnar accumulator that writes each string byte once | `backends/foundation_ui_traits/src/columnar_encoder.rs:92` (`ColumnarBatch`) |
| JS `ColumnarParser` with zero-copy `Uint32Array` views and a copy fallback | `backends/foundation_wasm_ui/runtimes/foundation-wasm-ui.js:94-187` (alignment check at `:129`, view at `:148`) |
| JS `columnarHandler` (parse, then apply) | `backends/foundation_wasm_ui/runtimes/foundation-wasm-ui.js:802` |
| Real Arrow IPC encoder for `DomOp` | `backends/foundation_arrow/src/dom_ops.rs:54`, `:164` |
| Specification | `specifications/completed/39-foundation-wasm-ui/features/19-columnar-zero-copy/`, `decisions/010-arrow-batch-schema.md`, `decisions/011-no-string-keys.md` |

The document's claim "no virtual DOM" is already true. Spec 39 has no virtual DOM. The one
place that compares trees is `DomOp::MorphNode` (decision 027), which morphs an HTML string
into a live subtree (`dom_op.rs:159`).

## 3. Gap

1. **Location of the applicator.** Today the WASM module encodes ops and ships them through
   `host_apply` (`backends/foundation_wasm_ui/src/protocol/mod.rs`). The JS applicator then
   applies them. For server-sent frames, the bytes arrive in JS first and JS applies them. So
   the "bypass WASM" path the document describes already exists for server frames. There is
   no gap here for the web.
2. **No schema.** The columnar layout is hand-written in Rust and parsed by hand in
   JavaScript. Each new platform needs another hand-written parser. In a FlatBuffers schema
   the readers are generated. The vector alignment must be kept; see
   [`wire-envelope.md`](wire-envelope.md) §4 risk 1.
3. **Native.** No native consumer of the columnar body exists. A native renderer reads
   memory directly, so it gets zero-copy with any aligned layout. It does not need typed
   arrays.

## 4. Open risks

1. Zero-copy only helps when frames are large. Most UI updates are a few ops. For small
   frames, a FlatBuffers table read is about as fast. We do not have a benchmark that shows
   when the columnar form wins. We must measure before we make it the only format.
2. Strings still need a `TextDecoder` call per cell in JS. The numeric columns are zero-copy;
   the string columns are not. The document's "zero object allocation" claim is too strong.
3. `MorphNode` carries HTML. That works on the web only.

## 5. Recommendation: **adopt the idea, move it into the FlatBuffers schema**

Owner direction (PR #157 review): columnar is a type in the FlatBuffers schema, generated for
every language. It is not a separate hand-written format.

- Define the columnar `DomOp` batch as a FlatBuffers table of typed vectors: operation codes
  (`[ubyte]`), node ids (`[uint32]`), string offsets (`[uint32]`) and one string byte vector.
  Put `force_align: 8` on the numeric vectors.
- The zero-copy gain stays. A FlatBuffers vector of scalars is a contiguous little-endian
  array. The `flatc` TypeScript reader returns a typed-array view (for example `Uint32Array`)
  over the received buffer for such a vector, so JavaScript reads the columns with no copy and
  no hand-written parser. Swift and Kotlin readers get the same columns from their generated
  code.
- Retire `ColumnarBatch` (`columnar_encoder.rs`) and the JavaScript `ColumnarParser`
  (`foundation-wasm-ui.js:94-187`) when the generated form matches them in a benchmark (1, 10,
  100 and 10 000 operations). Until then they stay, so nothing breaks.
- The Arrow IPC `DomOp` encoder (`foundation_arrow/src/dom_ops.rs`) is not needed for UI
  frames once the schema type exists. See Q20 in [`questions.md`](questions.md).
- Native comes first (owner decision D4), so the first consumer of the schema type is the
  native renderer, not the web.
