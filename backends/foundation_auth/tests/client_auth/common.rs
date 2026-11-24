//! Shared helpers for the provider tests.

use std::sync::Arc;

use foundation_auth::client_auth::{AuthError, AuthFuture, AuthInfo, CallCredentials, ClientAuth};
use foundation_auth::trust::{MemoryTrustStore, TrustConfig, TrustedClients};
use foundation_auth::AuthContext;
use foundation_context::{Clock, Ctx};
use foundation_core::valtron::SimulatedClock;
use foundation_netio::shared::context::ConnectionContext;
use foundation_netio::shared::http::{Extensions, SimpleHeader, SimpleHeaders};
use foundation_testing::dst::simulated_env;

/// The audience of the test host.
pub const AUDIENCE: &str = "https://agents.example.com/rpc";
/// The seed of the deterministic test signing key.
pub const TEST_SEED: [u8; 32] = [0x33; 32];

/// A context whose wall clock is `secs` after the UNIX epoch, with a seeded
/// entropy source.
pub fn ctx_at(secs: u64) -> (Ctx, Arc<SimulatedClock>) {
    let clock = Arc::new(SimulatedClock::new(std::time::Duration::ZERO));
    clock.advance_wall(std::time::Duration::from_secs(secs));
    let ctx = Ctx::fixed(Arc::clone(&clock) as Arc<dyn Clock>);
    ctx.store(simulated_env(&ctx, 81));
    (ctx, clock)
}

/// An empty trust store in memory.
pub async fn trusted(ctx: &Ctx) -> Arc<TrustedClients> {
    TrustedClients::open(
        Arc::new(MemoryTrustStore::new()),
        ctx,
        TrustConfig::default(),
    )
    .await
    .expect("the memory trust store opens")
}

/// Headers with `Authorization: Bearer <token>`.
pub fn bearer_headers(token: &str) -> SimpleHeaders {
    let mut headers = SimpleHeaders::new();
    headers.insert(SimpleHeader::AUTHORIZATION, vec![format!("Bearer {token}")]);
    headers
}

/// A call with no headers, no peer and no extensions.
pub fn call<'a>(
    headers: &'a SimpleHeaders,
    connection: &'a ConnectionContext,
    extensions: &'a Extensions,
    procedure: &'a str,
) -> CallCredentials<'a> {
    CallCredentials {
        procedure,
        headers,
        connection,
        extensions,
    }
}

/// What a fixed provider returns.
#[derive(Clone)]
pub enum Outcome {
    /// `Ok(Some(info))`.
    Accept,
    /// `Ok(None)`.
    Skip,
    /// `Err(error)`.
    Fail(AuthError),
}

/// A provider with a fixed outcome, for the composite tests.
pub struct Fixed {
    mode: &'static str,
    outcome: Outcome,
}

impl Fixed {
    /// A provider named `mode` with `outcome`.
    #[must_use]
    pub fn new(mode: &'static str, outcome: Outcome) -> Self {
        Self { mode, outcome }
    }
}

impl ClientAuth for Fixed {
    fn mode(&self) -> &'static str {
        self.mode
    }

    fn authenticate<'a>(&'a self, _call: CallCredentials<'a>) -> AuthFuture<'a> {
        let outcome = self.outcome.clone();
        Box::pin(async move {
            match outcome {
                Outcome::Accept => Ok(Some(AuthInfo::new(AuthContext::new(
                    String::new(),
                    None,
                    None,
                )))),
                Outcome::Skip => Ok(None),
                Outcome::Fail(error) => Err(error),
            }
        })
    }
}
