//! The optional `Env` typed entry of [`Ctx`](crate::Ctx).
//!
//! `Ctx` carries three engine-minimum elements: the clock, the config and the
//! seeded random number generator. This module adds the three remaining ambient
//! effects in one optional entry:
//!
//! - the [`timer`], which waits until a deadline on the clock of the `Ctx`;
//! - the [`entropy`] source, which gives random bytes for keys and tokens;
//! - the [`ids`] source, which gives new sortable IDs.
//!
//! One bundle gives one `get` and one error. A `Ctx::get::<T>()` keys on one
//! `TypeId`, so a trait object cannot be an entry on its own.
//!
//! The `Env` holds no clock. The clock is an engine-minimum element of `Ctx`
//! (spec 76), and a second clock would break the deterministic simulation law
//! "one clock per run". The `Env` also holds no `Ctx`, so the map cannot close
//! an `Arc` cycle.

pub mod entropy;
pub mod ids;
pub mod timer;

use std::sync::Arc;

use derive_more::{Display, Error};

pub use entropy::{EntropySource, EntropySourceExt, OsEntropy, SeededEntropy};
pub use ids::{CounterSource, IdSource, Scru128Source};
pub use timer::{Timer, TimerFuture};

/// The side-effect sources that are not engine-minimum elements of
/// [`Ctx`](crate::Ctx).
///
/// Clone is cheap: each field is an `Arc`. The `Ctx` holds one `Env` as a typed
/// entry.
#[derive(Clone)]
pub struct Env {
    timer: Arc<dyn Timer>,
    entropy: Arc<dyn EntropySource>,
    ids: Arc<dyn IdSource>,
}

impl Env {
    /// An `Env` from three sources.
    #[must_use]
    pub fn new(
        timer: Arc<dyn Timer>,
        entropy: Arc<dyn EntropySource>,
        ids: Arc<dyn IdSource>,
    ) -> Self {
        Self {
            timer,
            entropy,
            ids,
        }
    }

    /// The timer.
    #[must_use]
    pub fn timer(&self) -> Arc<dyn Timer> {
        Arc::clone(&self.timer)
    }

    /// The entropy source.
    #[must_use]
    pub fn entropy(&self) -> Arc<dyn EntropySource> {
        Arc::clone(&self.entropy)
    }

    /// The ID source.
    #[must_use]
    pub fn ids(&self) -> Arc<dyn IdSource> {
        Arc::clone(&self.ids)
    }

    /// A copy with a different timer. The other two members stay shared.
    #[must_use]
    pub fn with_timer(self, timer: Arc<dyn Timer>) -> Self {
        Self { timer, ..self }
    }

    /// A copy with a different entropy source. The other two members stay
    /// shared.
    #[must_use]
    pub fn with_entropy(self, entropy: Arc<dyn EntropySource>) -> Self {
        Self { entropy, ..self }
    }

    /// A copy with a different ID source. The other two members stay shared.
    #[must_use]
    pub fn with_ids(self, ids: Arc<dyn IdSource>) -> Self {
        Self { ids, ..self }
    }
}

impl core::fmt::Debug for Env {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Env").finish_non_exhaustive()
    }
}

/// Why an `Env` is not available.
#[derive(Debug, Clone, PartialEq, Eq, Display, Error)]
pub enum EnvError {
    /// The context holds no `Env`. The host did not install one.
    #[display("the context holds no Env (the host did not install one)")]
    Missing,
    /// A real source failed while the host built the `Env`.
    #[display("the effect source failed: {_0}")]
    Source(#[error(not(source))] String),
}

impl EnvError {
    /// The error of a failed real source, as `EnvError::Source`.
    #[must_use]
    pub fn source(error: &impl core::fmt::Display) -> Self {
        Self::Source(error.to_string())
    }
}
