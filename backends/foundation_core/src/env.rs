//! The host builders for the effect environment (spec 84).
//!
//! [`Env`] lives in `foundation_context`, because the timer trait must sit below
//! `foundation_core`. This module builds the real bundles and stores them in a
//! [`Ctx`]. Only a host entry point calls these functions.
//!
//! A host builds the clock first, then the `Ctx`, then the `Env`:
//!
//! ```
//! # use std::sync::Arc;
//! # use foundation_context::{Ctx, MonotonicClock};
//! # use foundation_core::env;
//! # fn main() -> Result<(), foundation_context::env::EnvError> {
//! let ctx = Ctx::fixed(Arc::new(MonotonicClock::new()));
//! let env = env::install(&ctx, env::system)?;
//! # let _ = env;
//! # Ok(())
//! # }
//! ```
//!
//! The real bundles use valtron's [`ValtronTimer`], so they compile on native and
//! on wasm.

use std::sync::Arc;

use foundation_context::{Ctx, Instant};

pub use foundation_context::env::{
    CounterSource, EntropySource, Env, EnvError, IdSource, OsEntropy, Scru128Source, SeededEntropy,
    Timer, TimerFuture,
};

/// The real timer: valtron's sleep on the clock of the given [`Ctx`].
///
/// The timer reads no clock. Valtron compares `deadline` with the clock of the
/// `Ctx` that the caller passes, so one timer works on real time and on
/// simulated time. The deterministic simulation harness moves the clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct ValtronTimer;

impl Timer for ValtronTimer {
    fn sleep_until(&self, ctx: Ctx, deadline: Instant) -> TimerFuture {
        Box::pin(async move {
            #[cfg(all(feature = "std", not(target_family = "wasm")))]
            crate::valtron::sleep_until_async(deadline, &ctx).await;

            // A single-threaded wasm host has no threads, so the `!Send` valtron
            // task future is safe to assert as `Send` here (spec 84, section
            // 6.6). The worker runs the timer on the JavaScript event loop.
            #[cfg(target_family = "wasm")]
            foundation_compact::SendWrapper::new(crate::valtron::sleep_until_async(deadline, &ctx))
                .await;
        })
    }
}

/// Build an `Env` from a timer and an entropy source.
fn from_sources(
    ctx: &Ctx,
    timer: Arc<dyn Timer>,
    entropy: Arc<dyn EntropySource>,
) -> Result<Env, EnvError> {
    let ids = Scru128Source::new(ctx.clock(), entropy.as_ref())
        .map_err(|error| EnvError::Source(error.to_string()))?;
    Ok(Env::new(timer, entropy, Arc::new(ids)))
}

/// The real effects of a native host.
///
/// # Errors
/// [`EnvError::Source`] when the operating-system entropy source fails.
pub fn system(ctx: &Ctx) -> Result<Env, EnvError> {
    from_sources(ctx, Arc::new(ValtronTimer), Arc::new(OsEntropy))
}

/// The real effects of a Worker or a browser page.
///
/// The entropy source selects its backend at compile time: `WebCrypto` on
/// wasm32-unknown-unknown, the operating system on native.
///
/// # Errors
/// [`EnvError::Source`] when the `WebCrypto` entropy source fails.
pub fn wasm(ctx: &Ctx) -> Result<Env, EnvError> {
    from_sources(ctx, Arc::new(ValtronTimer), Arc::new(OsEntropy))
}

/// The effects of a simulation. The same `ctx` clock and the same `seed` replay
/// the run.
///
/// # Errors
/// [`EnvError::Source`] when the seeded source cannot be built. A seeded source
/// does not fail, so this error exists for a uniform builder surface.
pub fn simulated(ctx: &Ctx, seed: u64) -> Result<Env, EnvError> {
    from_sources(
        ctx,
        Arc::new(ValtronTimer),
        Arc::new(SeededEntropy::new(seed)),
    )
}

/// Stores the result of `build` in `ctx` and returns it.
///
/// # Errors
/// The error of the builder that `build` calls.
pub fn install(ctx: &Ctx, build: fn(&Ctx) -> Result<Env, EnvError>) -> Result<Env, EnvError> {
    let env = build(ctx)?;
    ctx.store(env.clone());
    Ok(env)
}

/// Converts an `env = ...` macro expression into an [`Env`].
///
/// The macros accept a ready `Env` or a `Result<Env, E>`. A failed `Result`
/// stops the program before the pool starts, with the error in the message.
#[doc(hidden)]
pub trait IntoEnv {
    /// The `Env`, or a panic that names the failure.
    fn into_env(self) -> Env;
}

impl IntoEnv for Env {
    fn into_env(self) -> Env {
        self
    }
}

impl<E: core::fmt::Display> IntoEnv for Result<Env, E> {
    fn into_env(self) -> Env {
        match self {
            Ok(env) => env,
            Err(error) => {
                panic!("the host could not build the effect environment: {error}")
            }
        }
    }
}
