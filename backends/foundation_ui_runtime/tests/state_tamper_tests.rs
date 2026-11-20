use foundation_ui_runtime::state::{mac_input, Expect, Session, StateCodec, StateError};
use foundation_ui_wire::{finish_signed_state, read_signed_state, SignedStateT, WireLimits};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

use super::support::{codec, session, state, test_ctx, EXPECT, MAC, TTL, WALL};

#[test]
fn mac_input_has_exact_byte_layout() {
    let mut state = SignedStateT::default();
    state.format_version = 0x0201;
    state.flags = 0x03;
    state.key_id = 0x0706_0504;
    state.issued_at = 0x0f0e_0d0c_0b0a_0908;
    state.expires_at = 0x1716_1514_1312_1110;
    state.component_type = 0x1b1a_1918;
    state.root_node = 0x1f1e_1d1c;
    state.session_binding = Some(vec![0xaa, 0xbb]);
    state.body = Some(vec![0xcc, 0xdd, 0xee]);
    let expected = [
        &b"ewe.ui.state.v1"[..],
        &(1_u8..=31).collect::<Vec<_>>(),
        &[2, 0, 0, 0, 0xaa, 0xbb, 3, 0, 0, 0, 0xcc, 0xdd, 0xee],
    ]
    .concat();
    assert_eq!(mac_input(&state), expected);
}

#[test]
fn changed_body_is_rejected() {
    let (ctx, _) = test_ctx(WALL);
    let mut sealed = codec().seal(&ctx, &session(), &state()).unwrap();
    let view = read_signed_state(&sealed, &WireLimits::default()).unwrap();
    let body = view.body().unwrap().bytes();
    let offset = body.as_ptr().addr() - sealed.as_ptr().addr();
    sealed[offset] = 2;
    assert_eq!(
        codec().open(&ctx, &session(), &sealed, EXPECT),
        Err(StateError::BadTag)
    );
}

#[test]
fn other_session_is_rejected() {
    let (ctx, _) = test_ctx(WALL);
    let sealed = codec().seal(&ctx, &session(), &state()).unwrap();
    let other = Session::new("sess-B").unwrap();
    assert_eq!(
        codec().open(&ctx, &other, &sealed, EXPECT),
        Err(StateError::SessionMismatch)
    );
}

#[test]
fn expired_state_is_rejected() {
    let (ctx, clock) = test_ctx(WALL);
    let sealed = codec().seal(&ctx, &session(), &state()).unwrap();
    clock.set_elapsed(TTL - 1);
    assert!(codec().open(&ctx, &session(), &sealed, EXPECT).is_ok());
    clock.set_elapsed(TTL);
    assert_eq!(
        codec().open(&ctx, &session(), &sealed, EXPECT),
        Err(StateError::Expired)
    );
    let (before, _) = test_ctx(WALL - 1);
    assert_eq!(
        codec().open(&before, &session(), &sealed, EXPECT),
        Err(StateError::Expired)
    );
}

#[test]
fn other_component_is_rejected() {
    let (ctx, _) = test_ctx(WALL);
    let sealed = codec().seal(&ctx, &session(), &state()).unwrap();
    let other = Expect {
        component_type: 8,
        ..EXPECT
    };
    assert_eq!(
        codec().open(&ctx, &session(), &sealed, other),
        Err(StateError::ComponentMismatch)
    );
}

#[test]
fn other_format_version_is_rejected() {
    let (ctx, _) = test_ctx(WALL);
    let sealed = codec().seal(&ctx, &session(), &state()).unwrap();
    let other = Expect {
        format_version: 2,
        ..EXPECT
    };
    assert_eq!(
        codec().open(&ctx, &session(), &sealed, other),
        Err(StateError::ComponentMismatch)
    );
}

#[test]
fn tag_with_other_label_is_rejected() {
    let (ctx, _) = test_ctx(WALL);
    let sealed = codec().seal(&ctx, &session(), &state()).unwrap();
    let mut value = read_signed_state(&sealed, &WireLimits::default())
        .unwrap()
        .unpack();
    let mut input = mac_input(&value);
    let other_label = b"ewe.ui.other.v1";
    input[..other_label.len()].copy_from_slice(other_label);
    value.tag = Some(tag(&input));
    let sealed = finish_signed_state(&value);
    assert_eq!(
        codec().open(&ctx, &session(), &sealed, EXPECT),
        Err(StateError::BadTag)
    );
}

#[test]
fn first_failing_check_wins() {
    let (ctx, clock) = test_ctx(WALL);
    let sealed = codec().seal(&ctx, &session(), &state()).unwrap();
    let mut value = read_signed_state(&sealed, &WireLimits::default())
        .unwrap()
        .unpack();
    let other = Session::new("sess-B").unwrap();
    let other_component = Expect {
        component_type: 8,
        ..EXPECT
    };

    assert_eq!(
        codec().open(&ctx, &other, &sealed, other_component),
        Err(StateError::SessionMismatch),
    );
    clock.set_elapsed(TTL);
    assert_eq!(
        codec().open(&ctx, &other, &sealed, other_component),
        Err(StateError::Expired)
    );

    value.tag.as_mut().unwrap()[0] ^= 1;
    let sealed = finish_signed_state(&value);
    assert_eq!(
        codec().open(&ctx, &other, &sealed, other_component),
        Err(StateError::BadTag)
    );

    value.key_id = 9;
    let sealed = finish_signed_state(&value);
    assert_eq!(
        codec().open(&ctx, &other, &sealed, other_component),
        Err(StateError::UnknownKey)
    );

    value.session_binding = Some(vec![0; 31]);
    let sealed = finish_signed_state(&value);
    assert!(matches!(
        codec().open(&ctx, &other, &sealed, other_component),
        Err(StateError::Malformed { .. })
    ));
}

#[test]
fn valid_tag_and_invalid_ciphertext_reports_decrypt_error() {
    let (ctx, _) = test_ctx(WALL);
    let sealed = codec().seal(&ctx, &session(), &state()).unwrap();
    let mut value = read_signed_state(&sealed, &WireLimits::default())
        .unwrap()
        .unpack();
    value.flags = 1;
    value.tag = Some(tag(&mac_input(&value)));
    let sealed = finish_signed_state(&value);
    assert_eq!(
        codec().open(&ctx, &session(), &sealed, EXPECT),
        Err(StateError::Decrypt)
    );
}

fn tag(input: &[u8]) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(&MAC).unwrap();
    mac.update(input);
    mac.finalize().into_bytes().to_vec()
}

#[test]
fn wire_fixture_is_well_formed_but_has_no_valid_tag() {
    let fixture = include_bytes!("../../foundation_ui_wire/fixtures/state_counter.bin");
    let wire = read_signed_state(fixture, &WireLimits::default()).unwrap();
    assert_eq!(wire.body().unwrap().bytes(), &[1, 0, 0, 0]);
    assert_eq!(wire.tag().unwrap().len(), 32);
    let (ctx, _) = test_ctx(wire.issued_at());
    assert_eq!(
        codec().open(&ctx, &session(), fixture, EXPECT),
        Err(StateError::BadTag)
    );
}

#[test]
fn every_authenticated_field_change_is_rejected() {
    let (ctx, _) = test_ctx(WALL);
    let codec = codec();
    let session = session();
    let sealed = codec.seal(&ctx, &session, &state()).unwrap();
    let signed = read_signed_state(&sealed, &WireLimits::default())
        .unwrap()
        .unpack();

    type Change = (&'static str, fn(&mut SignedStateT));
    let changes: [Change; 10] = [
        ("format version", |state| state.format_version ^= 1),
        ("flags", |state| state.flags ^= 1),
        ("key identifier", |state| state.key_id ^= 1),
        ("issue time", |state| state.issued_at ^= 1),
        ("expiry time", |state| state.expires_at ^= 1),
        ("component type", |state| state.component_type ^= 1),
        ("root node", |state| state.root_node ^= 1),
        ("session binding", |state| {
            state.session_binding.as_mut().unwrap()[0] ^= 1
        }),
        ("body", |state| state.body.as_mut().unwrap()[0] ^= 1),
        ("tag", |state| state.tag.as_mut().unwrap()[0] ^= 1),
    ];
    for (name, mutate) in changes {
        let mut changed = signed.clone();
        mutate(&mut changed);
        let bytes = finish_signed_state(&changed);
        assert!(
            codec.open(&ctx, &session, &bytes, EXPECT).is_err(),
            "{name}"
        );
    }
}
