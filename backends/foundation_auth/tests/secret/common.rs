//! Shared helpers for the signed-secret tests.

use std::sync::Arc;

use foundation_auth::secret::{SecretDomain, SigningKey, VerifyingKey};
use foundation_context::{Clock, Ctx};
use foundation_core::valtron::SimulatedClock;
use foundation_testing::dst::simulated_env;

/// The seed of the deterministic test key.
pub const TEST_SEED: [u8; 32] = [0x5a; 32];

/// A context whose wall clock is `secs` after the UNIX epoch, with a seeded
/// entropy source.
pub fn ctx_at(secs: u64) -> (Ctx, Arc<SimulatedClock>) {
    let clock = Arc::new(SimulatedClock::new(std::time::Duration::ZERO));
    clock.advance_wall(std::time::Duration::from_secs(secs));
    let ctx = Ctx::fixed(Arc::clone(&clock) as Arc<dyn Clock>);
    ctx.store(simulated_env(&ctx, 81));
    (ctx, clock)
}

/// The deterministic test signing key.
pub fn signing_key() -> SigningKey {
    SigningKey::from_seed(TEST_SEED, None)
}

/// The verifying key of `key`.
pub fn verifying_key(key: &SigningKey) -> VerifyingKey {
    key.verifying_key().clone()
}

/// The agent RPC domain.
pub fn domain() -> SecretDomain {
    SecretDomain::AGENT_RPC
}

/// Lowercase hexadecimal of `bytes`.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut out, byte| {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
        out
    })
}
