use foundation_compact::rng::Rng;
use foundation_context::{Ctx, MinimumElement};
use foundation_ui_runtime::state::{Session, StateCodec, StateError};

use super::support::{codec, session, state, test_ctx, EXPECT, TTL, WALL};

#[test]
fn sealed_state_opens_for_same_session() {
    let (ctx, _clock) = test_ctx(WALL);
    let session = session();
    let codec = codec();
    let state = state();
    let sealed = codec.seal(&ctx, &session, &state).unwrap();
    let opened = codec.open(&ctx, &session, &sealed, EXPECT).unwrap();
    assert_eq!(opened.format_version, state.format_version);
    assert_eq!(opened.component_type, state.component_type);
    assert_eq!(opened.root_node, state.root_node);
    assert_eq!(opened.body, state.body);
    assert_eq!(opened.issued_at, WALL);
    assert_eq!(opened.expires_at, WALL + TTL);
}

#[test]
fn round_trip_holds_for_random_bodies() {
    let (ctx, clock) = test_ctx(WALL);
    let codec = codec();
    let session = session();
    for sample in 0..100 {
        let body = {
            let rng = ctx.rng();
            let mut rng = rng.lock().unwrap();
            let length = match sample {
                0 => 0,
                1 => 4096,
                _ => usize::try_from(rng.next_u32() % 4097).unwrap(),
            };
            let mut body = vec![0; length];
            rng.fill_bytes(&mut body);
            body
        };
        let mut state = state();
        state.body = body;
        state.private = sample % 2 == 0;
        clock.set_elapsed(0);
        let sealed = codec.seal(&ctx, &session, &state).unwrap();
        for elapsed in [0, TTL - 1] {
            clock.set_elapsed(elapsed);
            let opened = codec.open(&ctx, &session, &sealed, EXPECT).unwrap();
            assert_eq!(opened.body, state.body);
        }
    }
}

#[test]
fn large_body_is_rejected_at_seal() {
    let (ctx, _) = test_ctx(WALL);
    let mut state = state();
    state.body.resize(4097, 0);
    assert_eq!(
        codec().seal(&ctx, &session(), &state),
        Err(StateError::BodyTooLarge {
            limit: 4096,
            actual: 4097
        })
    );
}

#[test]
fn seal_is_deterministic_for_a_seed() {
    let (first, _) = test_ctx(WALL);
    let (second, _) = test_ctx(WALL);
    let mut state = state();
    state.private = true;
    let codec = codec();
    let session = session();
    let sealed = codec.seal(&first, &session, &state).unwrap();
    assert_eq!(sealed, codec.seal(&second, &session, &state).unwrap());
    assert_ne!(sealed, codec.seal(&first, &session, &state).unwrap());
}

#[test]
fn missing_injected_sources_return_typed_errors() {
    let (ctx, clock) = test_ctx(WALL);
    let codec = codec();
    let session = session();
    let mut state = state();
    state.private = true;
    let sealed = codec.seal(&ctx, &session, &state).unwrap();

    let no_clock = Ctx::incomplete_for_test(clock.clone(), MinimumElement::Clock);
    assert_eq!(
        codec.seal(&no_clock, &session, &state),
        Err(StateError::MissingEnv { what: "clock" })
    );
    assert_eq!(
        codec.open(&no_clock, &session, &sealed, EXPECT),
        Err(StateError::MissingEnv { what: "clock" })
    );

    let no_rng = Ctx::incomplete_for_test(clock, MinimumElement::Rng);
    assert_eq!(
        codec.seal(&no_rng, &session, &state),
        Err(StateError::MissingEnv { what: "rng" })
    );
    assert!(codec.open(&no_rng, &session, &sealed, EXPECT).is_ok());
}

#[test]
fn session_identifier_is_required() {
    assert_eq!(Session::new(""), Err(StateError::MissingSession));
    assert_eq!(session().id(), "sess-A");
}

#[test]
fn configured_body_limit_preserves_round_trip_at_boundary() {
    use foundation_ui_runtime::state::{HmacStateCodec, StateConfig, StateKey, StateKeyRing};

    use super::support::{ENC, MAC};

    let (ctx, _) = test_ctx(WALL);
    let limit = 1024 * 1024 + 1;
    let config = StateConfig::default().max_body_bytes(limit);
    let make_codec = |config| {
        HmacStateCodec::new(
            StateKeyRing::new(StateKey::new(1, MAC, ENC).unwrap()),
            config,
        )
    };
    let codec = make_codec(config);
    let session = session();
    let mut state = state();
    state.body.resize(limit, 7);
    state.private = true;
    let sealed = codec.seal(&ctx, &session, &state).unwrap();
    assert_eq!(
        codec.open(&ctx, &session, &sealed, EXPECT).unwrap().body,
        state.body
    );
    assert_eq!(
        make_codec(config.max_body_bytes(limit - 1)).open(&ctx, &session, &sealed, EXPECT),
        Err(StateError::BodyTooLarge {
            limit: limit - 1,
            actual: limit
        })
    );
    state.body.push(8);
    assert_eq!(
        codec.seal(&ctx, &session, &state),
        Err(StateError::BodyTooLarge {
            limit,
            actual: limit + 1
        })
    );
}
