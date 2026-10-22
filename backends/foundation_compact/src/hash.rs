//! Deterministic, wasm-safe content hashing (foldhash).
//!
//! WHY: several crates need a non-cryptographic `Hasher` for content
//! fingerprints — tool-call loop detection, embedding cache keys, SimHash,
//! bloom filters. The common choice, `ahash`, pulls in `getrandom 0.3` through
//! its default `runtime-rng` feature, and `getrandom 0.3` does not compile on
//! `wasm32-unknown-unknown` without an extra backend feature — a hard build
//! break for any crate that is otherwise wasm-clean. `foldhash` needs no
//! `getrandom`/`rand`, is `#![no_std]`-compatible, deterministic by default,
//! and fast on wasm (it avoids AES, which wasm lacks).
//!
//! WHAT: a [`Hasher`] newtype over foldhash's **quality** variant
//! ([`foldhash::quality::FoldHasher`]), seeded from
//! [`foldhash::quality::FixedState`] so `Hasher::default()` is deterministic
//! (equal inputs hash equal within a process), plus re-exports of foldhash's
//! [`FixedState`]/[`RandomState`] for `HashMap`/`HashSet` use.
//!
//! WHY quality over `foldhash::fast`: SimHash (used by `foundation_ai`'s loop
//! detector) relies on the *statistical* properties of per-token hashes —
//! foldhash's own docs recommend the `quality` variant for such algorithms,
//! while `fast` "has known statistical imperfections" and is meant for hash
//! tables. For the short strings hashed here the speed difference is
//! negligible, so the correct-for-all variant wins.
//!
//! HOW: foldhash is a normal dependency (not vendored). This module only adds
//! the `Default`-able wrapper the call sites need, so downstream crates can
//! swap `ahash::AHasher::default()` for [`Hasher::default()`] without also
//! taking a direct foldhash dependency.

use core::hash::{BuildHasher, Hasher as CoreHasher};

/// A deterministic, wasm-safe, non-cryptographic hasher (foldhash `quality`).
///
/// Drop-in replacement for `ahash::AHasher::default()` at content-fingerprint
/// sites. `Hasher::default()` uses a fixed seed, so equal inputs always hash
/// equal within a process — which is exactly what content hashing wants.
///
/// This is **not** a cryptographic hash: do not use it for MACs, key
/// derivation, password storage, or anything security-sensitive, and do not
/// persist its output across foldhash versions or platforms (foldhash makes
/// no cross-version stability guarantee).
#[derive(Clone)]
pub struct Hasher(foldhash::quality::FoldHasher<'static>);

impl Default for Hasher {
    #[inline]
    fn default() -> Self {
        Self(foldhash::quality::FixedState::default().build_hasher())
    }
}

impl CoreHasher for Hasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.0.finish()
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        self.0.write(bytes);
    }
}

/// Deterministic [`BuildHasher`] for maps/sets that want a fixed seed
/// (identical layout across runs within the same foldhash version).
pub use foldhash::quality::FixedState;

/// Randomly-seeded [`BuildHasher`] for maps/sets that want a per-instance
/// seed (DoS-resistant, default choice for user-controlled keys).
pub use foldhash::quality::RandomState;

/// The underlying foldhash crate, re-exported for the full API
/// (`fast`, `quality`, `SharedSeed`, …).
pub use foldhash;
