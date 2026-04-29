---
title: Setup and Quick Start
section: 01
---

# Setup and Quick Start

## Installation

Add proptest to your `Cargo.toml`:

```bash
cargo add --dev proptest
```

Or manually:

```toml
[dev-dependencies]
proptest = "1.5"
```

proptest has no unstable features and works on stable Rust. It does not require nightly.

## First property test

Create a test file:

```rust
// tests/property_tests.rs
use proptest::prelude::*;

// The function under test
fn reverse(input: &[i32]) -> Vec<i32> {
    let mut out = input.to_vec();
    out.reverse();
    out
}

proptest! {
    #[test]
    fn reverse_is_involutive(ref input in any::<Vec<i32>>()) {
        let reversed = reverse(input);
        let double = reverse(&reversed);
        prop_assert_eq!(double, *input);
    }
}
```

Run it:

```bash
cargo test reverse_is_involutive
```

You will see:

```
running 1 test
  case: 0; result: Ok; steps: 0
  case: 1; result: Ok; steps: 0
  case: 2; result: Ok; steps: 0
  ...
  case: 255; result: Ok; steps: 0
test reverse_is_involutive ... ok
```

The framework ran 256 test cases, each with a different random `Vec<i32>`. Every case passed, so the test succeeded.

## Understanding the output

Each line is one test case:

- **case**: the test case number
- **result**: `Ok` (passed), `Err` (failed), or `Abort` (too many discarded cases)
- **steps**: shrinking steps taken (0 when the case passed)

The default 256 cases per property comes from `Config::num_cases`. You can change it:

```rust
proptest! {
    #![proptest_config(ProptestConfig {
        cases: 1024,        // run 1024 cases instead of 256
        max_shrink_steps: 5000,  // shrinking iterations
        verbosity: Verbosity::Verbose,
        ..Default::default()
    })]

    #[test]
    fn my_property(ref x in any::<u64>()) {
        prop_assert!(x.wrapping_add(1) != 0 || *x == u64::MAX);
    }
}
```

## Inline vs. module-level tests

You can put property tests in the same file as your code:

```rust
// src/lib.rs
pub fn clamp(value: i32, min: i32, max: i32) -> i32 {
    if value < min { min } else if value > max { max } else { value }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn clamp_in_bounds(
            ref value in any::<i32>(),
            ref min in any::<i32>(),
            ref max in any::<i32>()
        ) {
            prop_assume!(*min <= *max);
            let result = clamp(*value, *min, *max);
            prop_assert!(result >= *min);
            prop_assert!(result <= *max);
        }
    }
}
```

`prop_assume!` discards cases that don't satisfy the precondition (`min <= max`). Without it, proptest would try cases where `min > max`, which would trigger the assertion failure but then discard them — wasting test cycles.

## The prelude

`use proptest::prelude::*` imports:

- `proptest!` — the main macro
- `any::<T>()` — generator for any value of type T
- `prop_assert!`, `prop_assert_eq!`, `prop_assert_ne!` — assertions with shrinking
- `ProptestConfig`, `Strategy`, `TestCaseResult` — configuration and trait types
- `prop_compose!` — macro for building custom generators
- `prop_oneof!`, `prop_fold!`, `prop_map!` — combinator macros

## Running property tests

```bash
# Run all property tests
cargo test

# Run a specific property test
cargo test clamp_in_bounds

# Run with verbose output (every case printed)
PROPTEST_VERBOSITY=full cargo test clamp_in_bounds

# Run with a specific seed for reproducibility
PROPTEST_CASES=1000 cargo test clamp_in_bounds

# Run only failing tests (shows shrunk minimal cases)
cargo test -- --ignored
```

## The failure report

When a property fails, proptest gives you a complete diagnosis:

```
thread 'tests::clamp_in_bounds' panicked at 'assertion failed: result >= *min', tests/property_tests.rs:42
---
Failed 1 cases in 12 tests
Case: (value = 0, min = 2, max = 1)
Shrunk to: (value = 0, min = 2, max = 2)
Reproduce with: prop_assert!(*min <= *max); // this was filtered by prop_assume!
```

Key information:

- **Original case**: the first failing input found
- **Shrunk case**: the minimal failing input (after proptest's shrinking algorithm)
- **Reproduce with**: code to regenerate the exact failing case

The shrunk case is usually the smallest input that triggers the bug. Use it as a regression test.

## Common errors

**"cannot find macro `proptest` in this scope"**

Add `use proptest::prelude::*;` to the test module.

**"the trait `Strategy` is not implemented"**

The type inside `any::<T>()` must implement `Arbitrary`, which most standard library types do. For custom types, derive or implement `Arbitrary`.

**"test panicked" with "too many rejections"**

Your `prop_assume!` filter is discarding too many cases. By default, proptest allows at most 10,000 rejections per property. If your precondition is too restrictive, use a more targeted generator instead:

```rust
// Bad: too many rejections
proptest! {
    #[test]
    fn test(ref x in any::<i32>(), ref y in any::<i32>()) {
        prop_assume!(x > 0 && y > 0);
        prop_assume!(x < y);
        // ...
    }
}

// Good: generate only valid inputs
proptest! {
    #[test]
    fn test((ref x, ref y) in (1..1000i32, 1..1000i32).prop_flat_map(|(a, b)| (a..=b).prop_map(move |c| (a, c)))) {
        // x and y are always positive and x < y
    }
}
```

---

## Checklist for a new property test

- [ ] Install `proptest` in `[dev-dependencies]`
- [ ] `use proptest::prelude::*;` in the test module
- [ ] Write the `proptest!` macro block
- [ ] Use `ref` in generator bindings to borrow generated values
- [ ] Use `prop_assert!` instead of `assert!`
- [ ] Use `prop_assume!` to filter invalid cases (sparingly)
- [ ] Run with `cargo test` and verify it passes
- [ ] Verify shrinking works by introducing a deliberate bug
