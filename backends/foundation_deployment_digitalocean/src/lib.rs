//! DigitalOcean API v2 client — droplet provisioning, SSH keys, and hardening.
//!
//! **WHY:** the second of spec-56's three VPS providers. Hetzner proved the shape;
//! DigitalOcean tests whether it transfers.
//!
//! **WHAT:** hand-written domain logic (`client`, `types`, `droplet_ops`,
//! `deployable`) over an auto-generated API surface in `generated/`.
//!
//! **HOW:** `generated/` carries **six endpoints out of DigitalOcean's 447** —
//! the spec covers the whole platform (Kubernetes, Spaces, databases, App
//! Platform) and we need droplets. Regenerate with:
//!
//! ```text
//! cargo run --bin genapi --features cli -- generate digitalocean \
//!     --spec-version 2.0 --api-version v2 \
//!     --include-operation droplets_create --include-operation droplets_get \
//!     --include-operation droplets_list --include-operation droplets_destroy \
//!     --include-operation sshKeys_list --include-operation sshKeys_create
//! ```

pub mod generated;
