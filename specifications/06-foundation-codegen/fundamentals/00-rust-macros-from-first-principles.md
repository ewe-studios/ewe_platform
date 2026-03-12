# Rust Macros from First Principles

## What This Document Covers

This document teaches you everything about Rust's macro system, from the absolute fundamentals to the advanced techniques needed to build source-scanning code generation tools. By the end, you will understand:

1. What macros are and why Rust has two kinds
2. How procedural macros work internally
3. Why proc macros cannot see each other's invocations
4. Every known technique for collecting macro-annotated items
5. How to build a source scanner that replaces linker tricks
6. How LLVM tree shaking enables per-module WASM binaries

---

## Chapter 1: What Are Macros?

### The Fundamental Problem

Programming often involves writing repetitive code. Consider implementing `Display` for 20 structs, or generating API endpoint handlers for every route. You could write each one by hand, but this is:

- Error-prone (copy-paste mistakes)
- Tedious (same pattern, different names)
- Hard to maintain (change the pattern, change 20 files)

Macros solve this by **generating code at compile time**. You write the pattern once, and the compiler expands it everywhere it's used.

### Rust's Two Macro Systems

Rust has two fundamentally different macro systems:

#### 1. Declarative Macros (`macro_rules!`)

These are pattern-matching macros. You define patterns and what they expand to:

```rust
macro_rules! vec {
    () => { Vec::new() };
    ($($elem:expr),+ $(,)?) => {{
        let mut v = Vec::new();
        $(v.push($elem);)+
        v
    }};
}

// Usage:
let v = vec![1, 2, 3];

// Expands to:
let v = {
    let mut v = Vec::new();
    v.push(1);
    v.push(2);
    v.push(3);
    v
};
```

**Key properties:**
- Defined inline in the same crate
- Pattern matching on token trees
- Hygienic (generated identifiers don't leak)
- Cannot inspect types, only syntactic patterns
- Limited to what pattern matching can express

#### 2. Procedural Macros (Proc Macros)

These are **Rust functions** that take code as input and produce code as output. They run as a compiler plugin during compilation:

```rust
// This is a proc macro (lives in its own crate)
#[proc_macro_attribute]
pub fn module(attr: TokenStream, item: TokenStream) -> TokenStream {
    // attr = the arguments: module="some_module"
    // item = the struct/fn/etc being annotated

    // Parse, transform, return new code
    let input = parse_macro_input!(item as DeriveInput);
    // ... generate code ...
    output.into()
}
```

**Key properties:**
- Must live in a separate crate (`proc-macro = true` in Cargo.toml)
- Are arbitrary Rust functions — can do anything
- Take `TokenStream` → return `TokenStream`
- Three kinds: derive macros, attribute macros, function-like macros
- Can use any Rust library (file I/O, networking, etc.)

### Why Two Systems?

Declarative macros are simple and fast but limited. Proc macros are powerful but:
- Require a separate crate (compilation unit boundary)
- Are slower (they're compiled and executed during compilation)
- Are harder to write correctly

**Rule of thumb:** Use `macro_rules!` for simple pattern expansion. Use proc macros when you need to inspect, analyze, or transform code structures.

---

## Chapter 2: Procedural Macros in Detail

### The Three Kinds of Proc Macros

#### Derive Macros

Added to structs/enums with `#[derive(MyMacro)]`. They can only **add** new code — they cannot modify the original item.

```rust
// Definition (in proc-macro crate):
#[proc_macro_derive(MyTrait)]
pub fn my_trait_derive(input: TokenStream) -> TokenStream {
    // input = the struct definition
    // output = new impl blocks (NOT the struct itself)
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    quote! {
        impl MyTrait for #name {
            fn method(&self) -> &str { stringify!(#name) }
        }
    }.into()
}

// Usage:
#[derive(MyTrait)]
struct Foo { x: i32 }
// The struct is unchanged; a new impl block is added
```

#### Attribute Macros

Applied as `#[my_macro(...)]`. They receive **both** the attribute arguments and the annotated item, and they **replace** the item with their output.

```rust
// Definition:
#[proc_macro_attribute]
pub fn module(attr: TokenStream, item: TokenStream) -> TokenStream {
    // attr = arguments inside the parentheses
    // item = the entire annotated item
    // IMPORTANT: you must re-emit the original item if you want to keep it!

    let args = parse_macro_input!(attr as ModuleArgs);
    let input = parse_macro_input!(item as DeriveInput);

    quote! {
        #input  // re-emit original struct
        // ... additional generated code ...
    }.into()
}

// Usage:
#[module(module = "auth", export = true)]
struct AuthHandler { ... }
```

**Critical detail:** Attribute macros *replace* their input. If you don't re-emit the original struct in your output, it disappears. This is different from derive macros, which only add code.

#### Function-like Macros

Called like functions with `!`. They take arbitrary tokens and produce arbitrary output.

```rust
// Definition:
#[proc_macro]
pub fn sql(input: TokenStream) -> TokenStream {
    // input = everything inside sql!(...)
    // Can be completely custom syntax
    // ...
}

// Usage:
let query = sql!(SELECT * FROM users WHERE id = 42);
```

### The TokenStream Pipeline

Understanding `TokenStream` is essential. Here's what happens when the compiler encounters a proc macro:

```
Source Code
    │
    ▼
Lexer (tokenization)
    │  Converts text into tokens:
    │  "struct Foo { x: i32 }" → [struct, Foo, {, x, :, i32, }]
    │
    ▼
TokenStream
    │  A flat sequence of TokenTree values:
    │  - Ident("struct"), Ident("Foo"), Group(Brace, [...])
    │
    ▼
Proc Macro Function
    │  Your Rust code receives this TokenStream
    │  You parse it (usually with `syn`)
    │  You generate output (usually with `quote`)
    │
    ▼
Output TokenStream
    │
    ▼
Compiler continues with the expanded code
```

A `TokenStream` is a sequence of `TokenTree` values:

```rust
pub enum TokenTree {
    Group(Group),      // Delimited group: { ... }, ( ... ), [ ... ]
    Ident(Ident),      // Identifier: struct, foo, i32
    Punct(Punct),      // Punctuation: +, -, ::, =>
    Literal(Literal),  // Literal: 42, "hello", 3.14
}
```

### The `syn` Crate: Parsing TokenStreams

Parsing raw `TokenTree` values is painful. The `syn` crate provides a full Rust syntax parser:

```rust
use syn::{parse_macro_input, DeriveInput, ItemStruct, Attribute};

// Parse a derive macro input (struct/enum/union):
let input = parse_macro_input!(tokens as DeriveInput);

// Now you have structured access:
input.ident          // The type name (e.g., "Foo")
input.generics       // Generic parameters
input.data           // Struct fields, enum variants, etc.
input.attrs          // All attributes on the item
```

`syn` can parse the entire Rust grammar. Key types:

| Type | Parses | Example |
|------|--------|---------|
| `DeriveInput` | struct/enum/union | `struct Foo { x: i32 }` |
| `ItemStruct` | struct specifically | `struct Foo { x: i32 }` |
| `ItemFn` | functions | `fn foo() -> i32 { 42 }` |
| `ItemImpl` | impl blocks | `impl Foo { ... }` |
| `ItemTrait` | trait definitions | `trait Bar { ... }` |
| `File` | entire Rust file | Everything in a `.rs` file |
| `Attribute` | attributes | `#[module(name = "auth")]` |

### The `quote` Crate: Generating TokenStreams

`quote` provides a macro for generating `TokenStream` values using Rust-like syntax:

```rust
use quote::quote;

let name = &input.ident;
let field_count = fields.len();

let expanded = quote! {
    impl #name {
        pub fn field_count() -> usize {
            #field_count
        }
    }
};
```

The `#variable` syntax interpolates Rust values into the generated code. Iterators work with `#(...)*`:

```rust
let field_names = fields.iter().map(|f| &f.ident);
let field_types = fields.iter().map(|f| &f.ty);

let expanded = quote! {
    impl #name {
        pub fn describe() -> Vec<(&'static str, &'static str)> {
            vec![
                #((stringify!(#field_names), stringify!(#field_types))),*
            ]
        }
    }
};
```

### Parsing Custom Attributes

For `#[module(module = "auth", export = true)]`, you need to parse the attribute arguments:

```rust
use syn::parse::{Parse, ParseStream};
use syn::{LitStr, LitBool, Token, Ident};

struct ModuleArgs {
    module: String,
    export: bool,
}

impl Parse for ModuleArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut module = String::new();
        let mut export = false;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            match key.to_string().as_str() {
                "module" => {
                    let value: LitStr = input.parse()?;
                    module = value.value();
                }
                "export" => {
                    let value: LitBool = input.parse()?;
                    export = value.value();
                }
                _ => return Err(syn::Error::new(key.span(), "unknown attribute")),
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(ModuleArgs { module, export })
    }
}
```

---

## Chapter 3: The Isolation Problem

### Why Proc Macros Can't See Each Other

This is the most important concept to understand. When the compiler encounters:

```rust
#[module(module = "auth")]
struct AuthHandler { ... }

#[module(module = "auth")]
struct AuthMiddleware { ... }

#[module(module = "api")]
struct ApiRouter { ... }
```

Each `#[module]` invocation is a **separate, independent function call**:

```
Call 1: module(attr="module=\"auth\"", item="struct AuthHandler { ... }")
Call 2: module(attr="module=\"auth\"", item="struct AuthMiddleware { ... }")
Call 3: module(attr="module=\"api\"", item="struct ApiRouter { ... }")
```

There is:
- **No shared state** between calls (each is a fresh function invocation)
- **No guaranteed ordering** (the compiler may expand them in any order)
- **No "all done" signal** (no callback after all macros are expanded)
- **No access to other files** (you only see the single annotated item)

This is by design. Proc macros are pure functions: `TokenStream -> TokenStream`. This makes them:
- Deterministic (same input → same output)
- Parallelizable (no shared mutable state)
- Cacheable (incremental compilation)

### Why Can't We Use Static Variables?

You might think: "proc macros are just Rust code, so use a `static Mutex<Vec<...>>` to collect items."

This **does not work reliably** because:

1. **Incremental compilation**: If only one file changes, only macros in that file re-run. The static is fresh each compilation, so you'd lose previously collected items.

2. **Separate compilation units**: Proc macros run in the compiler's process, but each invocation may be in a separate context. The compiler doesn't guarantee state persistence.

3. **Parallel compilation**: Multiple files may compile simultaneously in different threads or even processes. A mutex doesn't help across processes.

4. **No finalization hook**: Even if collection worked, there's no way to emit the "collected" code after all macros finish.

### The Build Order Problem

Consider this dependency chain:

```
my_macros (proc-macro crate)
    ↓ used by
my_lib (library crate)
    ↓ used by
my_app (binary crate)
```

The proc macro crate is compiled **first**, before any of its users. When `my_lib` compiles, the macro runs on each annotated item. But the macro code can't "look ahead" to see all the items in `my_lib` — it processes them one at a time as the compiler encounters them.

---

## Chapter 4: Collection Strategies

Given that proc macros can't collect items, here are all known strategies:

### Strategy 1: Linker-Based Collection (Native Only)

**How it works:** Each macro invocation emits code that places a static value into a special linker section. At runtime, the linker has grouped all these values together.

**Crates:** `inventory`, `linkme`

```rust
// inventory approach:
inventory::collect!(Registration);

// Each macro emits:
inventory::submit! { Registration { name: "Foo" } }

// At runtime:
for item in inventory::iter::<Registration> { ... }
```

**Pros:**
- Zero-effort collection — just annotate and go
- No central list to maintain
- Works across crate boundaries

**Cons:**
- **Does NOT work on WASM** (no linker sections, no `.init_array`)
- Platform-specific (Linux, macOS, Windows have different section names)
- Runtime overhead (linked list traversal or section scanning)
- Depends on `ctor` crate for constructor functions

**Under the hood (inventory):**

```
Compile time:
    Each submit!() generates a function in .init_array linker section

Load time (before main):
    OS loader calls all .init_array functions
    Each function pushes its Registration onto a global linked list:
    HEAD → Registration3 → Registration2 → Registration1 → NULL

Runtime:
    inventory::iter() walks the linked list
```

**Under the hood (linkme):**

```
Compile time:
    Each #[distributed_slice(ITEMS)] generates a static in a named section

Link time:
    Linker coalesces all statics with the same section name:
    Section "ITEMS": [Item1 | Item2 | Item3]

Runtime:
    ITEMS is a &[Item] pointing to the section start/end
    Direct slice access — no linked list, no constructors
```

### Strategy 2: Aggregator Macro (Manual List)

**How it works:** The user maintains a single macro call listing all types:

```rust
register_all! {
    auth: [AuthHandler, AuthMiddleware],
    api: [ApiRouter, ApiResponse],
    db: [DbPool, DbMigration],
}
```

The macro expands this into whatever registration code is needed.

**Pros:**
- Works everywhere (WASM, no_std, any platform)
- Simple to implement
- Compile-time checked (typo = compilation error)
- No runtime overhead

**Cons:**
- User must manually maintain the list
- Easy to forget adding new items
- Doesn't scale well (large list = large macro invocation)

### Strategy 3: Build Script Source Scanning

**How it works:** A `build.rs` or standalone tool scans Rust source files using `syn`, finds all items annotated with your macro, and generates code based on the collected information.

```
build.rs runs BEFORE compilation:
    1. Walk all .rs files in src/
    2. Parse each file with syn::parse_file()
    3. Find all #[module(...)] attributes
    4. Extract module name, struct name, file path
    5. Generate code into OUT_DIR or src/bin/
```

**Pros:**
- Works everywhere (WASM, no_std, any platform)
- Full visibility of all annotated items
- Can generate arbitrary output (main.rs files, Cargo.toml, config)
- No runtime overhead
- Deterministic

**Cons:**
- Requires parsing source files (slower than just compiling)
- Must resolve module paths manually (the compiler hasn't run yet)
- `build.rs` can't add new `[[bin]]` targets (Cargo reads Cargo.toml first)
- Must be kept in sync with the actual crate module structure

### Strategy 4: Standalone Build Tool

**How it works:** A separate binary (or cargo subcommand) scans source, generates an entire crate with per-module binaries, then builds it.

```
cargo run -p my_builder:
    1. Scan all .rs files in the source crate
    2. Collect all #[module(...)] annotations
    3. Generate a new crate:
       - Cargo.toml with [[bin]] entries
       - src/bin/module_a.rs (imports only module_a types)
       - src/bin/module_b.rs (imports only module_b types)
    4. Run: cargo build --target wasm32-unknown-unknown
    5. Each binary → separate .wasm file
    6. LLVM tree-shakes unused code from each binary
```

**Pros:**
- Full control over the build process
- Can generate `[[bin]]` targets (unlike build.rs)
- Can run `wasm-opt` and post-processing
- Works everywhere
- Cleanest separation of concerns

**Cons:**
- Extra build step (not automatic with `cargo build`)
- Must be run manually or integrated into CI
- Generating a separate crate adds complexity

### Strategy 5: Hybrid (Proc Macro + Source Scanner)

**This is what we're building.** The architecture splits responsibilities:

```
#[module(...)] proc macro:
    - Per-item code generation only
    - Emits trait impls, exports, or markers
    - No collection, no shared state

Source scanner (foundation_codegen):
    - Parses all source files with syn
    - Finds all #[module(...)] annotations
    - Resolves module paths and crate structure
    - Exports a HashMap<TypeName, DerivedTarget>
    - Consumer code uses this map for any purpose:
      - Generate main.rs files for WASM binaries
      - Generate registration code
      - Generate documentation
      - Generate test harnesses
```

This is the approach we implement in this specification.

---

## Chapter 5: Source Scanning Deep Dive

### How syn::parse_file Works

`syn` can parse an entire Rust file into an AST without the compiler:

```rust
use syn::{parse_file, File, Item};

let source = std::fs::read_to_string("src/handlers.rs")?;
let ast: File = parse_file(&source)?;

// ast.items contains every top-level item:
for item in &ast.items {
    match item {
        Item::Struct(s) => println!("Found struct: {}", s.ident),
        Item::Fn(f) => println!("Found fn: {}", f.sig.ident),
        Item::Impl(i) => println!("Found impl block"),
        Item::Trait(t) => println!("Found trait: {}", t.ident),
        Item::Enum(e) => println!("Found enum: {}", e.ident),
        Item::Mod(m) => println!("Found mod: {:?}", m.ident),
        _ => {}
    }
}
```

### The Visitor Pattern

For deep scanning (finding items inside modules, impl blocks, etc.), use `syn::visit::Visit`:

```rust
use syn::visit::Visit;

struct MacroFinder {
    target_macro: String,
    found: Vec<FoundItem>,
}

impl<'ast> Visit<'ast> for MacroFinder {
    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        for attr in &node.attrs {
            if attr.path().is_ident(&self.target_macro) {
                // Found a struct with our target macro!
                self.found.push(FoundItem {
                    name: node.ident.to_string(),
                    // ... extract more info ...
                });
            }
        }
        // Continue visiting nested items
        syn::visit::visit_item_struct(self, node);
    }

    fn visit_item_enum(&mut self, node: &'ast syn::ItemEnum) {
        // Same pattern for enums
        for attr in &node.attrs {
            if attr.path().is_ident(&self.target_macro) {
                self.found.push(FoundItem {
                    name: node.ident.to_string(),
                    // ...
                });
            }
        }
        syn::visit::visit_item_enum(self, node);
    }

    fn visit_item_trait(&mut self, node: &'ast syn::ItemTrait) {
        // Same pattern for traits
        for attr in &node.attrs {
            if attr.path().is_ident(&self.target_macro) {
                self.found.push(FoundItem {
                    name: node.ident.to_string(),
                    // ...
                });
            }
        }
        syn::visit::visit_item_trait(self, node);
    }
}
```

### Module Path Resolution

The hardest part of source scanning is figuring out the **module path** for each item. Rust has two module systems:

#### File-based modules (modern, Rust 2018+):

```
src/
├── lib.rs           → crate root (my_crate)
├── handlers.rs      → my_crate::handlers
├── handlers/
│   ├── mod.rs       → my_crate::handlers (alternative to handlers.rs)
│   ├── auth.rs      → my_crate::handlers::auth
│   └── api.rs       → my_crate::handlers::api
├── models/
│   ├── mod.rs       → my_crate::models
│   └── user.rs      → my_crate::models::user
```

#### Inline modules:

```rust
// In lib.rs:
mod inline_module {
    pub struct Foo; // Path: my_crate::inline_module::Foo

    mod nested {
        pub struct Bar; // Path: my_crate::inline_module::nested::Bar
    }
}
```

#### The resolution algorithm:

```
1. Start with the crate name from Cargo.toml
2. For each .rs file:
   a. Determine its module path from its filesystem location:
      - src/lib.rs → crate_name
      - src/foo.rs → crate_name::foo
      - src/foo/mod.rs → crate_name::foo
      - src/foo/bar.rs → crate_name::foo::bar
   b. Parse the file
   c. Walk the AST
   d. For inline modules (mod name { ... }), push name onto the path
   e. For each annotated item, record: file_path + module_path + item_name
```

#### Path edge cases:

```rust
// #[path] attribute overrides the file path:
#[path = "custom_location.rs"]
mod my_module;  // Loads from custom_location.rs, not my_module.rs

// Re-exports change the public path:
pub use handlers::auth::AuthHandler;  // Now also at crate::AuthHandler

// cfg-gated modules may or may not exist:
#[cfg(feature = "auth")]
mod auth;  // Only exists if "auth" feature is enabled
```

For our scanner, we handle the common cases (file-based and inline modules) and document the edge cases that require manual intervention.

---

## Chapter 6: LLVM Tree Shaking for WASM

### How Dead Code Elimination Works

When you compile Rust to WASM (or any target), the compilation pipeline is:

```
Rust Source
    │
    ▼
rustc frontend (parsing, type checking, borrow checking)
    │
    ▼
MIR (Mid-level Intermediate Representation)
    │  Rust-specific optimizations
    │
    ▼
LLVM IR (Low-level Intermediate Representation)
    │  LLVM optimizations including:
    │  - Dead code elimination
    │  - Function inlining
    │  - Constant propagation
    │  - Dead argument elimination
    │
    ▼
Machine code / WASM bytecode
```

### What Gets Eliminated

LLVM performs **interprocedural dead code elimination (DCE)**. Starting from entry points (e.g., `main`, `#[no_mangle]` functions), it traces all reachable code and removes everything else:

```
Entry point: main()
    │
    ├── calls init_auth()
    │      ├── uses AuthHandler     ✅ KEPT
    │      └── uses AuthMiddleware  ✅ KEPT
    │
    └── calls init_api()
           └── uses ApiRouter      ✅ KEPT

NOT reachable from main():
    - DbPool                       ❌ REMOVED
    - DbMigration                  ❌ REMOVED
    - LoggingMiddleware            ❌ REMOVED
    - TestHelper                   ❌ REMOVED
```

### Why Separate Binaries Enable Better Tree Shaking

If you have ONE binary that uses everything, nothing gets removed. The trick is to create **separate binaries per module**:

```
auth_module binary (src/bin/auth.rs):
    use my_lib::handlers::AuthHandler;
    use my_lib::handlers::AuthMiddleware;

    fn main() {
        AuthHandler::init();
        AuthMiddleware::init();
    }

    LLVM output: auth.wasm
    Contains ONLY: AuthHandler, AuthMiddleware, and their dependencies
    Everything else: REMOVED

api_module binary (src/bin/api.rs):
    use my_lib::api::ApiRouter;

    fn main() {
        ApiRouter::init();
    }

    LLVM output: api.wasm
    Contains ONLY: ApiRouter and its dependencies
    Everything else: REMOVED
```

### Maximizing Tree Shaking

```toml
[profile.release]
lto = true          # Link-Time Optimization: analyze the ENTIRE program
opt-level = "z"     # Optimize for size (most aggressive DCE)
codegen-units = 1   # Single compilation unit (better cross-module DCE)
panic = "abort"     # Remove panic unwinding machinery
strip = true        # Remove symbol tables and debug info
```

**LTO is critical.** Without LTO, the compiler optimizes each crate independently. Functions that are only called from one place can't be inlined across crate boundaries. With LTO, the entire dependency graph is analyzed as one unit.

### The Full Pipeline for Per-Module WASM

```
Source Scanning (foundation_codegen)
    │
    │  Scans all .rs files
    │  Finds all #[module(module = "X")] annotations
    │  Groups by module name
    │
    ▼
Code Generation
    │
    │  For each module group:
    │    Generate src/bin/{module_name}.rs
    │    With only the imports for that module's types
    │
    ▼
Cargo Build
    │
    │  cargo build --release --target wasm32-unknown-unknown
    │  Each [[bin]] compiles independently
    │  LLVM tree-shakes unused code per binary
    │
    ▼
WASM Output
    │
    │  target/wasm32-unknown-unknown/release/
    │    auth.wasm      (small — only auth code)
    │    api.wasm       (small — only api code)
    │    db.wasm        (small — only db code)
    │
    ▼
Optional: wasm-opt
    │
    │  wasm-opt -Oz auth.wasm -o auth.wasm
    │  Further size reduction
    │
    ▼
Deployment-ready WASM files
```

---

## Chapter 7: The Registry Pattern Without Linker Tricks

### Architecture Overview

Our approach uses **source-time collection** instead of link-time or runtime collection:

```
┌──────────────────────────────────────────────────┐
│              Traditional (inventory/linkme)        │
│                                                    │
│  Compile → Link (collect in sections) → Run        │
│  Each macro emits a static → Linker groups them    │
│  ❌ Doesn't work on WASM                          │
└──────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────┐
│              Our Approach (foundation_codegen)     │
│                                                    │
│  Scan Source → Collect → Generate → Compile        │
│  Read .rs files → Parse with syn → Build registry  │
│  ✅ Works everywhere (WASM, no_std, embedded)     │
└──────────────────────────────────────────────────┘
```

### Step-by-Step Flow

```
Step 1: Developer annotates structs
─────────────────────────────────────
    #[module(module = "auth", export = true)]
    struct AuthHandler { ... }

    #[module(module = "auth")]
    struct AuthMiddleware { ... }

    #[module(module = "api")]
    struct ApiRouter { ... }


Step 2: foundation_codegen scans the crate
──────────────────────────────────────────
    let scanner = CrateScanner::new("path/to/my_lib");
    let registry = scanner.scan_for("module")?;

    // registry contains:
    // {
    //   "AuthHandler": DerivedTarget {
    //     macro_name: "module",
    //     attributes: { "module": "auth", "export": "true" },
    //     location: Location {
    //       file: "src/handlers/auth.rs",
    //       line: 15,
    //       column: 1,
    //     },
    //     module_path: "my_lib::handlers::auth",
    //     crate_name: "my_lib",
    //     crate_root: "/path/to/my_lib",
    //     cargo_toml: "/path/to/my_lib/Cargo.toml",
    //     item_kind: ItemKind::Struct,
    //   },
    //   "AuthMiddleware": DerivedTarget { ... },
    //   "ApiRouter": DerivedTarget { ... },
    // }


Step 3: Consumer code uses the registry
────────────────────────────────────────
    // Group by module
    let modules = registry.group_by_attribute("module");

    // Generate per-module main.rs files
    for (module_name, targets) in &modules {
        let main_rs = generate_main_rs(module_name, targets);
        write_file(format!("src/bin/{}.rs", module_name), main_rs);
    }

    // Or generate anything else:
    // - Test harnesses
    // - Documentation
    // - Configuration files
    // - RPC stubs
    // - WASM bindings
```

### Why This Is Better for WASM

| Aspect | inventory/linkme | foundation_codegen |
|--------|------------------|-------------------|
| WASM support | No | Yes |
| no_std support | Partial | Yes |
| Collection time | Runtime | Build time |
| Runtime overhead | Linked list / section scan | Zero |
| Cross-crate | Yes | Yes (scan multiple crates) |
| Deterministic | Platform-dependent | Yes |
| Custom output | Limited (just registration) | Anything (main.rs, docs, config) |

---

## Summary

You now understand:

1. **Rust macros**: Declarative (`macro_rules!`) for patterns, procedural for code transformation
2. **Proc macro isolation**: Each invocation is independent — no shared state, no collection
3. **The collection problem**: Proc macros can't see each other; you need an external mechanism
4. **Available strategies**: Linker tricks (native only), aggregator macros (manual), source scanning (universal)
5. **Source scanning**: Parse `.rs` files with `syn`, walk the AST with `Visit`, resolve module paths from filesystem layout
6. **LLVM tree shaking**: Separate binaries + LTO = minimal per-module WASM files
7. **The hybrid approach**: Simple proc macro for per-item codegen + source scanner for collection = works everywhere

The `foundation_codegen` crate implements the source scanning approach, providing a generic, reusable tool that any code generator can build upon.
