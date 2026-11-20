use foundation_ui_runtime::state::{
    ChaChaStateSigner, HmacStateSigner, StateConfig, StateError, StateKey, StateKeyRing,
    StateSigner,
};

use super::support::{test_ctx, WALL};

#[test]
fn rejects_zero_and_duplicate_identifiers() {
    assert!(matches!(StateKey::new(0, [1; 32]), Err(StateError::BadKey)));
    let ring = StateKeyRing::new(StateKey::new(1, [1; 32]).unwrap());
    assert_eq!(ring.current_id(), 1);
    assert!(matches!(
        ring.with_previous(StateKey::new(1, [2; 32]).unwrap()),
        Err(StateError::BadKey)
    ));
}

fn rotation(ring: StateKeyRing, private: bool) -> Box<dyn StateSigner> {
    let config = StateConfig::default();
    if private {
        Box::new(ChaChaStateSigner::from_key_ring(ring, config).unwrap())
    } else {
        Box::new(HmacStateSigner::from_key_ring(ring, config).unwrap())
    }
}

#[test]
fn previous_key_verifies_old_state_and_current_key_signs_new_state() {
    let (ctx, _) = test_ctx(WALL);
    for private in [true, false] {
        let old = rotation(
            StateKeyRing::new(StateKey::new(1, [1; 32]).unwrap()),
            private,
        );
        let both = rotation(
            StateKeyRing::new(StateKey::new(2, [2; 32]).unwrap())
                .with_previous(StateKey::new(1, [1; 32]).unwrap())
                .unwrap(),
            private,
        );
        let signed = old.sign(&ctx, None, b"42").unwrap();
        assert_eq!(both.verify(&ctx, None, &signed).unwrap(), b"42");
        let current = both.sign(&ctx, None, b"43").unwrap();
        assert_eq!(
            old.verify(&ctx, None, &current),
            Err(StateError::UnknownKey)
        );
    }
}
