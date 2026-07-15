//! Round-trip tests for the `proxy!` macro (Decision 19).
//!
//! Each test constructs a `ProxyConfig` via the `proxy!` macro and asserts
//! field-by-field against an equivalent programmatic builder chain. Verifies
//! criterion 4: macro output matches equivalent builder chain.
//!
//! Types do not derive `PartialEq`, so we compare individual fields.

use foundation_proxy::{proxy, BackendTarget, HealthCheckConfig, ProxyConfig, ServiceConfig, SslConfig, SslProvider};
use std::time::Duration;

// ── Helpers ──

fn assert_backend_eq(a: &BackendTarget, b: &BackendTarget) {
    assert_eq!(a.url, b.url, "backend url mismatch");
    assert_eq!(a.weight, b.weight, "backend weight mismatch for {}", a.url);
    assert_eq!(a.max_connections, b.max_connections, "backend max_connections mismatch for {}", a.url);
}

fn assert_health_check_eq(a: &HealthCheckConfig, b: &HealthCheckConfig) {
    assert_eq!(a.path, b.path);
    assert_eq!(a.interval, b.interval);
    assert_eq!(a.timeout, b.timeout);
    assert_eq!(a.healthy_threshold, b.healthy_threshold);
    assert_eq!(a.unhealthy_threshold, b.unhealthy_threshold);
}

fn assert_ssl_provider_eq(a: &SslProvider, b: &SslProvider) {
    match (a, b) {
        (SslProvider::LetsEncrypt { email: e1 }, SslProvider::LetsEncrypt { email: e2 }) => {
            assert_eq!(e1, e2);
        }
        (SslProvider::Cloudflare { zone_id: z1 }, SslProvider::Cloudflare { zone_id: z2 }) => {
            assert_eq!(z1, z2);
        }
        (SslProvider::Static { cert: c1, key: k1 }, SslProvider::Static { cert: c2, key: k2 }) => {
            assert_eq!(c1, c2);
            assert_eq!(k1, k2);
        }
        (SslProvider::None, SslProvider::None) => {}
        _ => panic!("SslProvider mismatch: {a:?} vs {b:?}"),
    }
}

fn assert_service_eq(a: &ServiceConfig, b: &ServiceConfig) {
    assert_eq!(a.name, b.name, "service name mismatch");
    assert_eq!(a.host, b.host, "service host mismatch for {}", a.name);
    assert_eq!(a.path_prefix, b.path_prefix, "service path_prefix mismatch for {}", a.name);
    assert_eq!(a.backends.len(), b.backends.len(), "service backends count mismatch for {}", a.name);
    for (_i, (ab, bb)) in a.backends.iter().zip(b.backends.iter()).enumerate() {
        assert_backend_eq(ab, bb);
    }
    match (&a.health_check, &b.health_check) {
        (Some(ah), Some(bh)) => assert_health_check_eq(ah, bh),
        (None, None) => {}
        _ => panic!("health_check presence mismatch for {}", a.name),
    }
}

fn assert_configs_eq(macro_config: &ProxyConfig, builder_config: &ProxyConfig) {
    assert_eq!(macro_config.domain, builder_config.domain, "domain mismatch");
    assert_eq!(macro_config.public_ip, builder_config.public_ip, "public_ip mismatch");
    assert_eq!(macro_config.bind_addr, builder_config.bind_addr, "bind_addr mismatch");
    assert_ssl_provider_eq(&macro_config.ssl.provider, &builder_config.ssl.provider);
    assert_eq!(macro_config.services.len(), builder_config.services.len(), "services count mismatch");
    for (_i, (ms, bs)) in macro_config.services.iter().zip(builder_config.services.iter()).enumerate() {
        assert_service_eq(ms, bs);
    }
}

// ── Tests ──

#[test]
fn test_proxy_macro_minimal() {
    let config = proxy! {
        domain: "example.com",
        public_ip: "1.2.3.4",
        ssl: none,
        services: {},
    };
    let expected = ProxyConfig::new("example.com", "1.2.3.4").build();
    assert_configs_eq(&config, &expected);
}

#[test]
fn test_proxy_macro_bind_addr() {
    let config = proxy! {
        domain: "example.com",
        public_ip: "1.2.3.4",
        bind_addr: "127.0.0.1:8080",
        ssl: none,
        services: {},
    };
    let expected = ProxyConfig::new("example.com", "1.2.3.4")
        .bind("127.0.0.1:8080")
        .build();
    assert_configs_eq(&config, &expected);
}

#[test]
fn test_proxy_macro_lets_encrypt_ssl() {
    let config = proxy! {
        domain: "example.com",
        public_ip: "5.6.7.8",
        ssl: lets_encrypt {
            email: "admin@example.com",
        },
        services: {
            app: {
                host: "app.example.com",
                backends: ["http://localhost:3000"],
                health_check: {
                    path: "/up",
                    interval: 5,
                    timeout: 2,
                },
            },
        },
    };
    let expected = ProxyConfig::new("example.com", "5.6.7.8")
        .ssl(SslConfig::lets_encrypt("admin@example.com"))
        .service(
            ServiceConfig::new("app", "app.example.com")
                .backend("http://localhost:3000")
                .health_check("/up", Duration::from_secs(5)),
        )
        .build();
    assert_configs_eq(&config, &expected);
}

#[test]
fn test_proxy_macro_static_cert_ssl() {
    let config = proxy! {
        domain: "example.com",
        public_ip: "10.0.0.1",
        ssl: static_cert {
            cert: "/etc/ssl/cert.pem",
            key: "/etc/ssl/key.pem",
        },
        services: {},
    };
    let expected = ProxyConfig::new("example.com", "10.0.0.1")
        .ssl(SslConfig::static_cert("/etc/ssl/cert.pem", "/etc/ssl/key.pem"))
        .build();
    assert_configs_eq(&config, &expected);
}

#[test]
fn test_proxy_macro_cloudflare_ssl() {
    let config = proxy! {
        domain: "example.com",
        public_ip: "10.0.0.2",
        ssl: cloudflare {
            zone_id: "zone_abc123",
        },
        services: {},
    };
    let expected = ProxyConfig::new("example.com", "10.0.0.2")
        .ssl(SslConfig::cloudflare("zone_abc123"))
        .build();
    assert_configs_eq(&config, &expected);
}

#[test]
fn test_proxy_macro_empty_services_block() {
    // Empty services block — valid, proxy with no routes.
    let config = proxy! {
        domain: "example.com",
        public_ip: "1.2.3.4",
        ssl: none,
        services: {},
    };
    assert!(config.services.is_empty());
}

#[test]
fn test_proxy_macro_service_no_backends() {
    // Service with no backends — valid, means no routing for that host.
    let config = proxy! {
        domain: "example.com",
        public_ip: "1.2.3.4",
        ssl: none,
        services: {
            empty_svc: {
                host: "empty.example.com",
            },
        },
    };
    assert_eq!(config.services.len(), 1);
    assert_eq!(config.services[0].name, "empty_svc");
    assert_eq!(config.services[0].host, "empty.example.com");
    assert!(config.services[0].backends.is_empty());
}

#[test]
fn test_proxy_macro_multiple_backends() {
    let config = proxy! {
        domain: "example.com",
        public_ip: "1.2.3.4",
        ssl: none,
        services: {
            api: {
                host: "api.example.com",
                backends: [
                    "http://localhost:8080",
                    "http://localhost:8081",
                    "http://localhost:8082",
                ],
            },
        },
    };
    let expected = ProxyConfig::new("example.com", "1.2.3.4")
        .service(
            ServiceConfig::new("api", "api.example.com")
                .backend("http://localhost:8080")
                .backend("http://localhost:8081")
                .backend("http://localhost:8082"),
        )
        .build();
    assert_configs_eq(&config, &expected);
}

#[test]
fn test_proxy_macro_multiple_services() {
    let config = proxy! {
        domain: "example.com",
        public_ip: "1.2.3.4",
        ssl: none,
        services: {
            app: {
                host: "app.example.com",
                backends: ["http://localhost:3000"],
                health_check: {
                    path: "/up",
                    interval: 5,
                    timeout: 2,
                },
            },
            admin: {
                host: "admin.example.com",
                backends: ["http://localhost:3001"],
            },
        },
    };
    let expected = ProxyConfig::new("example.com", "1.2.3.4")
        .service(
            ServiceConfig::new("app", "app.example.com")
                .backend("http://localhost:3000")
                .health_check("/up", Duration::from_secs(5)),
        )
        .service(
            ServiceConfig::new("admin", "admin.example.com")
                .backend("http://localhost:3001"),
        )
        .build();
    assert_configs_eq(&config, &expected);
}

#[test]
fn test_proxy_macro_path_prefix() {
    let config = proxy! {
        domain: "example.com",
        public_ip: "1.2.3.4",
        ssl: none,
        services: {
            api: {
                host: "example.com",
                path_prefix: "/api",
                backends: ["http://localhost:4000"],
            },
        },
    };
    let expected = ProxyConfig::new("example.com", "1.2.3.4")
        .service(
            ServiceConfig::new("api", "example.com")
                .path_prefix("/api")
                .backend("http://localhost:4000"),
        )
        .build();
    assert_configs_eq(&config, &expected);
}

#[test]
fn test_proxy_macro_health_check_all_fields() {
    let config = proxy! {
        domain: "example.com",
        public_ip: "1.2.3.4",
        ssl: none,
        services: {
            monitored: {
                host: "monitored.example.com",
                backends: ["http://localhost:5000"],
                health_check: {
                    path: "/healthz",
                    interval: 10,
                    timeout: 3,
                    healthy: 3,
                    unhealthy: 2,
                },
            },
        },
    };
    let expected = ProxyConfig::new("example.com", "1.2.3.4")
        .service(
            ServiceConfig::new("monitored", "monitored.example.com")
                .backend("http://localhost:5000")
                .health_check_config(HealthCheckConfig {
                    path: "/healthz".into(),
                    interval: Duration::from_secs(10),
                    timeout: Duration::from_secs(3),
                    healthy_threshold: 3,
                    unhealthy_threshold: 2,
                }),
        )
        .build();
    assert_configs_eq(&config, &expected);
}

#[test]
fn test_proxy_macro_output_is_proxy_config_type() {
    // Type-level check: proxy! output is exactly ProxyConfig.
    fn takes_proxy_config(_c: ProxyConfig) {}
    let config = proxy! {
        domain: "example.com",
        public_ip: "1.2.3.4",
        ssl: none,
        services: {},
    };
    takes_proxy_config(config);
}

#[test]
fn test_proxy_macro_explicit_none_ssl() {
    // ssl: none should produce SslConfig::default() (i.e. SslProvider::None)
    let config = proxy! {
        domain: "example.com",
        public_ip: "1.2.3.4",
        ssl: none,
        services: {},
    };
    assert!(matches!(config.ssl.provider, SslProvider::None));
}
