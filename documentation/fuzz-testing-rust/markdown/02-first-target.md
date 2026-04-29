---
title: Your First Fuzz Target
section: 02
---

# Your First Fuzz Target

## The fuzz_target macro

Every fuzz target starts with the `fuzz_target!` macro from `libfuzzer-sys`:

```rust
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // ... fuzz logic here
});
```

The macro tells libFuzzer to call your closure repeatedly, passing different byte slices each time. The `#![no_main]` attribute replaces the standard `main` function with libFuzzer's entry point.

## Example: fuzzing a JSON parser

```rust
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Parse the bytes as JSON — if parsing fails, that is fine
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(data) {
        // Now do something with the parsed value that might panic
        let _ = my_crate::process(&value);
    }
});
```

This target feeds arbitrary bytes to the JSON parser, then passes whatever parses successfully into `process()`. The fuzzer will explore every branch inside `process()`, including deeply nested object traversal, enum matching, and string formatting.

## Example: fuzzing compilation (no panic guarantee)

For a library where the compilation step is the critical path:

```rust
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(schema) = serde_json::from_slice::<serde_json::Value>(data) {
        // We do not care if compilation succeeds or fails.
        // We only care that it does not panic.
        let _ = my_crate::compile(&schema);
    }
});
```

The key pattern: `let _ = ...`. Discarding the result is intentional. The fuzzer explores both the success and failure paths through `compile()`. Both paths must be panic-free.

## Example: two-input validation

When you need two inputs — a schema and an instance — split the byte slice:

```rust
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 {
        return;
    }
    let mid = data.len() / 2;
    let (schema_bytes, instance_bytes) = data.split_at(mid);

    let Ok(schema) = serde_json::from_slice::<serde_json::Value>(schema_bytes) else {
        return;
    };
    let Ok(instance) = serde_json::from_slice::<serde_json::Value>(instance_bytes) else {
        return;
    };
    let Ok(validator) = my_crate::compile(&schema) else {
        return;
    };

    // Exercise all validation methods
    let _ = validator.is_valid(&instance);
    let _ = validator.validate(&instance);
    let _ = validator.iter_errors(&instance).count();
});
```

This target covers both the compilation and validation code paths. Early returns on parse/compile failure are normal — they simply prune that execution branch. The fuzzer will evolve inputs to avoid early returns, gradually reaching deeper code.

## Handling malformed input

The fuzzer generates garbage. Your target must handle:

- **Invalid JSON** — `serde_json::from_slice` returns `Err`. Use `if let Ok(...)` or early return.
- **Unexpected types** — Expecting an object but getting a number. Handle with `as_object()` / `as_str()` and return early.
- **Extreme nesting** — JSON with thousands of nested objects. If your parser supports depth limits, test them.
- **Empty input** — `data.len() == 0`. Check for minimum length before splitting.
- **Unicode** — Arbitrary byte sequences that are not valid UTF-8. Use `core::str::from_utf8` and handle errors.

Do not `unwrap()` or `expect()` on any user-controlled value. If you `unwrap()` a `from_utf8()` result, the fuzzer will crash on non-UTF-8 input, and you will not know whether it is a real bug or just the fuzzer testing bad encoding.

## The golden rule

**A fuzz target must never panic.** Every branch must either:

1. Succeed and return normally, or
2. Fail gracefully and return early / return `Err`

If the target panics, the fuzzer stops and saves the crashing input to `fuzz/artifacts/`.

## Structuring multiple targets

Each target should focus on one code path:

| Target | What it tests | Input |
|--------|---------------|-------|
| `builder` | Schema compilation | JSON schema bytes |
| `validation` | Instance validation | Schema bytes + instance bytes |
| `parser` | String parsing | Arbitrary byte strings |
| `uri` | URI resolution | URI bytes |
| `pointer` | JSON pointer traversal | Pointer bytes + JSON object bytes |

Keep targets small and focused. If one target covers both compilation and validation, the fuzzer wastes time on compile failures when you only want to test validation.

## Using seed files

Place meaningful inputs in `fuzz/seeds/<target_name>/`:

```bash
mkdir -p fuzz/seeds/builder
echo '{"type": "string"}' > fuzz/seeds/builder/simple.json
echo '{"allOf": [{"type": "object"}, {"required": ["id"]}]}' > fuzz/seeds/builder/al_of.json
```

The fuzzer reads these on startup and uses them as starting points for mutation. Seeds dramatically speed up coverage compared to starting from random bytes.

Run with seeds:

```bash
cargo +nightly fuzz run builder
```

The fuzzer automatically finds seeds in `corpus/builder/` and `seeds/builder/`.

---

## Checklist for a new target

- [ ] Target starts with `#![no_main]`
- [ ] Uses `fuzz_target!(|data: &[u8]| { ... })`
- [ ] Does not `unwrap()` on any user-controlled value
- [ ] Has early returns for parse failures (not panics)
- [ ] Has at least one seed file
- [ ] Compiles with `cargo +nightly fuzz build <target>`
- [ ] Runs successfully with `cargo +nightly fuzz run <target> -- -max_total_time=10`
