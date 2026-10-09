//! Credential provisioning — app registry + SSH key generation (spec-57, feature 009).
//!
//! WHY: turns the vault into a service apps can call to auto-provision SSH keys.
//! Native-only: app secrets are Argon2id-hashed (`foundation_auth`, native-only)
//! and private keys are `age`-encrypted at rest.
//!
//! WHAT: [`keygen`] (Ed25519/RSA generation + age encrypt/decrypt), [`store`]
//! (apps + ssh_keys persistence), [`apps`]/[`ssh_keys`] (handlers).

pub mod apps;
pub mod keygen;
pub mod ssh_keys;
pub mod store;
