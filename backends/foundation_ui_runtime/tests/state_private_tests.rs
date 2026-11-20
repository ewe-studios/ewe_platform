use foundation_ui_runtime::state::{StateCodec, StateError};
use foundation_ui_wire::{read_signed_state, WireLimits};

use super::support::{codec, session, state, test_ctx, EXPECT, WALL};

#[test]
fn private_state_hides_body() {
    let (ctx, _) = test_ctx(WALL);
    let codec = codec();
    let session = session();
    let mut state = state();
    state.private = true;
    state.body = b"secret-row-id-0001".to_vec();

    let sealed = codec.seal(&ctx, &session, &state).unwrap();
    assert!(!sealed.windows(8).any(|window| window == b"secret-r"));
    let view = read_signed_state(&sealed, &WireLimits::default()).unwrap();
    assert_eq!(view.flags(), 1);
    assert_eq!(view.body().unwrap().len(), 12 + state.body.len() + 16);
    let opened = codec.open(&ctx, &session, &sealed, EXPECT).unwrap();
    assert_eq!(opened.body, state.body);
}

#[test]
fn bad_tag_on_private_state_is_bad_tag_not_decrypt() {
    let (ctx, _) = test_ctx(WALL);
    let codec = codec();
    let session = session();
    let mut state = state();
    state.private = true;
    let mut sealed = codec.seal(&ctx, &session, &state).unwrap();
    let view = read_signed_state(&sealed, &WireLimits::default()).unwrap();
    let body = view.body().unwrap().bytes();
    let offset = body.as_ptr().addr() - sealed.as_ptr().addr();
    sealed[offset + 12] ^= 1;
    assert_eq!(
        codec.open(&ctx, &session, &sealed, EXPECT),
        Err(StateError::BadTag)
    );
}
