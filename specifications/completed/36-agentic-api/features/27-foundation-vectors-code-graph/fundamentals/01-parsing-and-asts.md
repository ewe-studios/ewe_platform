# Fundamentals 01 — Parsing & abstract syntax trees

How source text becomes a tree you can walk. If you've never written a parser,
start here; it makes Doc 02 (tree-sitter) and Doc 03 (the walker) obvious.

---

## 1. From characters to structure

Source code is a flat string. To reason about it structurally you turn it into a
tree in two classic steps:

1. **Lexing (tokenizing).** Group characters into *tokens*: `fn`, `helper`, `(`,
   `)`, `->`, `i32`, `{`, … A lexer knows that `i32` is one identifier token, not
   three characters to think about individually.
2. **Parsing.** Apply the language *grammar* to the token stream to produce a
   tree. The grammar says "a function item is `fn` then a name then params then a
   return type then a block" — the parser matches that and builds nodes.

## 2. Concrete vs abstract syntax trees

- A **concrete syntax tree (CST / parse tree)** keeps *everything*: every keyword,
  brace, comma, whitespace, comment. It's a faithful, lossless picture of the
  text.
- An **abstract syntax tree (AST)** keeps only what *matters* for meaning: the
  function, its name, its body — dropping punctuation noise.

tree-sitter (Doc 02) produces a CST but lets you *navigate it like an AST* by
asking only for the named children you care about (the `function_item`'s `name`
field, its `body` field). In practice we treat tree-sitter's tree as our AST: we
walk it and pick out the node types we defined a meaning for.

## 3. A grammar, concretely

A tiny slice of Rust's grammar, in words:

```
function_item := "fn" name parameters ("->" type)? block
struct_item   := "struct" name (field_declaration_list | ";")
use_declaration := "use" use_clause ";"
call_expression := expression "(" arguments ")"
```

tree-sitter has this encoded for ~25 languages. When it parses
`fn helper() -> i32 { 42 }`, it yields a `function_item` node whose `name` field
is `helper`, whose return type is `i32`, and whose `body` is a block — exactly the
structure our walker reads.

## 4. Why structural extraction is deterministic and free

This is the key property the whole feature rests on:

- **Deterministic.** The same file always parses to the same tree. No model, no
  temperature, no randomness. `file contains struct Parser` is a *fact* read off
  the tree — that's why it's tagged EXTRACTED (Doc 00 §3). Re-running extraction
  gives byte-identical results, which makes caching (Doc 10) and reproducible
  graphs possible.
- **Free.** No API calls, no tokens spent. Parsing a 1000-line file is
  microseconds of pure CPU. Contrast the optional LLM pass (F27c, Doc 09) which
  costs tokens and is non-deterministic — used only for edges the AST genuinely
  can't see.

So: **extract everything you can structurally; only reach for the model for what's
left.**

## 5. Error recovery (why a half-broken file still works)

Real code being edited is often syntactically broken mid-keystroke. A naive parser
aborts at the first error and gives you nothing. tree-sitter does **error
recovery** (Doc 02 §3): it inserts `ERROR` nodes around the broken region and
keeps parsing the rest. For us that means a file with one malformed function still
yields a usable tree for every other entity in it — extraction degrades
gracefully instead of failing wholesale.

## 6. What the walker takes from the tree

Our Rust walker (Doc 03) cares about a small set of node types:

| tree-sitter node     | becomes                                  |
|----------------------|-------------------------------------------|
| `struct_item`        | a `Struct` node + `file contains` edge    |
| `enum_item`          | an `Enum` node + `contains`               |
| `trait_item`         | a `Trait` node + `contains`               |
| `function_item`      | a `Function` node + `contains`            |
| `impl_item`          | `impl Trait for T` → `implements` edge    |
| `use_declaration`    | an `imports` edge                         |
| `call_expression`    | a call → resolved to a `calls` edge       |

Everything else (whitespace, braces, expressions we don't model) is walked through
but not turned into graph elements.

---

**Next:** Doc 02 — tree-sitter (the parser we use, and its wasm caveat).
