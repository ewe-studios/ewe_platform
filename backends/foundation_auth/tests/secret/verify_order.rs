//! One test for each step of `SecretVerifier::verify`, in the order of the
//! design. The time steps use the simulated clock.

use base64::Engine;
use foundation_auth::secret::{
    DecodeLimits, SecretError, SecretIssuer, SecretPurpose, SecretRequest, SecretVerifier,
    SigningKey, VerifyPolicy,
};
use foundation_compact::Duration;

use super::common;

const T0: u64 = 1_800_000_000;

fn issue(
    ctx: foundation_context::Ctx,
    request: &SecretRequest,
) -> foundation_auth::secret::SignedSecret {
    SecretIssuer::new(
        common::domain(),
        common::signing_key(),
        ctx,
        Duration::from_hours(72),
    )
    .issue(request)
    .expect("issue")
}

fn verifier(
    ctx: foundation_context::Ctx,
    key: &SigningKey,
    policy: VerifyPolicy,
) -> SecretVerifier {
    SecretVerifier::new(
        common::domain(),
        vec![common::verifying_key(key)],
        ctx,
        policy,
    )
}

fn decode_json(secret: &foundation_auth::secret::SignedSecret) -> serde_json::Value {
    let json = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(secret.encode().as_bytes())
        .expect("base64url");
    serde_json::from_slice(&json).expect("JSON")
}

fn reencode(value: &serde_json::Value) -> String {
    let json = serde_json::to_vec(value).expect("JSON");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json)
}

#[test]
fn step_1_size_limit() {
    let (ctx, _clock) = common::ctx_at(T0);
    let secret = issue(ctx.clone(), &SecretRequest::enroll(Duration::from_hours(1)));
    let policy = VerifyPolicy::enroll().with_decode_limits(DecodeLimits {
        max_encoded_bytes: 8,
    });
    let verifier = verifier(ctx, &common::signing_key(), policy);
    assert!(matches!(
        verifier.verify(&secret.encode()),
        Err(SecretError::Malformed(_))
    ));
}

#[test]
fn step_2_format() {
    let (ctx, _clock) = common::ctx_at(T0);
    let verifier = verifier(ctx, &common::signing_key(), VerifyPolicy::enroll());
    assert!(matches!(
        verifier.verify("this is not a signed secret"),
        Err(SecretError::Malformed(_))
    ));
}

#[test]
fn step_2_unknown_version() {
    let (ctx, _clock) = common::ctx_at(T0);
    let verifier = verifier(ctx, &common::signing_key(), VerifyPolicy::enroll());
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(br#"{"v":3}"#);
    assert_eq!(
        verifier.verify(&encoded),
        Err(SecretError::UnsupportedVersion(3))
    );
}

#[test]
fn step_3_unknown_key() {
    let (ctx, _clock) = common::ctx_at(T0);
    let secret = issue(ctx.clone(), &SecretRequest::enroll(Duration::from_hours(1)));
    let other = SigningKey::from_seed([0x11; 32], None);
    let verifier = verifier(ctx, &other, VerifyPolicy::enroll());
    assert_eq!(
        verifier.verify(&secret.encode()),
        Err(SecretError::UnknownKey)
    );
}

#[test]
fn step_4_bad_signature() {
    let (ctx, _clock) = common::ctx_at(T0);
    let secret = issue(ctx.clone(), &SecretRequest::enroll(Duration::from_hours(1)));
    let mut value = decode_json(&secret);
    let signature = value["signature"].as_str().expect("signature");
    let mut chars: Vec<char> = signature.chars().collect();
    chars[0] = if chars[0] == 'A' { 'B' } else { 'A' };
    value["signature"] = serde_json::json!(chars.into_iter().collect::<String>());

    let verifier = verifier(ctx, &common::signing_key(), VerifyPolicy::enroll());
    assert_eq!(
        verifier.verify(&reencode(&value)),
        Err(SecretError::BadSignature)
    );
}

#[test]
fn step_5_wrong_purpose() {
    let (ctx, _clock) = common::ctx_at(T0);
    let secret = issue(ctx.clone(), &SecretRequest::enroll(Duration::from_hours(1)));
    let verifier = verifier(
        ctx,
        &common::signing_key(),
        VerifyPolicy::access("https://agents.example.com/rpc"),
    );
    assert_eq!(
        verifier.verify(&secret.encode()),
        Err(SecretError::WrongPurpose)
    );
    assert_eq!(secret.purpose(), SecretPurpose::Enroll);
}

#[test]
fn step_6_wrong_audience() {
    let (ctx, _clock) = common::ctx_at(T0);
    let secret = issue(
        ctx.clone(),
        &SecretRequest::access(Duration::from_hours(1), "https://a.example.com"),
    );
    let verifier = verifier(
        ctx,
        &common::signing_key(),
        VerifyPolicy::access("https://b.example.com"),
    );
    assert_eq!(
        verifier.verify(&secret.encode()),
        Err(SecretError::WrongAudience)
    );
}

#[test]
fn step_7_window_too_long() {
    let (ctx, _clock) = common::ctx_at(T0);
    let secret = issue(
        ctx.clone(),
        &SecretRequest::enroll(Duration::from_hours(48)),
    );
    let policy = VerifyPolicy::enroll().with_max_validity(Duration::from_hours(24));
    let verifier = verifier(ctx, &common::signing_key(), policy);
    assert_eq!(
        verifier.verify(&secret.encode()),
        Err(SecretError::WindowTooLong)
    );
}

#[test]
fn step_8_time_edges_and_skew() {
    let from = T0;
    let (issue_ctx, _clock) = common::ctx_at(from);
    let secret = issue(
        issue_ctx,
        &SecretRequest::enroll(Duration::from_hours(1)).with_not_before(from),
    );
    let encoded = secret.encode();
    let until = secret.valid_until();
    assert_eq!(until, from + 3600);

    let cases = [
        (from - 301, Some(SecretError::NotYetValid)),
        (from - 300, None),
        (from, None),
        (until - 1, None),
        (until + 299, None),
        (until + 300, Some(SecretError::Expired)),
    ];
    for (now, expected) in cases {
        let (ctx, _clock) = common::ctx_at(now);
        let verifier = verifier(ctx, &common::signing_key(), VerifyPolicy::enroll());
        let result = verifier.verify(&encoded);
        match expected {
            Some(error) => assert_eq!(result, Err(error), "at {now}"),
            None => assert!(result.is_ok(), "at {now}: {result:?}"),
        }
    }
}
