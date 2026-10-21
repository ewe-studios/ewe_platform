//! Signed, time-boxed bootstrap secrets and the signing key's lifecycle.

use std::time::{Duration, SystemTime};

use base64::Engine;
use foundation_agent_rpc::secret::{
    unix_secs, BootstrapSecrets, SignedSecret, SigningAlgorithm, SigningKey, SigningKeyConfig,
    SigningKeyOrigin, SIGNING_KEY_FILE,
};
use foundation_agent_rpc::AgentRpcError;

const RSA_FIXTURE: &str = include_str!("fixtures/test-only-rsa-signing-key.pem");

fn signer() -> BootstrapSecrets {
    BootstrapSecrets::new(SigningKey::generate().expect("key"))
}

fn now() -> u64 {
    unix_secs(SystemTime::now())
}

fn is_auth(err: &foundation_errstacks::ErrorTrace<AgentRpcError>) -> bool {
    matches!(err.current_context(), AgentRpcError::Auth(_))
}

/// base64url → JSON → re-encode, applying `edit` to the JSON object.
fn tamper(encoded: &str, edit: impl FnOnce(&mut serde_json::Map<String, serde_json::Value>)) -> String {
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let mut json: serde_json::Value =
        serde_json::from_slice(&b64.decode(encoded).expect("base64")).expect("json");
    edit(json.as_object_mut().expect("object"));
    b64.encode(serde_json::to_vec(&json).expect("json"))
}

#[test]
fn the_shared_form_is_base64url_json_with_the_documented_fields() {
    let secrets = signer();
    let secret = secrets.issue().expect("issue");
    let encoded = secret.encode();
    assert!(encoded
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'));

    let json: serde_json::Value = serde_json::from_slice(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(encoded.as_bytes())
            .expect("base64"),
    )
    .expect("json");
    for field in ["v", "alg", "public_key", "secret", "valid_from", "valid_until", "signature"] {
        assert!(json.get(field).is_some(), "missing {field}: {json}");
    }
    assert_eq!(json["alg"], "ed25519");
    assert_eq!(
        json["valid_until"].as_u64().expect("u64") - json["valid_from"].as_u64().expect("u64"),
        BootstrapSecrets::DEFAULT_VALIDITY.as_secs()
    );

    let decoded = SignedSecret::decode(&encoded).expect("decode");
    assert_eq!(decoded.id(), secret.id());
    assert_eq!(decoded.public_key(), secrets.signing_key().public_key());
    assert!(decoded.verify_self_signed());
}

#[test]
fn a_fresh_secret_verifies_once_issued() {
    let secrets = signer();
    let a = secrets.issue().expect("issue");
    let b = secrets.issue().expect("issue");
    assert_ne!(a.id(), b.id(), "every secret is fresh randomness");
    let verified = secrets.verify(&a.encode()).expect("verifies");
    assert_eq!(verified.id, a.id());
    assert_eq!(verified.valid_until, a.valid_until());
}

#[test]
fn secrets_outside_their_window_are_refused() {
    let secrets = signer();
    let issued = secrets.issue_for(Duration::from_secs(60)).expect("issue");
    let t = issued.valid_from();

    let err = secrets.verify_at(&issued.encode(), t + 60).expect_err("expired");
    assert!(is_auth(&err) && err.current_context().to_string().contains("expired"), "{err:?}");
    assert!(secrets.verify_at(&issued.encode(), t + 59).is_ok(), "last valid second");

    let later = SystemTime::now() + Duration::from_secs(3600);
    let future = secrets
        .issue_window(later, later + Duration::from_secs(60))
        .expect("issue");
    let err = secrets.verify(&future.encode()).expect_err("not yet valid");
    assert!(err.current_context().to_string().contains("not valid yet"), "{err:?}");

    let past = SystemTime::now() - Duration::from_secs(120);
    let stale = secrets
        .issue_window(past, past + Duration::from_secs(60))
        .expect("issue");
    assert!(secrets.verify(&stale.encode()).is_err());

    assert!(
        secrets
            .issue_window(SystemTime::now(), SystemTime::now())
            .is_err(),
        "an empty window is refused at issue time"
    );
}

#[test]
fn tampering_with_any_signed_field_is_refused() {
    let secrets = signer();
    let encoded = secrets.issue().expect("issue").encode().to_string();

    // Stretch the window.
    let stretched = tamper(&encoded, |o| {
        let until = o["valid_until"].as_u64().expect("u64");
        o.insert("valid_until".into(), (until + 365 * 24 * 3600).into());
    });
    // Swap the random secret.
    let swapped = tamper(&encoded, |o| {
        o.insert(
            "secret".into(),
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode([7u8; 32])
                .into(),
        );
    });
    // Flip a signature byte.
    let forged = tamper(&encoded, |o| {
        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let mut sig = b64
            .decode(o["signature"].as_str().expect("str"))
            .expect("sig");
        sig[0] ^= 1;
        o.insert("signature".into(), b64.encode(sig).into());
    });
    for (what, bad) in [("window", stretched), ("secret", swapped), ("signature", forged)] {
        let err = secrets.verify(&bad).expect_err(what);
        assert!(is_auth(&err), "{what}: {err:?}");
        assert!(
            err.current_context().to_string().contains("signature"),
            "{what}: {err:?}"
        );
        assert!(
            !SignedSecret::decode(&bad).expect("well formed").verify_self_signed(),
            "{what}: the client-side check catches it too"
        );
    }

    for garbage in ["", "!!", "e30", "AAAA"] {
        assert!(is_auth(&secrets.verify(garbage).expect_err(garbage)));
    }
    assert!(matches!(
        SignedSecret::decode("e30").expect_err("{}").current_context(),
        AgentRpcError::Codec(_)
    ));
}

#[test]
fn secrets_from_another_key_are_refused() {
    let ours = signer();
    let theirs = signer();
    let foreign = theirs.issue().expect("issue");
    assert!(foreign.verify_self_signed(), "validly signed — by someone else");
    let err = ours.verify(&foreign.encode()).expect_err("foreign key");
    assert!(
        err.current_context().to_string().contains("different signing key"),
        "{err:?}"
    );

    // Claiming our public key with their signature does not help either.
    let encoded = foreign.encode().to_string();
    let ours_pk = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(ours.signing_key().public_key());
    let relabelled = tamper(&encoded, |o| {
        o.insert("public_key".into(), ours_pk.into());
    });
    assert!(ours.verify(&relabelled).is_err());
}

#[test]
fn secrets_and_keys_are_never_printed() {
    let secrets = signer();
    let secret = secrets.issue().expect("issue");
    let encoded = secret.encode().to_string();
    let raw = tamper(&encoded, |_| {});
    let json: serde_json::Value = serde_json::from_slice(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(raw)
            .expect("b64"),
    )
    .expect("json");
    let inner = json["secret"].as_str().expect("str").to_string();
    for shown in [format!("{secret:?}"), format!("{secret}")] {
        assert!(!shown.contains(&inner) && !shown.contains(&encoded), "{shown}");
    }
    let key_pem = secrets.signing_key().to_pkcs8_pem();
    let shown = format!("{:?}", secrets.signing_key());
    assert!(!shown.contains("PRIVATE KEY") && !shown.contains(&key_pem[30..60]), "{shown}");
}

#[test]
fn the_key_is_generated_on_first_start_and_reloaded_after() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = SigningKeyConfig::in_state_dir(dir.path()).without_env_var();
    let path = dir.path().join(SIGNING_KEY_FILE);

    let (first, origin) = SigningKey::load_or_generate(&config).expect("generate");
    assert_eq!(origin, SigningKeyOrigin::Generated(path.clone()));
    assert_eq!(first.algorithm(), SigningAlgorithm::Ed25519);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).expect("meta").permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "owner-only key file");
    }

    let (again, origin) = SigningKey::load_or_generate(&config).expect("load");
    assert_eq!(origin, SigningKeyOrigin::Loaded(path));
    assert_eq!(again.public_key(), first.public_key());

    // A secret signed before the restart still verifies with the reloaded key.
    let before = BootstrapSecrets::new(first).issue().expect("issue");
    assert!(BootstrapSecrets::new(again).verify(&before.encode()).is_ok());
}

#[test]
fn a_lost_key_is_regenerated_and_old_secrets_die_with_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = SigningKeyConfig::in_state_dir(dir.path()).without_env_var();
    let (old, _) = SigningKey::load_or_generate(&config).expect("generate");
    let outstanding = BootstrapSecrets::new(old).issue().expect("issue");

    std::fs::remove_file(config.path()).expect("lose the key");
    let (new, origin) = SigningKey::load_or_generate(&config).expect("regenerate");
    assert!(matches!(origin, SigningKeyOrigin::Generated(_)));
    let err = BootstrapSecrets::new(new)
        .verify(&outstanding.encode())
        .expect_err("old secret");
    assert!(
        err.current_context().to_string().contains("different signing key"),
        "{err:?}"
    );
}

#[test]
fn a_corrupt_key_file_is_an_error_not_silently_replaced() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = SigningKeyConfig::in_state_dir(dir.path()).without_env_var();
    std::fs::write(config.path(), "not a key").expect("write");
    let err = SigningKey::load_or_generate(&config).expect_err("corrupt");
    assert!(matches!(err.current_context(), AgentRpcError::Config(_)));
    assert_eq!(
        std::fs::read_to_string(config.path()).expect("read"),
        "not a key",
        "the file is left for the operator"
    );
}

#[test]
fn the_environment_variable_names_the_key() {
    let dir = tempfile::tempdir().expect("tempdir");
    let operator_key = SigningKey::generate().expect("key");
    let key_path = dir.path().join("operator.pem");
    std::fs::write(&key_path, operator_key.to_pkcs8_pem().as_bytes()).expect("write");

    // Unique variable names: tests in this binary run in parallel.
    let by_path = "EWE_AGENT_RPC_TEST_SIGNING_KEY_PATH";
    std::env::set_var(by_path, &key_path);
    let config = SigningKeyConfig::in_state_dir(dir.path().join("state")).with_env_var(by_path);
    let (key, origin) = SigningKey::load_or_generate(&config).expect("from env path");
    assert_eq!(origin, SigningKeyOrigin::Environment);
    assert_eq!(key.public_key(), operator_key.public_key());
    assert!(!config.path().exists(), "nothing generated when the env names a key");

    let inline = "EWE_AGENT_RPC_TEST_SIGNING_KEY_INLINE";
    std::env::set_var(inline, operator_key.to_pkcs8_pem().as_str());
    let config = SigningKeyConfig::in_state_dir(dir.path()).with_env_var(inline);
    let (key, origin) = SigningKey::load_or_generate(&config).expect("inline PEM");
    assert_eq!(origin, SigningKeyOrigin::Environment);
    assert_eq!(key.public_key(), operator_key.public_key());

    let missing = "EWE_AGENT_RPC_TEST_SIGNING_KEY_MISSING";
    std::env::set_var(missing, dir.path().join("nope.pem"));
    let config = SigningKeyConfig::in_state_dir(dir.path().join("other")).with_env_var(missing);
    assert!(
        SigningKey::load_or_generate(&config).is_err(),
        "a named key that is missing is an error, not a reason to generate one"
    );
    assert!(!config.path().exists());

    for var in [by_path, inline, missing] {
        std::env::remove_var(var);
    }
}

#[test]
fn an_operator_supplied_rsa_key_signs_and_verifies() {
    let key = SigningKey::from_pkcs8_pem(RSA_FIXTURE).expect("rsa pkcs8");
    assert_eq!(key.algorithm(), SigningAlgorithm::RsaPssSha256);
    let secrets = BootstrapSecrets::new(key);
    let secret = secrets.issue().expect("issue");
    let decoded = SignedSecret::decode(&secret.encode()).expect("decode");
    assert_eq!(decoded.algorithm(), SigningAlgorithm::RsaPssSha256);
    assert!(decoded.verify_self_signed());
    assert!(secrets.verify(&secret.encode()).is_ok());
    // An Ed25519 server refuses it.
    assert!(signer().verify(&secret.encode()).is_err());
    // The PEM round-trips.
    let again = SigningKey::from_pkcs8_pem(&secrets.signing_key().to_pkcs8_pem()).expect("reparse");
    assert_eq!(again.public_key(), secrets.signing_key().public_key());
}

#[test]
fn the_secret_fits_comfortably_in_a_qr_code() {
    let secret = signer().issue().expect("issue");
    assert!(secret.encode().len() < 600, "{}", secret.encode().len());
    assert!(secret.is_valid_at(now()));
}
