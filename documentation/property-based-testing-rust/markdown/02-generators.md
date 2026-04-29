---
title: Generators and Strategies
section: 02
---

# Generators and Strategies

Generators (called `Strategy` in proptest) are the engine of property-based testing. They produce random values that satisfy type constraints and structural invariants. A well-designed generator is the difference between a property test that finds real bugs and one that mostly generates valid cases that trivially pass.

## Built-in generators

### Primitives

`any::<T>()` generates arbitrary values for any `T` that implements `Arbitrary`:

```rust
any::<u8>()       // 0..=255
any::<i32>()      // i32::MIN..=i32::MAX
any::<f64>()      // including NaN, Inf, -Inf
any::<bool>()     // true or false
any::<String>()   // any valid UTF-8 string, 0..256 chars
any::<Vec<u8>>()  // vector of 0..256 bytes
```

### Range generators

For bounded values, use range syntax:

```rust
0..100i32           // 0..=99 (exclusive end)
0..=100i32          // 0..=100 (inclusive end)
1..=100i32          // 1..=100
'a'..='z'           // single lowercase ASCII letter
```

### Collection generators

```rust
// Fixed-size vector
prop::collection::vec(any::<i32>(), 5)     // exactly 5 elements

// Variable-size vector
prop::collection::vec(any::<i32>(), 0..10) // 0 to 9 elements
prop::collection::vec(any::<i32>(), 10..=20) // 10 to 20 elements

// Set (no duplicates)
prop::collection::hash_set(any::<i32>(), 0..10)

// Map (key-value pairs)
prop::collection::hash_map(any::<String>(), any::<i32>(), 0..5)
```

### Option and Result

```rust
prop::option::of(any::<i32>())  // None | Some(value)
prop::result:: Ok(any::<i32>(), any::<String>())  // Ok | Err
```

## Combinators

Combinators transform generators into new generators. They are the primary building block for complex types.

### prop_map

Transform the output of a generator:

```rust
// Generate only even integers
(any::<i32>()).prop_map(|x| x & !1)

// Generate non-empty strings
(any::<String>()).prop_map(|s| if s.is_empty() { "x".to_string() } else { s })

// Generate percentages (0..=100)
(any::<u8>()).prop_map(|x| (x % 101) as u8)
```

### prop_filter

Reject values that don't match a predicate (uses rejection sampling, so use sparingly):

```rust
// Generate only non-zero integers
(any::<i64>()).prop_filter("non-zero", |&x| x != 0)

// Generate only valid email-like strings
(any::<String>()).prop_filter("contains @", |s| s.contains('@'))
```

### prop_flat_map

Generate a value that depends on a previously generated value:

```rust
// Generate a vector where the length is a random number
(any::<usize>()).prop_flat_map(|len| prop::collection::vec(any::<i32>(), len..=len))
```

Use `prop_flat_map` instead of `prop_assume!` when you need dependent values.

### prop_oneof

Choose from multiple generators:

```rust
// Generate either a number or a string
prop_oneof![
    any::<i32>().prop_map(|x| Value::Number(x)),
    any::<String>().prop_map(|s| Value::String(s)),
    Just(Value::Bool(true)),
    Just(Value::Bool(false)),
    Just(Value::Null),
]
```

### prop_union

Like `prop_oneof!` but with explicit weights:

```rust
// 80% numbers, 20% strings
prop_union![
    8 => any::<i32>().prop_map(|x| Value::Number(x)),
    2 => any::<String>().prop_map(|s| Value::String(s)),
]
```

### prop_fold

Build recursive structures:

```rust
// Build a tree of arbitrary depth
prop::collection::vec(any::<Leaf>(), 0..10).prop_fold(
    Vec::new(),
    |mut acc: Vec<Node>, leaf| {
        acc.push(Node::Leaf(leaf));
        acc
    }
)
```

## Composing generators for tuples

Tuples of generators produce tuples of values:

```rust
// Two independent values
(any::<i32>(), any::<String>())

// Three values
(any::<i32>(), any::<i32>(), any::<bool>())

// Nested
((any::<i32>(), any::<i32>()), any::<String>())
```

In the test:

```rust
proptest! {
    #[test]
    fn test((ref a, ref b, ref c) in (any::<i32>(), any::<i32>(), any::<bool>())) {
        // a: &i32, b: &i32, c: &bool
    }
}
```

## Custom types: deriving Arbitrary

For simple structs, derive `Arbitrary`:

```rust
use proptest::prelude::*;

#[derive(Debug, Clone, Arbitrary)]
struct User {
    name: String,
    age: u8,  // 0..=255
    email: String,
}

proptest! {
    #[test]
    fn test_user(ref user in any::<User>()) {
        prop_assert!(!user.name.is_empty() || user.age == 0);
    }
}
```

Derived `Arbitrary` uses the default generator for each field. This is fine for simple cases but often needs customization.

## Custom types: implementing Arbitrary manually

For control over field generators:

```rust
impl Arbitrary for User {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        (
            "[a-zA-Z]{3,20}".prop_filter(|s| !s.is_empty(), |s| s.clone()),  // name
            18u8..=120u8,                                                       // age
            "[a-z]+@[a-z]+\\.[a-z]+".prop_filter(|s| s.contains('@'), |s| s.clone()), // email
        )
            .prop_map(|(name, age, email)| User { name, age, email })
            .boxed()
    }
}
```

## Recursive types

For recursive structures (ASTs, trees, nested JSON), use `prop_oneof!` with a depth limit:

```rust
#[derive(Debug, Clone)]
enum Expr {
    Literal(i64),
    Add(Box<Expr>, Box<Expr>),
    Negate(Box<Expr>),
}

impl Arbitrary for Expr {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        // Use proptest's recursive strategy with depth control
        let leaf = any::<i64>().prop_map(Expr::Literal);
        leaf.prop_recursive(
            8,   // max depth
            256, // max items
            4,   // max branches per node
            |inner| {
                prop_oneof![
                    (inner.clone(), inner.clone()).prop_map(|(a, b)| Expr::Add(Box::new(a), Box::new(b))),
                    inner.prop_map(|e| Expr::Negate(Box::new(e))),
                ]
            }
        )
        .boxed()
    }
}
```

`prop_recursive` limits the depth and breadth of the generated structure, preventing stack overflow from infinitely nested inputs.

## Strings from regex

Generate strings that match a regex pattern:

```rust
use proptest::string::string_regex;

// Generate email-like strings
string_regex("[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}").unwrap()
```

This is more efficient than generating random strings and filtering.

## Common generator pitfalls

### Using `any::<f64>()` — it includes NaN and Inf

Most numeric code doesn't handle NaN correctly. Use a strategy that excludes them:

```rust
// Bad: includes NaN, Inf, -Inf
any::<f64>()

// Good: only finite numbers
(-1e15f64..1e15f64)
```

### Using `any::<String>()` — it generates invalid UTF-8 in the byte view

The `String` generator only produces valid UTF-8. But if you are testing a parser that reads bytes and then converts to UTF-8, test with `Vec<u8>` and `String::from_utf8`:

```rust
proptest! {
    #[test]
    fn test_parse_from_bytes(ref bytes in any::<Vec<u8>>()) {
        if let Ok(s) = String::from_utf8(bytes.clone()) {
            let result = my_parser(&s);
            // ...
        }
        // If UTF-8 fails, that's expected — the parser should reject it
    }
}
```

### Overusing prop_assume

`prop_assume!` discards cases, which wastes test cycles. If you need `prop_assume!` more than twice in a property, build a better generator instead.

### Generators that are too narrow

`0..10i32` only generates 10 values. With 256 test cases, you'll see duplicates and waste cycles. Use a wider range and let shrinking find edge cases:

```rust
// Too narrow — 10 values, many duplicates
(0..10i32, 0..10i32)

// Better — let shrinking find small values
(any::<i32>(), any::<i32>())
```

proptest's shrinking will reduce large values to small ones when a test fails, so starting with a wide range gives the fuzzer more diverse cases to explore.
