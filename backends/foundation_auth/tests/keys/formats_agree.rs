//! One Ed25519 key in OpenSSH form and in PKCS#8 form gives the same key.

use foundation_auth::keys::PrivateKey;

use super::common;

#[test]
fn openssh_and_pkcs8_agree() {
    let openssh = PrivateKey::parse(common::ED25519_OPENSSH.as_bytes()).expect("OpenSSH key");
    let pkcs8 = PrivateKey::parse(common::ED25519_PKCS8.as_bytes()).expect("PKCS#8 key");

    assert_eq!(openssh.public_key(), pkcs8.public_key());
    assert_eq!(openssh.kid(), pkcs8.kid());
    assert_eq!(openssh.pkcs8_der(), pkcs8.pkcs8_der());

    let from_openssh = openssh.into_signing_key().expect("Ed25519 key");
    let from_pkcs8 = pkcs8.into_signing_key().expect("Ed25519 key");
    assert_eq!(
        from_openssh.verifying_key().as_bytes(),
        from_pkcs8.verifying_key().as_bytes()
    );
    // Ed25519 signatures are deterministic, so the same key signs the same
    // bytes the same way. The key pair is the same when the PKCS#8 form is the
    // same.
    assert_eq!(from_openssh.to_pkcs8_pem(), from_pkcs8.to_pkcs8_pem());
}
