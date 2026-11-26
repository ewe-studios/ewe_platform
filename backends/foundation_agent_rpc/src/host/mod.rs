//! The hosts that run the agent-rpc core on one platform.
//!
//! WHY: the core is the same on every platform. A host gives the core its side
//! effects: time, randomness, the network, storage and models.
//!
//! WHAT: the shared host types and, per target, the host module: the native
//! host ([`native`]) and the Cloudflare Worker host ([`worker`]).
//!
//! HOW: a host module is built only on its target and with its feature. The
//! native host needs threads, files and Transport Layer Security (TLS). The
//! Worker host needs `worker`, WebAssembly (wasm) and its own bindings.

#[cfg(all(feature = "multi", not(target_family = "wasm")))]
pub mod native;

#[cfg(all(feature = "worker", target_family = "wasm"))]
pub mod worker;
