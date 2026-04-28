---
workspace_name: "ewe_platform"
spec_directory: "specifications/17-foundation-jsonschema"
this_file: "specifications/17-foundation-jsonschema/LEARNINGS.md"

created: 2026-04-28
last_updated: 2026-04-28
---

# Learnings: Foundation JSON Schema

## Read By

1. **Implementation Agent** reads this for context on previous discoveries
2. **Main Agent** reads this when resuming work on specification
3. **Future agents** read this to avoid repeating mistakes

## Purpose

Document non-obvious discoveries, critical decisions, and gotchas specific to THIS specification.

**Note**: Generic language patterns go in `.agents/skills/rust-clean-code/` instead.

---

## Critical Implementation Details

- The reference project (Stranger6667/jsonschema at `/home/darkvoid/Boxxed/@formulas/src.rust/src.jsonschema/jsonschema/`) has 14 crates. We consolidate into a single crate `foundation_jsonschema` with internal modules instead.
- The reference project uses `fluent-uri` for RFC 3986 URI resolution. We need a lightweight alternative or minimal internal implementation since fluent-uri may pull in std-only deps.
- Draft 4 uses `id` (not `$id`) for schema identification — this is a common source of bugs.
- `$recursiveRef` (Draft 2019-09) and `$dynamicRef` (Draft 2020-12) have subtly different semantics — both walk the dynamic scope but `$dynamicRef` uses named anchors while `$recursiveRef` checks `$recursiveAnchor: true`.
- JSON Pointer escaping: `~0` → `~`, `~1` → `/` — order matters (unescape `~1` first, then `~0`).

## Common Failures and Fixes

- (none yet — will be populated during implementation)

## Testing Insights

- The official JSON Schema Test Suite has ~7200 tests across 5 drafts
- The referencing test suite has ~20 known xfails related to RFC 3986 port normalization
- Test suite integration in the reference project uses proc macros for code generation — we should use a simpler approach (build.rs or manual test modules with clear commentary)

## Dependencies and Interactions

- `serde` + `serde_json` are workspace dependencies — use workspace versions
- `regex` is a workspace dependency — use workspace version
- `foundation_nostd` provides spin-based sync primitives if needed for no_std memoization caches

## Future Considerations

- Schema bundling (combining external refs into single document) could be added as a later feature
- Schema dereferencing (inlining $ref targets) could be added as a later feature
- Python/Ruby bindings are out of scope — this is a pure Rust library

---

_Last Updated: 2026-04-28_
