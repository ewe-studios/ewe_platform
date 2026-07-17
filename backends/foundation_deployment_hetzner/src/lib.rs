//! Hetzner Cloud API v1 client — VPS provisioning, SSH keys, and hardening.
//!
//! **WHY:** spec-56 needs a VPS it can create, harden and destroy from code, and
//! Hetzner is the smallest API of the three that does it: one `POST /servers`
//! creates a box, one `DELETE /servers/{id}` removes it.
//!
//! **WHAT:** hand-written domain logic (`client`, `types`, `server_ops`,
//! `deployable`) over an auto-generated API surface in `generated/`.
//!
//! **HOW:** `generated/` carries **six endpoints out of Hetzner's 151**, cut by
//! the declaration in `build_spec.rs` and regenerated with:
//!
//! ```text
//! cargo run --bin genapi --features cli -- generate hetzner \
//!     --spec-version 1.0.0 --api-version v1 \
//!     --include-operation create_server --include-operation get_server \
//!     --include-operation list_servers --include-operation delete_server \
//!     --include-operation list_ssh_keys --include-operation create_ssh_key
//! ```
//!
//! The API version (`v1`) is pinned into the client — callers never pass it — and
//! the spec revision (`1.0.0`) is validated at generation, so a Hetzner
//! republish fails loudly rather than silently changing our types.

pub mod generated;
