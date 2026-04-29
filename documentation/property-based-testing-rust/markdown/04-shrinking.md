---
title: Shrinking and Debugging
section: 04
---

# Shrinking and Debugging

## What shrinking is

When a property test fails, proptest doesn't just give you the first failing input it found. It systematically reduces the input to find the smallest value that still triggers the failure. This process is called **shrinking**.

For example, if a function panics on `vec![1000, -500, 0, 999, -1]`, proptest might shrink it to `vec![3, -1]` — the minimal case that still causes the panic.

## How shrinking works

For each type, proptest has a type-specific shrinking strategy:

- **Integers**: tries 0, then values closer to 0, then boundary values (MIN, MAX)
- **Vectors**: tries shorter vectors, then removes elements, then shrinks individual elements
- **Strings**: tries empty string, then shorter strings, then removes non-ASCII characters
- **Options**: tries None, then shrinks the inner value
- **Results**: tries Ok, then shrinks the inner value
- **Tuples**: shrinks each element independently
- **Structs**: shrinks each field independently

The shrinking algorithm works in passes:

1. **Coarse shrinking**: remove or zero out entire fields/elements
2. **Fine shrinking**: reduce values within each field to their minimal form
3. **Verification**: each shrunk value is re-tested; only values that still fail are kept

## Reading a shrink report

```
thread 'my_test' panicked at 'assertion failed', src/lib.rs:42
---
Failed 1 cases in 15 tests
Case: (a = 1000, b = -500, c = "hello world")
Shrunk to: (a = 3, b = -1, c = "")
Reproduce with: my_test(3, -1, "")
```

Key fields:

- **Failed 1 cases in N tests**: the number of test cases that passed before the failure was found
- **Case**: the first failing input found (random, not minimal)
- **Shrunk to**: the minimal failing input (this is what you care about)
- **Reproduce with**: code to regenerate the exact failing case

## Shrinking limits

By default, proptest performs up to 10,000 shrink steps. You can adjust this:

```rust
proptest! {
    #![proptest_config(ProptestConfig {
        max_shrink_steps: 50000,  // allow more shrinking steps
        ..Default::default()
    })]

    #[test]
    fn complex_property(ref input in any::<MyComplexType>()) {
        // ...
    }
}
```

Increase `max_shrink_steps` when:
- The input type is deeply nested
- The failing condition is subtle (only triggers on very specific values)
- Shrinking is terminating too early with a sub-optimal case

Decrease `max_shrink_steps` when:
- Shrinking takes too long (hundreds of seconds per failure)
- You are doing rapid development and want faster feedback

## Custom shrinking

For custom types, proptest's derived `Arbitrary` provides default shrinking. To customize:

```rust
impl Arbitrary for MyType {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;
    type Tree = BoxedTree<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        // ...
    }

    fn shrink(&self) -> Self::Tree {
        // Custom shrinker: only shrink specific fields
        let mut trees = Vec::new();

        // Try zeroing the numeric field
        trees.push(MyType { value: 0, ..self.clone() });

        // Try emptying the string field
        trees.push(MyType { name: String::new(), ..self.clone() });

        // Combine: try both at once
        trees.push(MyType { value: 0, name: String::new() });

        trees.into_iter().boxed()
    }
}
```

## Reproducing failures

### From the shrink report

Copy the "Reproduce with" code:

```rust
#[test]
fn regression_my_bug() {
    my_test(3, -1, "");
}
```

This runs the exact failing case without the property test overhead.

### From the proptest `rng_seed`

Each property test run uses a random seed. If a test passes, proptest prints the seed used:

```
prop_verbose: ... rng_seed: Some(Seed(1234567890abcdef))
```

Replay the exact same sequence:

```bash
PROPTEST_RNG_SEED=1234567890abcdef cargo test my_test
```

### From the `proptest-errors` directory

When a property fails, proptest saves the case to `proptest-errors/`:

```
proptest-errors/
└── my_test
    └── 12345678-abcdef-proptest-case
```

Replay it:

```bash
cargo test my_test -- --exact proptest_errors::my_test
```

## Debugging failed properties

### Step 1: Read the shrunk case

The shrunk case is your smallest reproduction. Study it carefully — it is usually the simplest input that exposes the bug.

### Step 2: Run the regression test

Create a test case with the shrunk input:

```rust
#[test]
fn debug_my_bug() {
    let result = my_function(3, -1, "");
    println!("{:?}", result);
    // Add assertions to understand the failure
}
```

Run with `--nocapture` to see println output:

```bash
cargo test debug_my_bug -- --nocapture
```

### Step 3: Trace through the code

Add `println!` or use a debugger to trace through the code with the shrunk input. The small input makes this manageable.

### Step 4: Identify the root cause

Look for:
- Off-by-one errors in index calculations
- Missing boundary checks (MIN, MAX, 0, empty)
- Integer overflow in release mode
- Division by zero
- Pattern matching that doesn't cover all cases
- Floating-point edge cases (NaN, Inf)

### Step 5: Fix and verify

Fix the bug. Run the property test again to confirm it passes:

```bash
cargo test my_test
```

Then add the shrunk case as a regression test:

```rust
#[test]
fn regression_my_bug() {
    my_test(3, -1, "");
}
```

## Verbosity levels

Control output with `PROPTEST_VERBOSITY`:

```bash
# Minimal (only failures and summary)
PROPTEST_VERBOSITY=low cargo test

# Standard (shows each case)
PROPTEST_VERBOSITY=normal cargo test

# Verbose (shows each case with generated values)
PROPTEST_VERBOSITY=verbose cargo test

# Full (shows shrink steps)
PROPTEST_VERBOSITY=full cargo test
```

In the test config:

```rust
proptest! {
    #![proptest_config(ProptestConfig {
        verbosity: Verbosity::Verbose,
        ..Default::default()
    })]

    #[test]
    fn my_test(ref x in any::<i32>()) {
        // ...
    }
}
```

## Common shrinking problems

### Shrinking doesn't find the minimal case

Possible causes:
- `max_shrink_steps` too low (increase it)
- Custom `Arbitrary` with poor shrink implementation (implement `shrink()` manually)
- The failure condition is extremely specific (use `prop_assume!` to narrow the generator)

### Shrinking is too slow

Possible causes:
- The test function is slow (each shrink step re-runs the test)
- `max_shrink_steps` too high (reduce it)
- Deeply nested structures (reduce nesting depth in the generator)

Fix: reduce test function cost, or accept a non-minimal case and add the regression test.

### Shrinking produces an input that doesn't reproduce the failure

This is a bug in proptest's shrinker. Report it at https://github.com/AltSysrq/proptest

### The shrunk case is too complex to understand

The input type may be too large for meaningful shrinking. Simplify the generator:

```rust
// Too complex for shrinking: deeply nested structures
any::<Vec<HashMap<String, Vec<Option<Result<i32, String>>>>>>()

// Better: use a simpler type and map to the complex type
any::<Vec<(String, i32)>>().prop_map(|v| /* convert */)
```

---

## Checklist for debugging a failed property

- [ ] Read the "Shrunk to" line for the minimal failing case
- [ ] Create a regression test with the shrunk input
- [ ] Run with `--nocapture` to see debug output
- [ ] Trace through the code with the shrunk input
- [ ] Fix the bug
- [ ] Verify the property test passes
- [ ] Keep the regression test as permanent documentation
