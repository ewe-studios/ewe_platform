//! Foundation platform sandbox: one core library for every sandbox substrate.
//!
//! Feature 00 is the contract. It holds the domain types, the typed error, the
//! limits, the two remote procedure call (RPC) service definitions and the
//! traits that the drivers, the side effects and the control plane implement.
//! It starts nothing, reads no clock and touches no file.
//!
//! The library code lives in [`shared`]. The modules [`native`], [`cloudflare`]
//! and [`cli`] are boundaries for later features; each one compiles only on its
//! own target and feature. The `slave` feature adds the slave engine to the
//! `native` module: a Linux guest program that serves `SlaveService`.
//!
//! # Determinism
//!
//! Every side effect arrives through [`foundation_context::Ctx`]. Identifiers
//! read the random number generator of the context, so two contexts with the
//! same seed give the same sequence of identifiers.

pub mod shared;

/// Native adapters. Features 01, 03, 04, 05, 09 and 10 fill this module.
#[cfg(not(target_family = "wasm"))]
pub mod native;

/// WebAssembly adapters for a Cloudflare Worker. Feature 07 fills this module.
#[cfg(all(target_family = "wasm", feature = "cloudflare"))]
pub mod cloudflare;

/// The command-line structures. Feature 06 fills this module.
#[cfg(all(not(target_family = "wasm"), feature = "cli"))]
pub mod cli;

/// The `Provider` facades over a control plane (feature 06). The `Provider`
/// trait itself stays in `foundation_deployment_platform` (decision D1).
#[cfg(all(not(target_family = "wasm"), feature = "provider"))]
pub mod providers;
