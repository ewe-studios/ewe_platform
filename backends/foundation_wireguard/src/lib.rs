//! `foundation_wireguard` — a userspace WireGuard® mesh (spec-55).
//!
//! WHY: Let any EWE service stand up a private, encrypted mesh from a single bootstrap
//! secret and let other services join it — over WireGuard, entirely in userspace (no
//! kernel module, no privileges in the default mode), across native/wasm/mobile.
//!
//! WHAT: This crate is the **top combiner**. It wraps `boringtun`'s Noise state machine
//! ([`shared::tunnel`]), derives keys from a seed ([`shared::keys`]), and drives tunnels
//! over the [`foundation_nativeapis`] data plane. Bootstrap, SWIM gossip, mesh
//! orchestration, relay, and the config surface layer on top in later features.
//!
//! HOW: Sans-I/O cores (`Tunn`, smoltcp, SWIM) are *driven*, never self-running — a
//! single valtron task couples `Tunn` timers with the data-plane poll loop. No tokio.

/// Cross-platform building blocks (keys, tunnel, membership, mesh, config). No
/// platform-specific re-exports live here (see `feedback_shared_module_purpose`).
pub mod shared;

/// Completeness gate — emits `compile_error!` for every known spec-55 gap
/// when `feature = "spec55-complete"` is enabled. Delete the corresponding
/// `compile_error!` line as each gap is fixed.
pub mod completeness;

/// Native-only glue: UDP transport and the tunnel driver task.
#[cfg(not(target_family = "wasm"))]
pub mod native;

/// Browser/WASM build (spec-55, F07). Always compiled — types and state machines
/// work on any target; actual I/O is gated on `target_family = "wasm"`.
pub mod wasm;

// Macro re-export (foundation_macros → foundation_wireguard, same pattern as proxy).
pub use foundation_macros::wireguard;
pub use foundation_macros::wireguard_main;

pub use shared::bootstrap::{BootstrapFlags, BootstrapToken, TokenSecret, WgBootstrap};
pub use shared::config::{
    DataPlaneConfig, DataPlaneMode, NetworkConfig, NodeConfig, RelayConfig, SecurityConfig,
    WgConfig, WgConfigBuilder,
};
pub use shared::error::{WgError, WgResult};
pub use shared::keys::{
    BootstrapKeys, IdentityKeypair, NetworkId, PeerPublicKey, SeedBits, WgSeed,
};
pub use shared::membership::{
    Capabilities, MemberState, Membership, PeerId, PeerRecord, Swim, SwimConfig, SwimEvent,
    SwimMessage, SwimOutbound,
};
pub use shared::tunnel::{WgOutcome, WgTunnel};
