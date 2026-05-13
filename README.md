# Platform

A series of crates providing different functionality and binaries for projects.

## Test Structure

This project uses Rust's idiomatic test organization:

### Unit Tests
- Located within each crate's `src/` directory using `#[cfg(test)]`
- Run with: `cargo test -p <crate-name>`

### Integration Tests
- Located in each crate's `tests/` directory
- Tests that verify a single crate's functionality
- Run with: `cargo test -p <crate-name>`

**Key Crates with Integration Tests:**
- `backends/foundation_core/tests/` - Core HTTP, WebSocket, event source tests
- `backends/foundation_nostd/tests/` - WASM-compatible synchronization primitive tests  
- `backends/foundation_macros/tests/` - Proc-macro tests
- `backends/foundation_testing/tests/` - Testing infrastructure self-tests

### Cross-Crate Integration Tests
- Located in `tests/` directory (ewe_platform_tests crate)
- Tests that verify interaction between multiple crates
- Run with: `cargo test -p ewe_platform_tests`

### Why This Structure?

Tests were migrated from a centralized `tests/` crate into their respective crates to:
- Follow Rust best practices (tests live with the code they test)
- Eliminate circular dependency concerns
- Make dev-dependencies work correctly (dev-deps only activate when testing the declaring crate)
- Improve discoverability and maintainability

## Running Tests

```bash
# All tests
cargo test --workspace

# Specific crate
cargo test -p foundation_core
cargo test -p foundation_nostd
cargo test -p foundation_testing
cargo test -p foundation_macros

# With specific features
cargo test -p foundation_core --features std,multi
```

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

## Binaries

See the [bin](./bin) directory

## Crates

See the [crates](./crates) and [backend](./backends) directories.


## Project Templates

See the [templates](./templates) directory.
