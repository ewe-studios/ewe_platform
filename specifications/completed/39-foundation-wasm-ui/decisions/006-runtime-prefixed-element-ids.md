# 006 — Runtime-prefixed element IDs (compile-time template scope + instance prefix)

**Date:** 2026-06-08
**Status:** Resolved

### Decision

**Do NOT require user-provided IDs.** Instead:

1. **Compile-time**: `html!` macro assigns sequential IDs (0, 1, 2...) **scoped to the template**
2. **Runtime**: each component instance gets a unique prefix (e.g., `42`)
3. **Final lookup**: `"42:0"`, `"42:1"` — no cross-component collisions

### How it works

```rust
// Compile-time: html! assigns IDs 0, 1, 2 within this template
html! {
    <div>              // template-id = 0
        <span>{name}</span>  // template-id = 1
    </div>
}

// Runtime: component gets instance ID 42
// Final JS lookup: "42:0", "42:1"
```

### Why this solves the problems

| Problem | Solution |
|---------|----------|
| **Loops** — same template, multiple iterations | Each iteration is a new component instance → different prefix |
| **Cross-component collision** | Component instance prefix isolates ID namespaces |
| **Registry lifecycle** | Component disconnect → unregister all IDs with that prefix |
| **Conditional rendering** | IDs are assigned regardless (waste is fine — sequential counter, not allocated) |

### Tradeoffs accepted

- ID gaps in conditionals — acceptable (sequential counter doesn't allocate)
- String-prefixed keys (`"42:0"`) — minor overhead, but clear and collision-free
- Component instance ID generation — simple atomic counter per app
