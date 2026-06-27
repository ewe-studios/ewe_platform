# foundation_testing — Test utilities and fixtures

## What it is
Shared testing infrastructure: mock providers, test fixtures, assertion helpers,
and integration test scaffolding.

## Key modules
- **`fixtures/`** — Pre-built test data for common scenarios.
- **`mocks/`** — Mock implementations of traits (HTTP client, storage providers).
- **`assertions/`** — Custom assertion macros for cleaner test output.

## Usage
```rust
#[cfg(test)]
mod tests {
    use foundation_testing::fixtures::sample_model;
    use foundation_testing::mocks::MockHttpClient;
}
```

## Integration
- Used by `foundation_ai`'s `testing` feature for `MockModelProvider` and
  `MockTool`.
- Shared across all crate test suites for consistent fixture data.
