use foundation_ui_runtime::state::{StateError, StateKey, StateKeyRing};

#[test]
fn keys_require_a_positive_unique_id_and_separate_secrets() {
    assert!(matches!(
        StateKey::new(0, [1; 32], [2; 32]),
        Err(StateError::BadKey)
    ));
    assert!(matches!(
        StateKey::new(1, [1; 32], [1; 32]),
        Err(StateError::BadKey)
    ));
    let ring = StateKeyRing::new(StateKey::new(1, [1; 32], [2; 32]).unwrap());
    assert_eq!(ring.current_id(), 1);
    assert!(matches!(
        ring.with_previous(StateKey::new(1, [3; 32], [4; 32]).unwrap()),
        Err(StateError::BadKey),
    ));
}

use foundation_ui_runtime::state::{HmacStateCodec, StateCodec, StateConfig};
use foundation_ui_wire::{read_signed_state, WireLimits};

use super::support::{codec, session, state, test_ctx, ENC, EXPECT, MAC, WALL};

#[test]
fn unknown_key_is_rejected() {
    let (ctx, _) = test_ctx(WALL);
    let unknown = HmacStateCodec::new(
        StateKeyRing::new(StateKey::new(9, MAC, ENC).unwrap()),
        StateConfig::default(),
    );
    let sealed = unknown.seal(&ctx, &session(), &state()).unwrap();
    assert_eq!(
        codec().open(&ctx, &session(), &sealed, EXPECT),
        Err(StateError::UnknownKey)
    );
}

#[test]
fn previous_key_still_opens_and_new_seal_uses_current() {
    let (ctx, _) = test_ctx(WALL);
    let mut state = state();
    state.private = true;
    let sealed = codec().seal(&ctx, &session(), &state).unwrap();
    let next = StateKeyRing::new(StateKey::new(2, [3; 32], [4; 32]).unwrap())
        .with_previous(StateKey::new(1, MAC, ENC).unwrap())
        .unwrap();
    let rotated = HmacStateCodec::new(next, StateConfig::default());
    let opened = rotated.open(&ctx, &session(), &sealed, EXPECT).unwrap();
    assert_eq!(opened.body, state.body);
    let resealed = rotated.seal(&ctx, &session(), &state).unwrap();
    let view = read_signed_state(&resealed, &WireLimits::default()).unwrap();
    assert_eq!(view.key_id(), 2);
}
