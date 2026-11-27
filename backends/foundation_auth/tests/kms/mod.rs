//! Spec 83 feature 08: envelope encryption with a remote key management
//! service (KMS).

mod aws_live;
mod common;
mod gcp_live;
mod kms_envelope;
mod kms_vault_transit;

// The specification and CI select this test with the filter `kms::aws_local`.
// The crate root of this test binary is the `kms` directory, so this nested
// module adds the `kms` path segment without moving the test file.
mod kms;
