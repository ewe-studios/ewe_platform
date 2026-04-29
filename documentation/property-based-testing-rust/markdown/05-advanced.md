---
title: Advanced Techniques and CI Integration
section: 05
---

# Advanced Techniques and CI Integration

## Parallel property testing

Property tests are independent and run in parallel by default with `cargo test`:

```bash
# Run all tests in parallel (default)
cargo test

# Control the number of parallel test threads
cargo test -- --test-threads=4

# Run a single test (useful for debugging)
cargo test -- --test-threads=1 my_test
```

## Increasing test coverage in CI

Run more cases in CI than locally:

```rust
// src/lib.rs or tests/property_tests.rs
fn get_config() -> ProptestConfig {
    let mut config = ProptestConfig::default();
    if cfg!(CI) {
        config.cases = 1024;      // 4x more in CI
        config.max_shrink_steps = 20000;
    }
    config
}

proptest! {
    #![proptest_config(get_config())]

    #[test]
    fn comprehensive_test(ref input in any::<MyType>()) {
        prop_assert!(my_function(&input).is_valid());
    }
}
```

Or set the number of cases via environment variable:

```bash
# In CI config
export PROPTEST_CASES=1024
cargo test
```

## CI configuration examples

### GitHub Actions

```yaml
name: Property Tests
on: [push, pull_request]

jobs:
  property-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable

      - name: Run property tests
        run: |
          # Run with extended cases for important properties
          PROPTEST_CASES=1024 cargo test -- --test-threads=4
```

### GitLab CI

```yaml
property-tests:
  stage: test
  script:
    - cargo test -- --test-threads=4
  variables:
    PROPTEST_CASES: "1024"
  rules:
    - if: '$CI_PIPELINE_SOURCE == "push"'
      variables:
        PROPTEST_CASES: "2048"
    - if: '$CI_PIPELINE_SOURCE == "schedule"'  # Nightly
      variables:
        PROPTEST_CASES: "10000"
```

### Nightly extended runs

Schedule nightly runs with very high case counts to catch rare edge cases:

```yaml
# Nightly extended property tests
property-tests-nightly:
  stage: test
  rules:
    - if: '$CI_PIPELINE_SOURCE == "schedule"'
  script:
    - cargo test -- --test-threads=8
  variables:
    PROPTEST_CASES: "100000"
```

## Integrating with other test frameworks

### Property tests with tokio

Property tests work with async code:

```rust
use proptest::prelude::*;
use tokio::runtime::Runtime;

proptest! {
    #[test]
    fn async_property(ref input in any::<String>()) {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let result = my_async_function(input).await.unwrap();
            prop_assert!(!result.is_empty());
        });
    }
}
```

### Property tests with mockall

Combine property tests with mocked dependencies:

```rust
use mockall::mock;
use proptest::prelude::*;

mock! {
    pub Database {
        fn query(&self, sql: &str) -> Result<Vec<String>, String>;
    }
}

proptest! {
    #[test]
    fn query_returns_valid_results(ref sql in any::<String>()) {
        let mut db = MockDatabase::new();
        db.expect_query()
            .withf(|s| s == sql)
            .returning(|_| Ok(vec!["row1".to_string()]));

        let result = process_query(&db, sql).unwrap();
        prop_assert!(!result.is_empty());
    }
}
```

## Test organization

### Unit-level properties (inline)

Keep properties close to the code they test:

```rust
// src/parser.rs
pub fn parse(input: &str) -> Result<Expr, Error> { ... }

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn parse_roundtrip(ref input in any::<Expr>()) {
            let formatted = format!("{}", input);
            let parsed = parse(&formatted).unwrap();
            prop_assert_eq!(parsed, input);
        }
    }
}
```

### Integration-level properties (tests/ directory)

Keep properties that test multiple modules together:

```rust
// tests/property_tests.rs
use proptest::prelude::*;
use my_crate::{parse, validate, execute};

proptest! {
    #[test]
    fn parse_validate_execute_pipeline(ref input in any::<Source>()) {
        let ast = parse(&input.code).unwrap();
        let validated = validate(&ast).unwrap();
        let result = execute(&validated).unwrap();
        prop_assert!(result.is_ok());
    }
}
```

### Separate test modules

Group related properties in separate files:

```
tests/
├── properties/
│   ├── mod.rs
│   ├── parsing.rs      // Round-trip and parsing invariants
│   ├── validation.rs   // Constraint satisfaction
│   └── execution.rs    // Execution correctness
```

```rust
// tests/properties/mod.rs
pub mod parsing;
pub mod validation;
pub mod execution;
```

## Performance considerations

### Generator performance

Complex generators can dominate test time. Profile with:

```bash
cargo test -- --test-threads=1 my_test
```

If generation is slow:

- **Avoid `prop_filter` with low acceptance rates**: Use targeted generators instead
- **Limit collection sizes**: `vec(any::<i32>(), 0..10)` instead of `vec(any::<i32>(), 0..1000)`
- **Cache generators**: Don't rebuild the same generator on each iteration

```rust
// Bad: rebuilds on every call
fn gen() -> impl Strategy<Value = Vec<i32>> {
    prop::collection::vec(any::<i32>(), 0..100)
}

// Good: cached
fn gen() -> &'static BoxedStrategy<Vec<i32>> {
    static CACHE: LazyLock<BoxedStrategy<Vec<i32>>> = LazyLock::new(|| {
        prop::collection::vec(any::<i32>(), 0..100).boxed()
    });
    &CACHE
}
```

### Test function performance

If the property test body is slow (e.g., involves I/O or complex computation):

- **Reduce cases locally**: 256 cases is enough for most tests
- **Use `-j` for parallelism**: `cargo test -j4`
- **Cache expensive setup**: Move setup outside the property test

## Excluding properties from fast builds

Skip expensive properties during rapid development:

```rust
#[cfg(not(feature = "slow-tests"))]
const DEFAULT_CASES: usize = 64;

#[cfg(feature = "slow-tests")]
const DEFAULT_CASES: usize = 10000;

proptest! {
    #![proptest_config(ProptestConfig {
        cases: DEFAULT_CASES,
        ..Default::default()
    })]

    #[test]
    fn expensive_property(ref input in any::<LargeStruct>()) {
        // ...
    }
}
```

Run with slow tests:

```bash
cargo test --features slow-tests
```

Or use `#[ignore]` for properties that need a long time:

```rust
proptest! {
    #[test]
    #[ignore]  // Skip by default; run with -- --ignored
    fn very_slow_property(ref input in any::<LargeStruct>()) {
        #![proptest_config(ProptestConfig {
            cases: 100000,
            ..Default::default()
        })]
        // ...
    }
}
```

## Seeded property testing for reproducibility

Fix the random seed for deterministic runs:

```bash
# Run with a specific seed
PROPTEST_RNG_SEED=1234567890abcdef cargo test my_test
```

This reproduces the exact same sequence of inputs, useful for debugging intermittent failures.

## Coverage-guided property testing

Combine property tests with `cargo-llvm-cov` for coverage analysis:

```bash
cargo llvm-cov test --property-tests
```

This shows which code paths are covered by property tests vs. unit tests.

---

## Checklist for production property tests

- [ ] Tests run on stable Rust (no nightly required)
- [ ] Test cases are configurable (local: 256, CI: 1024, nightly: 10000)
- [ ] Tests are organized by module (unit properties near code, integration properties in tests/)
- [ ] Slow tests are gated by `#[ignore]` or feature flags
- [ ] CI is configured to run property tests on every push
- [ ] Nightly CI runs extended property tests with high case counts
- [ ] Seeds are saved for reproducible failures
- [ ] Regression tests are added for any discovered bugs
