# Fundamentals 03 — LanguageConfig & the generic walker (and Rust's bespoke walker)

How one walker handles many languages — and why **Rust is hand-written**, not
config-driven. This is the heart of extraction.

---

## 1. The idea: a config of node-type names

Most of what differs between languages is *vocabulary*, not *logic*. "A class" is
`class_definition` in Python, `class_declaration` in JS, `struct_item`/`enum_item`/
`trait_item` in Rust. "A function" is `function_definition` / `function_item` /
`method_declaration`. The *walk* — "descend the tree; when you see a class, emit a
node + a `contains` edge; when you see a call, queue it for resolution" — is the
same.

So graphify factors the vocabulary into a **`LanguageConfig`** and runs one generic
walker over it:

```rust
pub struct LanguageConfig {
    pub grammar: tree_sitter::Language,
    pub class_types:    &'static [&'static str],  // {"struct_item","enum_item","trait_item"} (rs)
    pub function_types: &'static [&'static str],  // {"function_item"} (rs)
    pub import_types:   &'static [&'static str],  // {"use_declaration"} (rs)
    pub call_types:     &'static [&'static str],  // {"call_expression"} (rs)
    pub call_accessor_node_types: &'static [&'static str], // {"field_expression"} (rs)
    pub name_field:  &'static str,                // "name"
    pub body_field:  &'static str,                // "body"
    pub call_function_field: &'static str,        // "function"
    pub call_accessor_field: &'static str,        // "field"
    pub import_handler: Option<ImportHandlerFn>,        // per-language import edges
    pub resolve_function_name_fn: Option<NameResolverFn>, // C/C++ declarator unwrap
    pub extra_walk_fn: Option<ExtraWalkFn>,            // JS arrows, C# namespaces, …
}
```

The generic walker (`_extract_generic` in graphify) reads node `kind()`s, checks
them against these sets, and emits nodes/edges accordingly. Add a language by
adding a config — *for the languages the generic walker fits*.

## 2. Why Rust is bespoke (the important correction)

The "one walker for everything" story is **only ~half true**. In graphify ~12
languages use the generic walker; ~11 — **including Rust and Go, our priority
languages** — have **hand-written extractors** (`extract_rust`, `extract_go`).
Rust's structure doesn't fit the generic mold cleanly:

- **Three class-like node types** (`struct_item`, `enum_item`, `trait_item`) with
  different child shapes.
- **`impl` blocks** carry the real relationships: `impl Trait for Type` is an
  `implements` edge; `impl Type { … }` attaches methods to a type. The generic
  "class → contains function" model doesn't capture `impl`.
- **`use` paths** (`use foo::bar::Baz`) need Rust-specific resolution into import
  edges.
- **Method vs free-function calls**, macro invocations, associated functions —
  Rust-specific call shapes.

So F27a ships a **hand-written Rust walker** (`rust_walker::extract_rust_file`).
The generic `LanguageConfig` pattern above is what the **F27b follow-on** uses for
JS/TS + Python (which *do* fit the generic walker).

## 3. What the Rust walker emits

Walking a Rust file's tree, per node kind:

- `struct_item` / `enum_item` / `trait_item` → a node (`Struct`/`Enum`/`Trait`) +
  a `contains` EXTRACTED edge from the file node.
- `function_item` → a `Function` node + `contains`; its body is scanned for calls.
- `impl_item` →
  - `impl Trait for Type` → an `implements` edge (Type → Trait);
  - methods inside → `Method`/`Function` nodes attached to the type.
- `use_declaration` → an `imports` edge (file → imported path), Rust path
  resolution applied.
- `call_expression` → a call. **Bare** calls (`helper()`) resolve against the
  symbol index → `calls` edge. **Member** calls (`x.log()`) are flagged
  `is_member_call` and handled specially (Doc 04 §3).
- comments with rationale prefixes (`// WHY:`, `// NOTE:`, `// HACK:`, …) and
  doc-comments → a `rationale` **attribute** on the nearby node (Doc 04 §6).

## 4. Node kinds we model

```rust
enum NodeKind { File, Module, Struct, Enum, Trait, Function, Method,
                Impl, Constant, Static, TypeAlias, Macro }
```

These are the structural entities an agent asks about. Expression-level nodes
(loops, literals, blocks) are walked through but never become graph nodes — they'd
explode the graph without answering structural questions.

## 5. Relations we emit (F27a)

```rust
enum Relation { Contains, Imports, ImportsFrom, Inherits, Implements,
                Calls, Uses, Defines }
```

EXTRACTED from the tree: `contains`, `imports`, `implements`. INFERRED by
resolution: cross-file `calls` and `uses` (Doc 04). (graphify emits a longer list
including `instantiates`/`binds_method`/`references_constant`; F27a keeps the core
set that answers the agentic "which file / what calls" questions, and the model is
extensible.)

---

**Next:** Doc 04 — call-graph resolution (the subtle, false-positive-prone part).
