//! Keeps the test filter `kms::aws_local` that the specification and CI use.
//!
//! The crate root of this test binary is the `kms` directory, so this nested
//! module adds the `kms` path segment. The `#[path]` attribute keeps the test
//! file at `tests/kms/aws_local.rs`.

#[path = "aws_local.rs"]
mod aws_local;
