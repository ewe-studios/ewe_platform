use foundation_ui_runtime::state::{PlainStateCodec, StateCodec, StateError};
use foundation_ui_wire::{read_signed_state, WireLimits};

use super::support::{codec, session, state, test_ctx, EXPECT, WALL};

#[test]
fn plain_codec_round_trip() {
    let (ctx, _) = test_ctx(WALL);
    let codec = PlainStateCodec::default();
    let session = session();
    let state = state();
    let sealed = codec.seal(&ctx, &session, &state).unwrap();
    let view = read_signed_state(&sealed, &WireLimits::default()).unwrap();
    assert!(view.tag().unwrap().is_empty());
    assert!(view.session_binding().unwrap().is_empty());
    assert_eq!(view.key_id(), 0);
    assert_eq!(view.flags(), 0);
    let opened = codec.open(&ctx, &session, &sealed, EXPECT).unwrap();
    assert_eq!(opened.body, state.body);
    assert_eq!(opened.issued_at, WALL);
    assert_eq!(opened.expires_at, 0);
}

#[test]
fn plain_codec_rejects_tagged_state() {
    let (ctx, _) = test_ctx(WALL);
    let sealed = codec().seal(&ctx, &session(), &state()).unwrap();
    assert!(matches!(
        PlainStateCodec::default().open(&ctx, &session(), &sealed, EXPECT),
        Err(StateError::Malformed { .. })
    ));
}
