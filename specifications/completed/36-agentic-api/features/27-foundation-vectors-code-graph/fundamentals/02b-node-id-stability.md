# Fundamentals 02b — Node id stability

Ids are the glue. Edges reference nodes *by id*, dedup merges *by id/label*, and a
graph built on one machine must be queryable on another. Get id construction wrong
and the whole graph silently corrupts. This doc is short and load-bearing.

---

## 1. What an id must guarantee

- **Deterministic** — the same entity always gets the same id, every run, every
  machine. (Otherwise caching and merging break.)
- **Collision-resistant** — two different entities must not accidentally share an
  id, *especially* same-named files in different directories.
- **Portable** — an id built in `/home/alice/proj/src/a.rs` must match the one a
  CI box builds in `/build/123/src/a.rs`. Absolute paths are not portable; the id
  scheme must not bake them in.

## 2. `make_id(stem, name)`

The id of an entity is its **file stem** plus its **name**, lowercased and reduced
to `[a-z0-9_]`:

```rust
pub fn make_id(stem: &str, name: &str) -> String {
    let raw = if stem.is_empty() { name.to_lowercase() }
              else { format!("{}_{}", stem.to_lowercase(), name.to_lowercase()) };
    raw.chars()
       .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
       .collect()
}
```

So `struct Parser` in `parser.rs` → `parser_parser`. No dots, no slashes, no case
— ids are stable tokens you can use as map keys and in serialized JSON.

## 3. Parent-qualified stems (the collision fix)

Two files named `mod.rs` in different directories would both have stem `mod` and
collide on every entity. graphify fixes this by **parent-qualifying** the stem
(`_file_stem`, extract.py:51): include the parent directory name.

```rust
pub fn file_stem_qualified(path: &str) -> String {
    // src/parser/mod.rs  -> "parser_mod"
    // src/lexer/mod.rs   -> "lexer_mod"
    let parent = parent_dir_name(path);   // "parser"
    let stem   = file_stem(path);         // "mod"
    if parent.is_empty() || parent == "." { stem }
    else { format!("{parent}_{stem}") }
}
```

Now the two `mod.rs` files produce distinct stems (`parser_mod`, `lexer_mod`) and
their entities don't collide.

## 4. Absolute → relative path remap

File *nodes* are keyed on the path. To stay portable across machines, paths are
stored **relative to the repo root**, not absolute. graphify remaps
absolute→relative at the end of build (extract.py:3444). The rule: never persist
an absolute path in a node id or a `source_file` field — a graph that embeds
`/home/alice/...` won't match one built in CI.

## 5. Label normalization (the merge key)

Beyond ids, dedup (Doc 04 §5) merges nodes whose **labels** normalize to the same
key. `normalize_label` lowercases and keeps only `[a-z0-9_]`:

```rust
pub fn normalize_label(label: &str) -> String {
    label.to_lowercase().chars()
         .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
         .collect()
}
```

This is what lets a cross-file `calls` resolver match `helper()` (a call site) to
`fn helper` (a definition) even when surface punctuation differs, and what the
3-layer dedup uses to coalesce the same entity discovered from multiple files.

## 6. Why this is in its own doc

Every later doc leans on these three facts: **ids are `make_id(parent-qualified
stem, name)`, paths are relative, merges key on `normalize_label`.** If you skip
this, the call-resolution and dedup logic in Doc 04 won't make sense, and you'll
be tempted to "simplify" id construction in a way that reintroduces same-name-file
collisions.

---

**Next:** Doc 03 — the `LanguageConfig` walker and the Rust bespoke extractor.
