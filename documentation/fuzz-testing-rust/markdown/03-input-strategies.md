---
title: Input Strategies
section: 03
---

# Input Strategies

Fuzz targets receive raw bytes. How you interpret those bytes determines what code paths are reachable. This document covers common patterns for structuring fuzz input.

## Single blob (simplest)

The entire byte slice is one input:

```rust
fuzz_target!(|data: &[u8]| {
    if let Ok(json) = serde_json::from_slice::<Value>(data) {
        let _ = my_crate::process(&json);
    }
});
```

Best for: parsers, serializers, any function that takes a single raw input.

## Split in half (two inputs)

Divide the slice at the midpoint:

```rust
fuzz_target!(|data: &[u8]| {
    if data.len() < 2 { return; }
    let mid = data.len() / 2;
    let (a, b) = data.split_at(mid);

    let Ok(schema) = serde_json::from_slice::<Value>(a) else { return; };
    let Ok(instance) = serde_json::from_slice::<Value>(b) else { return; };

    let Ok(v) = my_crate::compile(&schema) else { return; };
    let _ = v.is_valid(&instance);
});
```

Best for: validation (schema + instance), comparison functions (input A vs input B).

## Split at arbitrary positions (three+ inputs)

Divide the slice into thirds or at specific offsets:

```rust
fuzz_target!(|data: &[u8]| {
    if data.len() < 6 { return; }
    let schema_end = data.len() / 3;
    let uri_end = 2 * data.len() / 3;

    let Ok(schema) = serde_json::from_slice::<Value>(&data[..schema_end]) else { return; };
    let Ok(base_uri) = core::str::from_utf8(&data[schema_end..uri_end]) else { return; };
    let Ok(reference) = core::str::from_utf8(&data[uri_end..]) else { return; };

    // ... use schema, base_uri, reference
});
```

Best for: URI resolution (schema + base URI + reference string), multi-argument functions.

## Length-prefixed input

Read the first few bytes as a length prefix, then use that many bytes for the next input:

```rust
fuzz_target!(|data: &[u8]| {
    if data.len() < 5 { return; }

    let schema_len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    if data.len() < 4 + schema_len + 1 { return; }

    let schema_bytes = &data[4..4 + schema_len];
    let instance_bytes = &data[4 + schema_len..];

    let Ok(schema) = serde_json::from_slice::<Value>(schema_bytes) else { return; };
    let Ok(instance) = serde_json::from_slice::<Value>(instance_bytes) else { return; };

    // ...
});
```

Best for: variable-length inputs where you want to control the split point precisely.

## Interpreting bytes as structured data

Use the first byte(s) to select a mode, then interpret the rest accordingly:

```rust
fuzz_target!(|data: &[u8]| {
    if data.is_empty() { return; }

    let mode = data[0];
    let payload = &data[1..];

    match mode {
        0 => { /* interpret payload as JSON object */ }
        1 => { /* interpret payload as JSON array */ }
        2 => { /* interpret payload as a string */ }
        3 => { /* interpret payload as a number */ }
        _ => { /* ignore */ }
    }
});
```

Best for: dispatching to different code paths based on input type.

## Using arbitrary() from proptest (advanced)

If you want typed input rather than raw bytes, you can combine `proptest` with fuzzing:

```rust
// Not a libFuzzer target — a proptest-based approach
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_compile(s in any::<MySchemaGen>()) {
        let json = serde_json::to_value(&s).unwrap();
        let _ = my_crate::compile(&json);
    }
}
```

This is property-based testing, not fuzzing. Use it alongside fuzz targets, not instead of them. Proptest generates typed inputs with generators, fuzzing generates byte sequences. They complement each other.

## Dictionary-driven input

A dictionary guides the fuzzer toward syntactically meaningful mutations. See the [Seed Corpus and Dictionaries](./04-corpus-and-dict.md) page for details.

## When to use which strategy

| Strategy | When |
|----------|------|
| Single blob | Parser, deserializer, single-argument function |
| Split in half | Two related inputs (schema + instance) |
| Split at positions | Three+ inputs of similar importance |
| Length-prefixed | Variable-length inputs with explicit boundaries |
| Mode dispatch | Multiple entry points share a target |
| proptest | Typed invariants, not crash detection |

## Common pitfalls

**Using `data[0]` without checking length.** Always check `data.is_empty()` or `data.len() < N` before indexing.

**Splitting at `data.len() / 2` when one side can be empty.** If `data.len() == 1`, then `mid == 0`, so `data.split_at(0)` returns `(&[], data)`. Guard against this.

**Parsing UTF-8 with `str::from_utf8_unchecked`.** Never use unsafe UTF-8 conversion on fuzzer input. Use `core::str::from_utf8` and handle `Err`.

**Assuming the fuzzer will reach deep code quickly.** libFuzzer explores breadth-first through the decision tree. If you have 10 nested branches, you may need hours of fuzzing to reach the deepest one. Use seed inputs to jump-start coverage.
