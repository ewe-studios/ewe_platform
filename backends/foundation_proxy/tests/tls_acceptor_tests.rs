//! `tls::build_acceptor` from PEM files (moved from the inline test module of
//! `src/tls.rs`).

use foundation_proxy::config::SslConfig;
use foundation_proxy::tls::build_acceptor;

#[test]
fn test_build_acceptor_static_from_fixtures() {
    let cert = include_bytes!("fixtures/cert.pem");
    let key = include_bytes!("fixtures/key.pem");
    let dir = std::env::temp_dir().join("proxy_tls_test");
    std::fs::create_dir_all(&dir).ok();
    let cert_path = dir.join("cert.pem");
    let key_path = dir.join("key.pem");
    std::fs::write(&cert_path, cert).unwrap();
    std::fs::write(&key_path, key).unwrap();

    let config = SslConfig::static_cert(
        &cert_path.display().to_string(),
        &key_path.display().to_string(),
    );
    assert!(build_acceptor(&config).is_ok(), "valid PEM should build acceptor");
}

#[test]
fn test_build_acceptor_none_errs() {
    assert!(build_acceptor(&SslConfig::default()).is_err());
}
