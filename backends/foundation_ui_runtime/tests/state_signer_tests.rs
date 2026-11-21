use std::time::Duration;

use foundation_context::{Ctx, MinimumElement};
use foundation_ui_runtime::state::{
    ChaChaStateSigner, HmacStateSigner, StateConfig, StateError, StateSigner,
};

use super::support::{test_ctx, TTL, WALL};

fn signers(key: u8, config: StateConfig) -> [Box<dyn StateSigner>; 2] {
    [
        Box::new(HmacStateSigner::new([key; 32], config).unwrap()),
        Box::new(ChaChaStateSigner::new([key; 32], config).unwrap()),
    ]
}

#[test]
fn both_restore_arbitrary_bytes_and_reject_wrong_bindings() {
    let (ctx, _) = test_ctx(WALL);
    for signer in signers(1, StateConfig::default()) {
        for body in [&[][..], &[0, 128, 255], &b"{\"count\":4}"[..]] {
            for bound in [None, Some(&[][..]), Some(b"alice"), Some(b"\x00alice")] {
                let signed = signer.sign(&ctx, bound, body).unwrap();
                assert_eq!(signer.verify(&ctx, bound, &signed).unwrap(), body);
                for other in [None, Some(&[][..]), Some(b"alice"), Some(b"\x00alice")] {
                    if other != bound {
                        assert_eq!(signer.verify(&ctx, other, &signed), Err(StateError::BadTag));
                    }
                }
            }
        }
    }
}

#[test]
fn every_input_byte_is_authenticated() {
    let (ctx, _) = test_ctx(WALL);
    for signer in signers(1, StateConfig::default()) {
        let signed = signer.sign(&ctx, None, b"a counter").unwrap();
        for index in 0..signed.len() {
            let mut changed = signed.clone();
            changed[index] ^= 1;
            assert!(signer.verify(&ctx, None, &changed).is_err(), "byte {index}");
        }
        for length in 0..signed.len() {
            assert!(signer.verify(&ctx, None, &signed[..length]).is_err());
        }
        let mut longer = signed;
        longer.push(0);
        assert!(signer.verify(&ctx, None, &longer).is_err());
    }
}

#[test]
fn both_reject_a_different_secret_and_another_format() {
    let (ctx, _) = test_ctx(WALL);
    let config = StateConfig::default();
    let signers_a = signers(1, config);
    let signers_b = signers(2, config);
    for (index, signer) in signers_a.iter().enumerate() {
        let signed = signer.sign(&ctx, None, b"42").unwrap();
        assert_eq!(
            signers_b[index].verify(&ctx, None, &signed),
            Err(StateError::BadTag)
        );
        assert!(signers_a[1 - index].verify(&ctx, None, &signed).is_err());
    }
}

#[test]
fn authenticated_time_window_includes_issue_and_excludes_expiry() {
    let (earlier, _) = test_ctx(WALL - 1);
    for signer in signers(1, StateConfig::default()) {
        let (ctx, clock) = test_ctx(WALL);
        let signed = signer.sign(&ctx, None, b"42").unwrap();
        assert_eq!(
            signer.verify(&earlier, None, &signed),
            Err(StateError::Expired)
        );
        clock.set_elapsed(TTL - 1);
        assert_eq!(signer.verify(&ctx, None, &signed).unwrap(), b"42");
        clock.set_elapsed(TTL);
        assert_eq!(signer.verify(&ctx, None, &signed), Err(StateError::Expired));
    }
}

#[test]
fn zero_lifetime_is_invalid() {
    let config = StateConfig::default().ttl(Duration::ZERO);
    assert!(matches!(
        HmacStateSigner::new([1; 32], config),
        Err(StateError::InvalidLifetime)
    ));
    assert!(matches!(
        ChaChaStateSigner::new([1; 32], config),
        Err(StateError::InvalidLifetime)
    ));
}

#[test]
fn body_limit_applies_on_both_input_paths() {
    let (ctx, _) = test_ctx(WALL);
    let permitted = signers(1, StateConfig::default().max_body_bytes(4));
    for (limited, normal) in signers(1, StateConfig::default().max_body_bytes(3))
        .iter()
        .zip(permitted)
    {
        let boundary = limited.sign(&ctx, None, b"123").unwrap();
        assert_eq!(limited.verify(&ctx, None, &boundary).unwrap(), b"123");
        let too_large = StateError::BodyTooLarge {
            actual: 4,
            limit: 3,
        };
        assert_eq!(limited.sign(&ctx, None, b"1234"), Err(too_large.clone()));
        let signed = normal.sign(&ctx, None, b"1234").unwrap();
        assert_eq!(limited.verify(&ctx, None, &signed), Err(too_large));
    }
}

#[test]
fn only_the_readable_format_exposes_the_body_and_the_private_format_uses_fresh_nonces() {
    let (ctx, _) = test_ctx(WALL);
    let body = b"the complete counter body";
    let [readable, private] = signers(1, StateConfig::default());
    let public = readable.sign(&ctx, None, body).unwrap();
    assert!(public.windows(body.len()).any(|w| w == body));
    let first = private.sign(&ctx, None, body).unwrap();
    let second = private.sign(&ctx, None, body).unwrap();
    assert_ne!(first, second);
    for sealed in [first, second] {
        assert!(!sealed.windows(body.len()).any(|w| w == body));
        assert_eq!(private.verify(&ctx, None, &sealed).unwrap(), body);
    }
}

#[test]
fn missing_services_do_not_fall_back_to_host_globals() {
    let (ctx, clock) = test_ctx(WALL);
    let no_clock = Ctx::incomplete_for_test(clock.clone(), MinimumElement::Clock);
    for signer in signers(1, StateConfig::default()) {
        assert_eq!(
            signer.sign(&no_clock, None, b""),
            Err(StateError::MissingEnv { what: "clock" })
        );
        let signed = signer.sign(&ctx, None, b"").unwrap();
        assert_eq!(
            signer.verify(&no_clock, None, &signed),
            Err(StateError::MissingEnv { what: "clock" })
        );
    }
    let no_rng = Ctx::incomplete_for_test(clock, MinimumElement::Rng);
    let [readable, private] = signers(1, StateConfig::default());
    assert!(readable.sign(&no_rng, None, b"").is_ok());
    assert_eq!(
        private.sign(&no_rng, None, b""),
        Err(StateError::MissingEnv { what: "rng" })
    );
}
