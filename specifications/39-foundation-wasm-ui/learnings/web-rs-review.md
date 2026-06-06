# web.rs — Full Architecture Review

> **Source**: `@formulas/src.rust/src.wasm/src.webrs/web.rs` (git: `richardanaya/js-wasm`)
> **Purpose**: A minimal, no-std Rust library for writing browser applications compiled to `wasm32-unknown-unknown`. Designed to be learned in an afternoon — trades completeness for simplicity.

---

## Table of Contents

1. [High-Level Architecture](#1-high-level-architecture)
2. [The JS Bridge Layer (`js` crate)](#2-the-js-bridge-layer-js-crate)
3. [The JavaScript Runtime (`js-wasm.js`)](#3-the-javascript-runtime-js-wasmjs)
4. [The `#[web::main]` Proc Macro (`web_macro` crate)](#4-the-webmain-proc-macro-web_macro-crate)
5. [TypeScript Definition Parser & Rust Code Generator (`web_gen` crate)](#5-the-typescript-definition-parser--rust-code-generator-web_gen-crate)
6. [The Async Executor (`executor`)](#6-the-async-executor-executor)
7. [The `web` Crate — Web API Surface](#7-the-web-crate--web-api-surface)
8. [Memory Management Across the Boundary](#8-memory-management-across-the-boundary)
9. [Event Handling Pattern](#9-event-handling-pattern)
10. [Critical Assessment — Strengths, Weaknesses, Lessons](#10-critical-assessment--strengths-weaknesses-lessons)

---

## 1. High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Browser (JS Runtime)                      │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                    js-wasm.js (runtime)                    │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌─────────────────┐  │  │
│  │  │ Generational  │  │  Env Imports │  │  Context Object │  │  │
│  │  │ Arena         │  │  (WASM ↕)   │  │  (memory, strs, │  │  │
│  │  │ (ExternRef)   │  │  7 functions │  │   allocations)  │  │  │
│  │  └──────────────┘  └──────────────┘  └─────────────────┘  │  │
│  └───────────────────────────────────────────────────────────┘  │
└───────────────────────────┬─────────────────────────────────────┘
                            │ WASM linear memory + FFI imports
                            │ (7 extern "C" functions)
┌───────────────────────────┴─────────────────────────────────────┐
│                     WASM Module (compiled Rust)                  │
│  ┌────────────┐  ┌─────────────┐  ┌──────────────────────────┐  │
│  │  js crate  │  │  web crate  │  │  User code (#[web::main])│  │
│  │  (FFI +    │  │  (DOM,      │  │  ┌────────────────────┐  │  │
│  │  binary    │  │   Canvas,   │  │  │ web_macro:         │  │  │
│  │  protocol) │  │   Fetch,    │  │  │ fn main() →        │  │  │
│  │            │  │   History,  │  │  │ #[no_mangle]       │  │  │
│  │            │  │   GPU…)     │  │  │ pub fn main()      │  │  │
│  │            │  │             │  │  └────────────────────┘  │  │
│  │ ┌────────┐ │  │ ┌────────┐ │  │ ┌────────────────────┐  │  │
│  │ │invoke  │◄├──┤│js!()   │◄├──┤ │ Executor (async    │  │  │
│  │ │fns     │ │  │ │macros  │ │  │ │  poll loop +       │  │  │
│  │ │return  │◄├──┤│        │◄├──┤ │  woke wakers)       │  │  │
│  │ │values) │ │  │ │        │ │  │ └────────────────────┘  │  │
│  │ └────────┘ │  │ └────────┘ │  │                          │  │
│  └────────────┘  └─────────────┘  └──────────────────────────┘  │
│  ┌───────────────────────────────┐                               │
│  │  ALLOCATIONS: Mutex<Vec<Option<Vec<u8>>>>                      │
│  │  (string/array return buffers, indexed by allocation_id)      │
│  └───────────────────────────────┘                               │
└───────────────────────────────────────────────────────────────────┘
```

The system has **four distinct layers**:

| Layer | Crate / File | Role |
|-------|-------------|------|
| **JS Runtime** | `js-wasm.js` (TypeScript → Parcel) | Browser-side WASM loader, memory manager, FFI host |
| **FFI Bridge** | `js` crate (`#![no_std]`) | Binary protocol for calling JS from Rust and back |
| **Web API** | `web` crate | Typed wrappers over DOM, Canvas, Fetch, GPU, etc. |
| **Codegen** | `web_macro` + `web_gen` | `#[web::main]` macro + `.d.ts` → `.rs` generator |

---

## 2. The JS Bridge Layer (`js` crate)

**File**: `crates/js/src/lib.rs`
**Constraint**: `#![no_std]` with `extern crate alloc`

### 2.1 ExternRef — The Handle Type

All JavaScript objects that cross the boundary are represented as `ExternRef`:

```rust
pub struct ExternRef { pub value: i64 }
```

This is a **64-bit integer handle** that maps to a JS object stored in the JS-side `GenerationalArena`. The arena packs a 32-bit index and a 32-bit generation counter into a single `BigInt`, providing safe handle reuse with use-after-free detection.

**Reserved handles** (indices 0-4, never deallocated):

| Handle | Value | JS Object |
|--------|-------|-----------|
| `JS_UNDEFINED` | 0 | `undefined` |
| `JS_NULL` | 1 | `null` |
| `DOM_SELF` / `DOM_WINDOW` | 2 | `self` / `window` |
| `DOM_DOCUMENT` | 3 | `document` |
| `DOM_BODY` | 4 | `document.body` |

### 2.2 The 7 WASM Import Functions

The Rust side declares these `extern "C"` imports, fulfilled by `js-wasm.js`:

```rust
extern "C" {
    fn js_register_function(start: f64, len: f64) -> f64;
    fn js_invoke_function(fn_handle: f64, parameters_start: *const u8, parameters_length: usize) -> f64;
    fn js_invoke_function_and_return_object(...) -> i64;    // ExternRef handle
    fn js_invoke_function_and_return_bigint(...) -> i64;    // BigInt
    fn js_invoke_function_and_return_string(...) -> usize;  // allocation_id
    fn js_invoke_function_and_return_array_buffer(...) -> usize; // allocation_id
    fn js_invoke_function_and_return_bool(...) -> f64;      // 0.0 or 1.0
}
```

Key design choice: **function handles are `f64`** (not `i64`) because WASM 1.0 only supports `i32`/`f64` as import/export types. The handle is a numeric index into `context.functions[]` on the JS side.

### 2.3 Binary Parameter Protocol

Parameters are serialized into a flat `Vec<u8>` with a **type-tagged binary wire format**:

| Tag | Type | Wire Encoding |
|-----|------|--------------|
| 0 | `Undefined` | 1 byte |
| 1 | `Null` | 1 byte |
| 2 | `Float64` | 1 byte tag + 8 bytes LE `f64` |
| 3 | `BigInt` | 1 byte tag + 8 bytes LE `i64` |
| 4 | `String` | 1 byte tag + 8 bytes LE `(*ptr, len)` (WASM memory addresses) |
| 5 | `ExternRef` | 1 byte tag + 8 bytes LE `i64` handle |
| 6 | `Float32Array` | 1 byte tag + 8 bytes LE `(*ptr, len)` |
| 7 | `Bool(true)` | 1 byte |
| 8 | `Bool(false)` | 1 byte |
| 9 | `Float64Array` | 1 byte tag + 8 bytes LE `(*ptr, len)` |
| 10 | `Uint32Array` | 1 byte tag + 8 bytes LE `(*ptr, len)` |

Strings and typed arrays are passed **by pointer into WASM linear memory** — the JS side reads directly from `memory.buffer` via `DataView`. This avoids copying but requires the memory to remain valid during the call.

### 2.4 The `js!` Macro

```rust
#[macro_export]
macro_rules! js {
    ($e:expr) => {{
        static mut FN: Option<f64> = None;
        unsafe {
            if FN.is_none() {
                FN = Some(js::register_function($e).fn_handle);
            }
            JSFunction { fn_handle: FN.unwrap() }
        }
    }};
}
```

This macro:
1. Takes a JavaScript function body as a `&str` literal
2. Registers it with the JS runtime **once** via a `static mut` cache
3. Returns a `JSFunction` wrapper with the cached handle

The JS function is constructed via `Function('"use strict"; return (' + body + ')')()` — a safe variant of `eval` that creates a named function. The `"use strict"` directive is injected automatically.

**Every call site in the entire `web` crate uses this pattern**:
```rust
pub fn console_log(message: &str) {
    let console_log = js!(r#"
        function(message){
            console.log(message);
        }"#);
    console_log.invoke(&[message.into()]);
}
```

### 2.5 Return Path: Allocations

For string and array buffer returns, the JS side writes into WASM-managed allocations:

```rust
static ALLOCATIONS: Mutex<Vec<Option<Vec<u8>>>> = Mutex::new(Vec::new());

#[no_mangle]
pub fn create_allocation(size: usize) -> usize { ... }
#[no_mangle]
pub fn allocation_ptr(allocation_id: i32) -> *const u8 { ... }
#[no_mangle]
pub fn allocation_len(allocation_id: i32) -> f64 { ... }
pub fn clear_allocation(allocation_id: usize) { ... }
```

The JS runtime calls `create_allocation`, gets back an index, writes bytes to `memory[allocation_ptr(id)..]`, then returns the index. Rust reads from the allocation via `extract_string_from_memory(allocation_id)`.

**Memory leak risk**: Allocations are never automatically freed. `clear_allocation` exists but is never called anywhere in the codebase. String returns from JS accumulate in the `ALLOCATIONS` vector indefinitely.

---

## 3. The JavaScript Runtime (`js-wasm.js`)

**Source**: `src/js-wasm.ts` → built with Parcel → `js-wasm.js`
**NPM package**: `js-wasm` v0.5.6

### 3.1 GenerationalArena

The core data structure managing JS-side object handles:

```typescript
class GenerationalArena {
    objects: any[];        // stored JS values
    generations: number[];  // generation counter per slot
    freeList: number[];    // reclaimed slot indices
    nextIndex: number;

    allocate(o): bigint     // packs (index | generation << 32)
    deallocate(handle): void // negates generation, adds to freeList
    retrieve(handle): any   // validates generation matches
}
```

The generation counter prevents use-after-free: if slot 5 held object A (gen=1), gets freed (gen=-1), then re-allocated for object B (gen=2), a stale handle with gen=1 will fail the generation check.

### 3.2 WASM Loading

```typescript
async load(wasmURL: string) {
    const [env, context] = JsWasm.createEnvironment();
    const response = await fetch(wasmURL);
    const bytes = await response.arrayBuffer();
    const module = await WebAssembly.instantiate(bytes, { env });
    context.module = module;
    return context;
}
```

The `env` object passed to `WebAssembly.instantiate` contains all 7 import functions. After instantiation, `context.module` is set so the imports can access WASM memory.

### 3.3 Auto-Discovery

On `DOMContentLoaded`, the runtime scans for `<script type="application/wasm" src="...">` tags and automatically loads/executes each one:

```typescript
document.addEventListener("DOMContentLoaded", function () {
    const wasmScripts = document.querySelectorAll("script[type='application/wasm']");
    for (let i = 0; i < wasmScripts.length; i++) {
        JsWasm.loadAndRunWasm(wasmScripts[i].src);
    }
});
```

### 3.4 Parameter Deserialization

The JS side mirrors the Rust binary protocol exactly. `readParameters(start, length)` reads from `memory.buffer` and reconstructs the typed parameter array. String parameters trigger a `readUtf8FromMemory` which uses `TextDecoder` on the WASM memory slice.

### 3.5 Function Registration

```typescript
js_register_function(start, len, utfByteLen) {
    let functionBody = utfByteLen === 16
        ? context.readUtf16FromMemory(start, len)
        : context.readUtf8FromMemory(start, len);
    const id = context.functions.length;
    context.functions.push(
        Function(`"use strict";return(${functionBody})`)()
    );
    return id;
}
```

Functions are compiled at registration time (not at each call), so the `js!` macro's `static` cache is actually redundant for performance — but it does save the FFI round-trip on first use.

---

## 4. The `#[web::main]` Proc Macro (`web_macro` crate)

**File**: `crates/web_macro/src/lib.rs`
**Dependencies**: `syn` 1.0 (full), `quote` 1.0

### 4.1 What It Does

```rust
#[proc_macro_attribute]
pub fn main(_args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::ItemFn);
    let name = input.sig.ident;
    let block = input.block;

    let expanded = quote! {
        #[no_mangle]
        pub fn #name() {
            executor::run(async move { #block })
        }
    };
    TokenStream::from(expanded)
}
```

**Transformation**:
```rust
// INPUT:
#[web::main]
async fn main() {
    console_log("hello");
    sleep(1000).await;
}

// OUTPUT:
#[no_mangle]
pub fn main() {
    executor::run(async move {
        console_log("hello");
        sleep(1000).await;
    })
}
```

### 4.2 Why This Exists

WASM modules compiled with `--target wasm32-unknown-unknown` have no runtime. The WASM module exports functions that the host (browser JS) calls. The standard convention is that the host calls an exported `main()` function to bootstrap the application.

The macro:
1. Strips `async` from the function signature (WASM can't export async functions)
2. Wraps the async body in `executor::run()` — the custom WASM async executor
3. Adds `#[no_mangle]` so the function is exported from the WASM module with the name `main`
4. The JS runtime (`js-wasm.js`) calls `module.instance.exports.main()` to start execution

### 4.3 Design Limitations

- **Only works for `fn main()`** — the macro ignores the original function signature entirely. If you use `#[web::main]` on a function with parameters or a return type, the parameters and return are silently discarded.
- **No error handling** — if the async block panics, there's no catch. The WASM module traps.
- **Tied to `web::executor`** — the macro hardcodes `executor::run`, so it only works when the `web` crate is in scope.
- **Uses syn 1.0** — outdated; syn 2.0 has been stable for years with better error reporting.

### 4.4 The `coroutine` Function

```rust
pub fn coroutine<T>(future: impl Future<Output = T> + 'static + Send + Sync)
where T: Send + Sync + 'static,
{
    let mut a = Some(Box::pin(future));
    set_timeout(move || {
        let b = a.take();
        if let Some(b) = b {
            DEFAULT_EXECUTOR.lock().run(b);
        }
    }, 0);
}
```

This spawns a **fire-and-forget concurrent task**. The `set_timeout(..., 0)` defers execution to the next JS event loop tick, preventing recursive executor invocation. The `Option::take()` pattern ensures the future is consumed exactly once.

---

## 5. The TypeScript Definition Parser & Rust Code Generator (`web_gen` crate)

**Directory**: `crates/web_gen/`
**Dependencies**: `nom` 7.1.2, `clap` 4 (derive)

This is the most architecturally interesting part of the project. It's a **CLI tool** that parses a custom TypeScript definition file format and generates Rust FFI bindings.

### 5.1 The Custom Type Definition Language

The parser (`typescript.rs`) defines its own DSL, not standard `.d.ts` syntax. It looks like this:

```
declare namespace WebGPU {
    interface GPUAdapter {
        requestDevice(): GPUDevice;
    }

    interface GPUDevice {
        createBuffer(size: number, usage: number, mappedAtCreation: boolean): GPUBuffer;
        createShaderModule(source: string): GPUShaderModule;
    }

    var navigator: Navigator;
}
```

The grammar supports:
- `declare namespace Name { ... }` — top-level grouping
- `interface Name { ... }` — type declarations with members
- `var name: type;` — global variable declarations
- `// comments` — line comments

**Types**: `void`, `string`, `number`, `boolean`, or any other identifier (treated as an interface reference).

### 5.2 AST Data Model

```rust
pub enum ValueType { Void, Interface(String), String, Number, Boolean }

pub struct Parameter { name: String, value_type: ValueType }
pub struct Function { name: String, parameters: Vec<Parameter>, return_type: ValueType }
pub struct Declaration { name: String, value_type: ValueType }
pub struct Interface { name: String, members: Vec<InterfaceMember> }
pub struct Namespace { name: String, parts: Vec<TypescriptDefinitionFilePart> }

pub enum InterfaceMember { Function(Function), Field(Declaration) }
pub enum TypescriptDefinitionFilePart { Comment, Interface, Declaration, NameSpace }
pub struct TypescriptDefinitionFile { parts: Vec<TypescriptDefinitionFilePart> }
```

### 5.3 Parsing with `nom`

The parser is a hand-written recursive descent parser using `nom` combinators:

```
parse_file
  └── many0(parse_part)
        ├── parse_interface     → "interface" ident "{" many0(parse_interface_member) "}"
        ├── parse_comment       → "//" take_while(!newline)
        ├── parse_declaration   → "var" ident ":" ident ";"
        └── parse_namespace     → "declare" "namespace" ident "{" many0(parse_part) "}"

parse_interface_member
  ├── parse_function  → ident "(" many0(parse_parameter) ")" ":" ident ";"
  └── parse_field     → ident ":" ident ";"

parse_parameter → ident ":" ident
```

**Notable**: The parser uses `alpha1` for identifiers, which means it only accepts alphabetic characters — no underscores, numbers, or hyphens in type/field names. This is a significant limitation for real-world APIs.

### 5.4 Code Generation

The `generation.rs` file takes the parsed AST and emits Rust source files:

```
For each Namespace:
  → create file: {namespace_name_snake_case}.rs
  → write: use js::*;
  → for each Declaration with Interface type:
    → for each InterfaceMember:
      ├── Field  → generate getter fn: pub fn field_name(this: &Interface) -> Type { js_unwrap!(this.field_name) }
      └── Function → generate stub fn: pub fn decl_fn_name(this: &Interface, params...) -> ReturnType { unimplemented!() }
```

**Type mapping**:

| TypeScript | Rust |
|-----------|------|
| `boolean` | `bool` |
| `number` | `f64` |
| `string` | `&str` (params) / `String` (returns) |
| Interface ref | `&InterfaceName` |

**Key observation**: Generated function bodies are `unimplemented!()` — this tool generates **stubs only**, not working implementations. The developer is expected to fill in the JS invocation logic. This is essentially a header-file generator that saves typing boilerplate signatures.

### 5.5 CLI Interface

```bash
web_gen <input_file> [--output-dir <dir>]
```

The input file is the custom `.d.ts`-like definition file. Output is one `.rs` file per namespace, written to `--output-dir` (default: current directory).

### 5.6 Why This Approach

The project authors recognized that manually writing FFI bindings for every Web API is tedious. Instead of the `wasm-bindgen` approach (Rust-side attributes + post-processing), they chose:

1. **Define the API surface in a TypeScript-like syntax** (familiar to web developers)
2. **Parse it into an AST** (nom-based parser)
3. **Generate Rust stubs** (one file per namespace)
4. **Fill in the implementations** by hand (using `js!` macros)

This is a **hybrid approach** — codegen for signatures, manual for implementations. It's less ambitious than `wasm-bindgen` (which generates both sides) but gives full control over the implementation.

### 5.7 Limitations of the Codegen

1. **No array support** — Array type generation is commented out in `generation.rs`
2. **`alpha1` identifiers** — No underscores, digits, or special chars in names
3. **No optional parameters** — All parameters are required
4. **No generics** — Interfaces can't be parameterized
5. **`unimplemented!()` bodies** — No actual JS invocation code generated
6. **No error handling** — `.unwrap()` everywhere in the generator
7. **Single type system** — Only 5 types (void, string, number, boolean, interface ref)
8. **Not standard `.d.ts`** — Custom DSL, can't consume existing TypeScript definitions

---

## 6. The Async Executor (`executor`)

**File**: `crates/web/src/executor.rs`
**Dependencies**: `woke` 0.0.4, `spin` 0.9.4

### 6.1 Architecture

Since WASM runs on the browser's main thread and there's no OS thread pool, the executor is a **single-threaded, cooperative scheduler** implemented as a deque of pending futures:

```rust
static DEFAULT_EXECUTOR: Mutex<Executor> = Mutex::new(Executor { tasks: None });

struct Executor { tasks: Option<TasksList> }  // VecDeque<Box<dyn Pendable + Send + Sync>>
```

### 6.2 Polling Strategy

```
executor.run(future)
  ├── add_task(future)     → push to back of VecDeque
  └── poll_tasks()
        └── for each task in deque:
              ├── task.is_pending() → poll the future
              │     ├── Poll::Ready → drop task (done)
              │     └── Poll::Pending → keep task
              └── if still pending → push back to deque
```

The `is_pending()` method calls `future.poll()` with a waker. The waker is provided by `woke`, which generates `Arc`-based wakers from a `Woke` implementation:

```rust
impl<T> Woke for Task<T> {
    fn wake_by_ref(_: &Arc<Self>) {
        set_timeout(|| { poll_tasks(); }, 0);
    }
}
```

When a future's waker is called (e.g., by a `sleep` callback or XHR onload), it schedules a `set_timeout(..., 0)` which defers `poll_tasks()` to the next JS event loop tick. This **prevents recursive polling** and avoids stack overflow.

### 6.3 Round-Robin Scheduling

The executor uses a **deque rotation** strategy: pop front, poll, if still pending push to back. This gives each task one poll per cycle — simple round-robin. No priority, no batching, no work-stealing.

### 6.4 The `coroutine` Function

```rust
pub fn coroutine<T>(future: impl Future<Output = T> + 'static + Send + Sync) {
    let mut a = Some(Box::pin(future));
    set_timeout(move || {
        let b = a.take();
        if let Some(b) = b { DEFAULT_EXECUTOR.lock().run(b); }
    }, 0);
}
```

This creates a **concurrent task** that starts on the next event loop tick. The `Option::take()` ensures the boxed future is moved exactly once. This is how you spawn background work alongside the main `#[web::main]` flow.

### 6.5 Limitations

- **No `JoinHandle`** — coroutines are fire-and-forget; you can't await their result
- **No cancellation** — once a task is in the deque, it runs until completion
- **Global mutex** — `DEFAULT_EXECUTOR` is behind a `Mutex`, so `poll_tasks()` blocks all other access
- **No backpressure** — if tasks wake each other faster than they complete, the deque grows unbounded

---

## 7. The `web` Crate — Web API Surface

**File**: `crates/web/src/lib.rs`

The `web` crate re-exports all submodules. It's the public API:

```rust
pub use web_macro::main;   // #[web::main]
pub use executor::coroutine;
pub use js::*;             // ExternRef, JSFunction, js!, allocations
```

### 7.1 Module Breakdown

| Module | What It Wraps | Key Functions |
|--------|--------------|---------------|
| `console` | `console.log/error/warn/time/timeEnd` | `console_log`, `console_error`, `console_warn`, `console_time`, `console_time_end` |
| `dom` | `document.querySelector`, element manipulation, events | `query_selector`, `element_set_inner_html`, `element_add_click_listener`, keyboard/mouse/change events |
| `canvas` | Canvas2DRenderingContext | `CanvasContext::from_element`, `fill_rect`, `stroke_rect`, `arc`, `draw_image`, etc. |
| `http_request` | `XMLHttpRequest` + `fetch()` wrapper | `XMLHttpRequest`, `fetch(FetchOptions)` → `impl Future<Output = FetchResponse>` |
| `local_storage` | `localStorage` | `local_storage_get/set/remove/clear` |
| `history` | `window.history` + `window.location` + `popstate` | `history_push_state`, `location_url`, `add_history_pop_state_event_listener` |
| `window` | `setTimeout`, `clearTimeout`, `requestAnimationFrame` | `set_timeout`, `clear_timeout`, `request_animation_frame` |
| `util` | Utilities | `random`, `sleep(ms)`, `wait_til_animation_frame()`, `create_object`, `create_array` |
| `web_gpu` | WebGPU API | `WebGPU::is_available`, `request_adapter`, `GPUDevice`, `GPURenderPipeline`, etc. |
| `web_component` | Custom Elements (incomplete) | `CustomElement` trait, `add_custom_component` |
| `common` | Shared patterns | `EventHandler`, `EventHandlerFuture`, `SharedStateMap` |

### 7.2 The `js!` Usage Pattern

Every single Web API wrapper follows the same pattern:

```rust
pub fn some_api(arg: Type) -> ReturnType {
    let func = js!(r#"
        function(arg){
            return browserAPI.doSomething(arg);
        }"#);
    func.invoke_and_return_Xxx(&[arg.into()])
}
```

This is **incredibly uniform**. The `js!` macro caches the function handle in a `static mut`, and each call serializes parameters through the binary protocol. There's no abstraction leakage — each function is a direct 1:1 mapping from Rust to JS.

### 7.3 Event Handling Architecture

Events use a **three-phase pattern**:

1. **JS Side**: Create a JS callback that calls `this.module.instance.exports.web_handle_<event>_handler(id, ...data)`
2. **Rust Side**: Store the callback handle + Rust closure in a `HashMap<i64, Box<dyn FnMut>>`
3. **Bridge**: The `#[no_mangle]` `web_handle_<event>_handler` function looks up the handler by ID and invokes it

```rust
// Phase 1+2: Register
pub fn element_add_click_listener(element: &ExternRef, handler: impl FnMut(MouseEvent) + Send + 'static)
    -> Arc<FunctionHandle>
{
    let function_ref = js!(r#"
        function(element){
            const handler = (e) => {
                this.module.instance.exports.web_handle_mouse_event_handler(id, e.offsetX, e.offsetY);
            };
            const id = this.storeObject(handler);
            element.addEventListener("click", handler);
            return id;
        }"#)
    .invoke_and_return_bigint(&[element.into()]);

    let function_handle = Arc::new(FunctionHandle(ExternRef { value: function_ref }));
    MOUSE_EVENT_HANDLER.add_listener(function_handle.clone(), Box::new(handler));
    function_handle
}

// Phase 3: Callback from JS
#[no_mangle]
pub extern "C" fn web_handle_mouse_event_handler(id: i64, x: f64, y: f64) {
    MOUSE_EVENT_HANDLER.call(id, MouseEvent { offset_x: x, offset_y: y });
}
```

The `id` comes from `this.storeObject(handler)` on the JS side — it's the `ExternRef` handle for the JS callback, reused as the lookup key on the Rust side. This is clever: **one handle serves double duty as both the JS callback reference and the Rust map key**.

### 7.4 Async API Pattern (Fetch, Sleep)

For async operations, the pattern uses `EventHandlerFuture`:

```rust
pub fn sleep(ms: impl Into<f64>) -> impl Future<Output = ()> {
    let sleep = js!(r#"
        function(ms, state_id){
            window.setTimeout(()=>{
                this.module.instance.exports.web_handle_empty_callback(state_id);
            }, ms);
        }"#);
    let (future, state_id) = EventHandlerFuture::<()>::create_future_with_state_id();
    sleep.invoke(&[ms.into(), state_id.into()]);
    future
}
```

`EventHandlerFuture` holds an `Arc<Mutex<EventHandlerSharedState>>` with `completed`, `waker`, and `result` fields. When the JS callback fires `web_handle_empty_callback(state_id)`, it calls `wake_future_with_state_id` which:
1. Sets `completed = true` and stores the result
2. Takes the `Waker` and calls `waker.wake()`
3. This triggers the executor's `set_timeout(..., 0)` → `poll_tasks()` → the future returns `Poll::Ready`

---

## 8. Memory Management Across the Boundary

### 8.1 Three Memory Systems

| System | Location | Purpose |
|--------|----------|---------|
| **WASM Linear Memory** | WASM heap | Rust `Vec`, `String`, all allocations via Rust's allocator |
| **ALLOCATIONS Vector** | Rust `Mutex<Vec<Option<Vec<u8>>>>` | Buffers for string/array returns from JS |
| **GenerationalArena** | JS heap | All JS object references (ExternRef handles) |

### 8.2 String Passing

**Rust → JS**: String pointers passed as `(start: usize, len: usize)` in the binary protocol. JS reads directly from WASM memory via `TextDecoder.decode(memory.subarray(start, start+len))`.

**JS → Rust**: JS encodes the string via `TextEncoder`, calls `create_allocation(len)` to get an allocation index, writes bytes to `memory[allocation_ptr(id)..]`, returns the allocation index. Rust reads via `extract_string_from_memory(id)` which clones the bytes from `ALLOCATIONS[id]`.

### 8.3 The ALLOCATIONS Leak

```rust
static ALLOCATIONS: Mutex<Vec<Option<Vec<u8>>>> = Mutex::new(Vec::new());

pub fn create_allocation(size: usize) -> usize {
    let mut buf = Vec::with_capacity(size);
    buf.resize(size, 0);
    let mut allocations = ALLOCATIONS.lock();
    let i = allocations.len();
    allocations.push(Some(buf));
    i
}

pub fn clear_allocation(allocation_id: usize) {
    let mut allocations = ALLOCATIONS.lock();
    allocations[allocation_id] = None;
}
```

`create_allocation` grows unboundedly. `clear_allocation` exists but is **never called**. Every string returned from JS (fetch responses, localStorage reads, DOM property reads) permanently occupies memory. For long-running WASM apps, this is a memory leak.

### 8.4 ExternRef Lifecycle

- `storeObject(obj)` → allocates in GenerationalArena, returns `BigInt` handle
- `releaseObject(handle)` → deallocates (skips handles 0-4)
- The JS side calls `releaseObject(id)` after one-shot callbacks (setTimeout, rAF)
- But persistent references (event listeners) are never released — they live as long as the DOM element exists

---

## 9. Event Handling Pattern

### 9.1 Three Event Handler Implementations

The codebase has **three different event handler patterns**, showing evolution over time:

**Pattern 1: Global HashMap (oldest)** — used in `dom.rs` for change/keyboard events:
```rust
static CHANGE_EVENT_HANDLERS: Mutex<Option<HashMap<Arc<FunctionHandle>, Box<dyn FnMut(ChangeEvent)>>>> = Mutex::new(None);
```

**Pattern 2: Static EventHandler struct (middle)** — used in `dom.rs` for mouse events:
```rust
static MOUSE_EVENT_HANDLER: EventHandler<MouseEvent> = EventHandler { listeners: Mutex::new(None) };
```

**Pattern 3: SharedStateMap + EventHandlerFuture (newest)** — used in `common.rs` and `util.rs`:
```rust
impl<T> EventHandlerFuture<T> {
    pub fn create_future_with_state_id() -> (Self, StateId);
    pub fn wake_future_with_state_id(id: StateId, result: T);
}
```

Pattern 3 is the most flexible — it supports async/await naturally. Patterns 1 and 2 require callback-based handling.

### 9.2 The FunctionHandle

```rust
pub struct FunctionHandle(pub ExternRef);

impl Hash for FunctionHandle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.value.hash(state);
    }
}
```

A wrapper around `ExternRef` that implements `Hash` and `Eq`, making it usable as a `HashMap` key. The JS callback handle becomes the key for looking up the Rust closure.

---

## 10. Critical Assessment — Strengths, Weaknesses, Lessons

### 10.1 Strengths

1. **Zero dependencies on wasm-bindgen** — The entire system is self-contained. No `wasm-bindgen`, `web-sys`, or `js-sys`. This means full control over the FFI boundary and no dependency on the wasm-bindgen CLI tool.

2. **Elegant handle system** — The `GenerationalArena` with packed index+generation is a well-known technique (used by `slotmap`, `generational-arena` crates). Packing it into a single `BigInt` for WASM FFI is clever.

3. **Simple async executor** — For a WASM single-threaded environment, the deque-based round-robin executor with `woke` wakers is minimal and correct. No thread pools, no complex state machines.

4. **Uniform API pattern** — Every Web API function follows the exact same `js!(...)` pattern. This makes the codebase extremely readable and easy to extend.

5. **Binary protocol is efficient** — Passing strings by pointer into WASM memory avoids copying. The type-tagged wire format is compact.

### 10.2 Weaknesses

1. **Memory leaks** — `ALLOCATIONS` vector never shrinks. String returns accumulate forever. This is a critical bug for production use.

2. **`static mut` in `js!` macro** — While sound in practice (each `js!` invocation creates its own `static`), `static mut` is undefined behavior in Rust if accessed from multiple threads. WASM is single-threaded, but this is still technically UB and fails newer Rust clippy lints.

3. **No typed errors** — All JS errors become WASM traps. There's no `Result` type wrapping. If a JS function throws, the WASM module aborts.

4. **Parser limitations** — The `web_gen` parser's `alpha1` restriction means it can't handle real-world API names like `getBoundingClientRect` (wait, that's alpha-only... actually it can since it's all alphabetic). But it can't handle `data-*` attributes, `aria-*` attributes, or any names with hyphens/underscores.

5. **No WebIDL/wasm-bindgen compatibility** — This system is entirely custom. It can't interoperate with crates that use `wasm-bindgen`.

6. **`unimplemented!()` in codegen** — The `web_gen` generates stub bodies. This means the codegen only saves typing signatures, not implementations. The ROI is low.

7. **No type safety for JS calls** — The `js!` macro accepts any string. There's no compile-time verification that the JavaScript is valid or that parameter types match.

8. **EventHandlerFuture not `Sync`** — The `common.rs` uses `std::sync::Mutex` and `std::collections::HashMap`, which require `std`. But the `js` crate is `#![no_std]`. This inconsistency means the `web` crate can't truly be `no_std`.

### 10.3 Lessons for Our Project (foundation-wasm-ui)

1. **Don't reinvent the FFI bridge** — Use `wasm-bindgen` + `web-sys`. The `js-wasm` approach is educational but production-ready tooling already exists and handles edge cases this doesn't (error handling, memory management, type safety).

2. **Memory management across WASM boundary is hard** — The `ALLOCATIONS` leak shows how easy it is to get wrong. If we build custom FFI, we need a proper allocation lifecycle (arena, reference counting, or explicit free).

3. **GenerationalArena is the right pattern** — For tracking JS objects from Rust (or vice versa), generational handles are the correct approach. The packed BigInt is a nice touch.

4. **Event bridge pattern is reusable** — The three-phase pattern (JS callback → WASM export → Rust dispatch) is how all WASM event handling works. The `EventHandlerFuture` with `SharedStateMap` is a good model for async event handling.

5. **Codegen from TypeScript definitions has value** — Even if `web_gen` is incomplete, the idea of generating Rust bindings from TypeScript type definitions is sound. We could do this better by parsing actual `.d.ts` files (with `typescript-parser` or similar) and generating full `wasm-bindgen` bindings.

6. **Simple executor works for WASM** — The deque-based executor is sufficient for single-threaded WASM. No need for complex async runtimes. The key insight is using `set_timeout(0)` to break recursion.

7. **The `#[web::main]` pattern is worth copying** — Transforming `async fn main()` into an executor-launched entry point is ergonomic. We should do something similar.

8. **Binary protocol for FFI parameters is efficient** — If we ever need custom FFI beyond `wasm-bindgen`, the type-tagged binary wire format used here is a good starting point.

---

## Appendix A: Complete File Map

```
crates/
├── js/                          # FFI bridge (#![no_std])
│   └── src/lib.rs               #   ExternRef, JSFunction, js! macro, allocations
├── web/                         # Web API wrappers
│   ├── src/lib.rs               #   Re-exports
│   ├── src/common.rs            #   EventHandler, EventHandlerFuture, SharedStateMap
│   ├── src/util.rs              #   random, sleep, wait_til_animation_frame, create_object/array
│   ├── src/console.rs           #   console.log/error/warn/time/timeEnd
│   ├── src/canvas.rs            #   Canvas2DRenderingContext (full API)
│   ├── src/dom/mod.rs           #   querySelector, element ops, mouse/keyboard/change events
│   ├── src/http_request.rs      #   XMLHttpRequest, fetch()
│   ├── src/local_storage.rs     #   localStorage get/set/remove/clear
│   ├── src/history.rs           #   history API, location API, popstate events
│   ├── src/window.rs            #   setTimeout, requestAnimationFrame
│   ├── src/web_gpu/mod.rs       #   WebGPU (partial: adapter, device, pipeline, buffer, render pass)
│   ├── src/web_component.rs     #   Custom Elements (incomplete trait + stub)
│   └── src/executor.rs          #   Async executor (deque round-robin + woke wakers)
├── web_macro/                   # Proc macro crate
│   └── src/lib.rs               #   #[web::main] attribute macro
└── web_gen/                     # Code generation CLI
    ├── src/main.rs              #   CLI entry (clap)
    ├── src/typescript.rs        #   nom parser for custom .d.ts-like DSL
    └── src/generation.rs        #   AST → Rust source code emitter

src/js-wasm.ts                   # TypeScript source for JS runtime
js-wasm.js                       # Compiled JS runtime (Parcel output)
js-wasm.d.ts                     # TypeScript declarations (empty)

examples/
├── helloworld/                  # console_log("Hello, world!")
├── web_snake/                   # Full Snake game with ECS (hecs)
├── web_fetch/                   # async fetch + ArrayBuffer
├── web_gpu/                     # WebGPU triangle
├── web_mouse_keyboard/          # Event handling
├── web_router/                  # SPA routing
├── web_async/                   # async/await demo
├── canvas/                      # Canvas drawing
├── fetch/                       # Fetch example
└── timer/                       # setTimeout example
```

## Appendix B: WASM Import/Export Contract

**WASM Imports** (provided by JS runtime):
```
env: {
  abort: () => never
  externref_drop: (obj: i64) => void
  js_register_function: (start: i32, len: i32, utfByteLen: i32) => i32
  js_invoke_function: (fnHandle: i32, paramsStart: i32, paramsLen: i32) => f64
  js_invoke_function_and_return_object: (...) => i64
  js_invoke_function_and_return_bigint: (...) => i64
  js_invoke_function_and_return_string: (...) => i32
  js_invoke_function_and_return_array_buffer: (...) => i32
  js_invoke_function_and_return_bool: (...) => f64
}
```

**WASM Exports** (used by JS runtime):
```
main: () => void
create_allocation: (size: i32) => i32
allocation_ptr: (id: i32) => i32
allocation_len: (id: i32) => i32
web_handle_empty_callback: (id: i64) => void
web_handle_mouse_event_handler: (id: i64, x: f64, y: f64) => void
web_handle_keyboard_event_handler: (id: i64, keyCode: f64) => void
web_handle_change_event: (id: i64, allocId: i32) => void
web_handle_http_load_event_handler: (id: i64) => void
web_handle_history_pop_state_event: (id: i64) => void
web_extern_ref_callback: (id: i64, value: i64) => void
```
