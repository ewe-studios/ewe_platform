//! Unit tests for ProxyConfig, ServiceConfig, SslConfig.

use foundation_proxy::{ProxyConfig, ServiceConfig, SslConfig, SslProvider};
use std::time::Duration;

#[test]
fn test_proxy_config_builder() {
    let config = ProxyConfig::new("example.com", "1.2.3.4")
        .ssl(SslConfig::lets_encrypt("admin@example.com"))
        .service(
            ServiceConfig::new("app", "app.example.com")
                .backend("http://localhost:3000")
                .health_check("/up", Duration::from_secs(5)),
        )
        .build();

    assert_eq!(config.domain, "example.com");
    assert_eq!(config.public_ip, "1.2.3.4");
    assert_eq!(config.services.len(), 1);
    assert_eq!(config.services[0].name, "app");
    assert_eq!(config.services[0].host, "app.example.com");
    assert_eq!(config.services[0].backends.len(), 1);
    assert_eq!(config.services[0].backends[0].url, "http://localhost:3000");
}

#[test]
fn test_ssl_config_variants() {
    let le = SslConfig::lets_encrypt("admin@example.com");
    assert!(matches!(le.provider, SslProvider::LetsEncrypt { ref email } if email == "admin@example.com"));

    let cf = SslConfig::cloudflare("zone_id_123");
    assert!(matches!(cf.provider, SslProvider::Cloudflare { ref zone_id } if zone_id == "zone_id_123"));

    let st = SslConfig::static_cert("/etc/cert.pem", "/etc/key.pem");
    assert!(matches!(st.provider, SslProvider::Static { .. }));

    assert!(matches!(SslConfig::default().provider, SslProvider::None));
}

#[test]
fn test_service_config_multiple_backends() {
    let svc = ServiceConfig::new("api", "api.example.com")
        .backend("http://localhost:8080")
        .backend("http://localhost:8081")
        .backend("http://localhost:8082");

    assert_eq!(svc.backends.len(), 3);
}

#[test]
fn test_proxy_config_load_file_valid_toml() {
    let temp = std::env::temp_dir().join("proxy_test.toml");
    let toml = r#"
domain = "test.example.com"
public_ip = "10.0.0.1"

[ssl]
provider = "None"

[[services]]
name = "app"
host = "app.test.example.com"
backends = ["http://localhost:3000"]
"#;
    std::fs::write(&temp, toml).unwrap();
    let result = ProxyConfig::load_file(temp.to_str().unwrap());
    std::fs::remove_file(&temp).ok();

    assert!(result.is_ok());
    let config = result.unwrap();
    assert_eq!(config.domain, "test.example.com");
    assert_eq!(config.services.len(), 1);
}
