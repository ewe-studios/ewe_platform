//! The auth expiry checks read the injected clock, not the real wall clock.
//!
//! WHY: Before spec 70 feature 133, `JwtToken::is_expired` and
//! `Session::is_valid` called the real wall clock directly. A test could not
//! move time, so an expiry boundary was not deterministic.
//!
//! WHAT: Build a token and a session that expire 60 s after a simulated wall
//! time. They are valid before that instant and expired after it. A second
//! test builds a `JwtManager` from a `Ctx` and sees the simulated time move.
//!
//! HOW: `SimulatedClock` starts at the UNIX epoch. `advance_wall` moves the
//! simulated wall clock by hand. The `Ctx` carries the clock, so the code
//! under test reads it through `ctx.clock()`.

use std::sync::Arc;
use std::time::Duration;

use chrono::{TimeZone, Utc};
use foundation_auth::shared::jwt::{JwtManager, JwtToken};
use foundation_auth::shared::session::Session;
use foundation_auth::shared::types::ConfidentialText;
use foundation_compact::time::unix_seconds;
use foundation_context::Ctx;
use foundation_core::valtron::SimulatedClock;

#[test]
fn token_and_session_expiry_follow_the_injected_clock() {
    let simulated = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::fixed(simulated.clone());

    // Pin the simulated wall clock to a known instant (2023-11-14T22:13:20Z).
    simulated.advance_wall(Duration::from_secs(1_700_000_000));
    let now = unix_seconds(ctx.clock().wall_now());

    let token = JwtToken::from_parts(
        "simulated_token".to_string(),
        None,
        now + 60,
        None,
        None,
        None,
        &ctx,
    );
    let created_at = Utc.timestamp_opt(now, 0).single().expect("valid timestamp");
    let expires_at = Utc
        .timestamp_opt(now + 60, 0)
        .single()
        .expect("valid timestamp");
    let session = Session {
        id: "session-1".to_string(),
        user_id: "user-1".to_string(),
        token: ConfidentialText::new("session_token".to_string()),
        created_at,
        expires_at,
        ip_address: None,
        user_agent: None,
        last_active_at: created_at,
        revoked: false,
    };

    // Before the simulated expiry both are valid: 60 s remain.
    assert!(
        !token.is_expired(&ctx),
        "the token must be valid at simulated time {now}"
    );
    assert_eq!(token.expires_in(&ctx), 60);
    assert!(
        session.is_valid(&ctx),
        "the session must be valid at simulated time {now}"
    );

    // Advance the simulated wall clock past the expiry.
    simulated.advance_wall(Duration::from_secs(61));

    assert!(
        token.is_expired(&ctx),
        "the token must be expired at simulated time {}",
        unix_seconds(ctx.clock().wall_now())
    );
    assert_eq!(token.expires_in(&ctx), 0);
    assert!(
        !session.is_valid(&ctx),
        "the session must be expired at simulated time {}",
        unix_seconds(ctx.clock().wall_now())
    );
}

#[test]
fn jwt_manager_built_from_a_ctx_sees_the_advanced_clock() {
    let simulated = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::fixed(simulated.clone());
    simulated.advance_wall(Duration::from_secs(1_700_000_000));
    let now = unix_seconds(ctx.clock().wall_now());

    // The token expires in 120 s. The refresh buffer is 60 s.
    let mut manager = JwtManager::new(ctx.clone()).with_refresh_buffer(60);
    let token = JwtToken::from_parts(
        "manager_token".to_string(),
        None,
        now + 120,
        None,
        None,
        None,
        &ctx,
    );
    manager.set_token(token);
    assert!(
        manager.has_valid_token(),
        "120 s remain, which is outside the 60 s buffer"
    );

    // 59 s remain now, which is inside the buffer. The manager must see it.
    simulated.advance_wall(Duration::from_secs(61));
    assert!(
        !manager.has_valid_token(),
        "59 s remain, which is inside the 60 s buffer"
    );
}
