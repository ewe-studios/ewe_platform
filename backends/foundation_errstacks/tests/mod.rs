// Integration test entry point for foundation_errstacks.
//
// Per the Rust clean-code testing skill, all tests live under `tests/`
// (never inline `#[cfg(test)] mod tests` in `src/`). This file acts as
// the single integration-test binary and includes the per-module test
// files from `tests/units/`.

mod units;
