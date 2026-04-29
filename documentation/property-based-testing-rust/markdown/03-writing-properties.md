---
title: Writing Properties
section: 03
---

# Writing Properties

A property is an assertion about your code that must hold for all valid inputs. Writing good properties is the hardest part of property-based testing. This document covers the most effective property patterns.

## Round-trip properties

The most reliable and useful property: transform A to B and back to A. If the round-trip succeeds, the forward and inverse transformations are correct.

```rust
proptest! {
    #[test]
    fn json_roundtrip(ref original in any::<serde_json::Value>()) {
        let serialized = serde_json::to_string(&original).unwrap();
        let deserialized: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        prop_assert_eq!(deserialized, original);
    }
}
```

Other round-trips:

```rust
// Compression
fn roundtrip_compress(ref data in any::<Vec<u8>>()) {
    let compressed = compress(data);
    let decompressed = decompress(&compressed).unwrap();
    prop_assert_eq!(decompressed, data);
}

// Encoding/decoding
fn roundtrip_base64(ref data in any::<Vec<u8>>()) {
    let encoded = base64::encode(data);
    let decoded = base64::decode(&encoded).unwrap();
    prop_assert_eq!(decoded, data);
}

// Parsing/formatting
fn roundtrip_format_parse(ref input in any::<i64>()) {
    let formatted = format!("{}", input);
    let parsed: i64 = formatted.parse().unwrap();
    prop_assert_eq!(parsed, input);
}
```

Round-trip properties catch: serialization bugs, encoding errors, precision loss, truncation, and character handling issues.

## Idempotence properties

Applying a function twice should be the same as applying it once:

```rust
proptest! {
    #[test]
    fn sort_is_idempotent(ref input in any::<Vec<i32>>()) {
        let mut sorted = input.clone();
        sorted.sort();
        let mut sorted_again = sorted.clone();
        sorted_again.sort();
        prop_assert_eq!(sorted, sorted_again);
    }

    #[test]
    fn trim_is_idempotent(ref input in "[\\s\\S]*") {
        let trimmed1 = input.trim();
        let trimmed2 = trimmed1.trim();
        prop_assert_eq!(trimmed1, trimmed2);
    }

    #[test]
    fn normalize_whitespace_is_idempotent(ref input in any::<String>()) {
        let norm1 = normalize_whitespace(&input);
        let norm2 = normalize_whitespace(&norm1);
        prop_assert_eq!(norm1, norm2);
    }
}
```

Idempotence properties catch: incomplete transformations, state-dependent bugs, and side effects.

## Consistency properties

Two implementations should produce the same result:

```rust
proptest! {
    #[test]
    fn fast_sort_matches_slow_sort(ref input in any::<Vec<i32>>()) {
        let mut a = input.clone();
        let mut b = input.clone();
        fast_sort(&mut a);
        slow_sort(&mut b);
        prop_assert_eq!(a, b);
    }

    #[test]
    fn custom_validate_matches_regex(ref input in any::<String>()) {
        let custom_result = custom_validate(&input);
        let regex_result = regex_is_match("^[a-z]+$", &input);
        prop_assert_eq!(custom_result, regex_result);
    }
}
```

Consistency properties catch: implementation drift, edge case handling differences, and regression bugs.

## Inverse properties

Two functions are inverses if applying one then the other returns the original:

```rust
proptest! {
    #[test]
    fn encode_decode_inverse(ref input in any::<Vec<u8>>()) {
        let encoded = encode(input);
        let decoded = decode(&encoded).unwrap();
        prop_assert_eq!(decoded, input);
    }

    #[test]
    fn uri_encode_decode_inverse(ref input in "[a-zA-Z0-9_.!~*'()-]*") {
        let encoded = url_encode(input);
        let decoded = url_decode(&encoded).unwrap();
        prop_assert_eq!(decoded, input);
    }
}
```

## Monotonicity properties

Adding constraints should never increase the valid set:

```rust
proptest! {
    #[test]
    fn additional_constraints_never_widen(ref schema in any::<Schema>(), ref instance in any::<Value>()) {
        let base_valid = validate(&schema, &instance);
        let extended_schema = schema.with_min_length(5);
        let extended_valid = validate(&extended_schema, &instance);
        // If the extended schema validates, the base schema must also validate
        if extended_valid {
            prop_assert!(base_valid);
        }
    }
}
```

## Error handling properties

Invalid inputs should always return errors, never panic:

```rust
proptest! {
    #[test]
    fn invalid_json_always_errors(ref input in any::<String>()) {
        // Skip valid JSON — those are tested in round-trip
        prop_assume!(serde_json::from_str::<serde_json::Value>(&input).is_err());
        let result = my_parse(&input);
        prop_assert!(result.is_err());
    }
}
```

## State machine properties

For code with mutable state, model the state machine and verify transitions:

```rust
proptest! {
    #[test]
    fn counter_invariant(ref ops in prop::collection::vec(
        prop_oneof![CounterOp::Increment, CounterOp::Decrement, CounterOp::Reset],
        0..20
    )) {
        let mut counter = Counter::new();
        let mut expected = 0i32;

        for op in ops {
            match op {
                CounterOp::Increment => {
                    counter.increment();
                    expected += 1;
                }
                CounterOp::Decrement => {
                    counter.decrement();
                    expected -= 1;
                }
                CounterOp::Reset => {
                    counter.reset();
                    expected = 0;
                }
            }
            prop_assert_eq!(counter.value(), expected);
        }
    }
}
```

## Equivalence properties

Different input orderings should produce the same result:

```rust
proptest! {
    #[test]
    fn commutativity(ref a in any::<i64>(), ref b in any::<i64>()) {
        prop_assert_eq!(add(a, b), add(b, a));
        prop_assert_eq!(multiply(a, b), multiply(b, a));
    }

    #[test]
    fn associativity(
        ref a in any::<i64>(),
        ref b in any::<i64>(),
        ref c in any::<i64>()
    ) {
        prop_assert_eq!(
            add(add(a, b), c),
            add(a, add(b, c))
        );
    }
}
```

## Invariants on output structure

The output should have specific structural properties:

```rust
proptest! {
    #[test]
    fn sort_output_is_sorted(ref input in any::<Vec<i32>>()) {
        let sorted = sort(&input);
        for window in sorted.windows(2) {
            prop_assert!(window[0] <= window[1]);
        }
    }

    #[test]
    fn compress_output_is_smaller(ref input in any::<Vec<u8>>()) {
        let compressed = compress(&input);
        // At minimum, compression should not increase size for large inputs
        if input.len() > 1000 {
            prop_assert!(compressed.len() <= input.len());
        }
    }
}
```

## Combining properties

Multiple properties on the same function:

```rust
proptest! {
    // Property 1: round-trip
    #[test]
    fn json_parse_format_roundtrip(ref original in any::<serde_json::Value>()) {
        let formatted = serde_json::to_string(&original).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&formatted).unwrap();
        prop_assert_eq!(parsed, original);
    }

    // Property 2: invalid input returns error
    #[test]
    fn json_parse_rejects_invalid(ref input in any::<String>()) {
        prop_assume!(serde_json::from_str::<serde_json::Value>(&input).is_err());
        let result = my_json_parse(&input);
        prop_assert!(result.is_err());
    }

    // Property 3: parse is idempotent on valid JSON
    #[test]
    fn json_parse_idempotent(ref input in any::<serde_json::Value>()) {
        let json1 = serde_json::to_string(&input).unwrap();
        let parsed1: serde_json::Value = serde_json::from_str(&json1).unwrap();
        let json2 = serde_json::to_string(&parsed1).unwrap();
        let parsed2: serde_json::Value = serde_json::from_str(&json2).unwrap();
        prop_assert_eq!(parsed1, parsed2);
    }
}
```

---

## Checklist for writing a new property

- [ ] Identify what the function is supposed to guarantee (round-trip, invariance, consistency)
- [ ] Choose the simplest property that tests the guarantee
- [ ] Write a generator that covers the input domain (not just common cases)
- [ ] Use `prop_assert!` for assertions (with shrinking)
- [ ] Run the test and verify it passes
- [ ] Intentionally break the code to verify the property catches the bug
- [ ] Add the minimal failing case as a regression test if discovered
