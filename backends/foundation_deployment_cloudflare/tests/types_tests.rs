//! Unit tests for Cloudflare domain types.

use foundation_deployment_cloudflare::*;
use serde_json;

#[test]
fn test_dns_record_type_as_str() {
    assert_eq!(DnsRecordType::A.as_str(), "A");
    assert_eq!(DnsRecordType::Aaaa.as_str(), "AAAA");
    assert_eq!(DnsRecordType::Cname.as_str(), "CNAME");
    assert_eq!(DnsRecordType::Txt.as_str(), "TXT");
    assert_eq!(DnsRecordType::Mx.as_str(), "MX");
}

#[test]
fn test_cloudflare_error_display() {
    let e = CloudflareError::Auth("token missing".into());
    assert_eq!(format!("{e}"), "Cloudflare auth failed: token missing");

    let e = CloudflareError::ZoneNotFound("example.com".into());
    assert_eq!(format!("{e}"), "Zone not found: example.com");

    let e = CloudflareError::Api { status: 400, message: "bad request".into() };
    assert_eq!(format!("{e}"), "API error (400): bad request");
}

#[test]
fn test_zone_status_serde() {
    let json = r#""active""#;
    let status: ZoneStatus = serde_json::from_str(json).unwrap();
    assert!(matches!(status, ZoneStatus::Active));
}

#[test]
fn test_dns_record_type_serde() {
    let json = r#""A""#;
    let t: DnsRecordType = serde_json::from_str(json).unwrap();
    assert_eq!(t, DnsRecordType::A);

    let json = r#""CNAME""#;
    let t: DnsRecordType = serde_json::from_str(json).unwrap();
    assert_eq!(t, DnsRecordType::Cname);
}

#[test]
fn test_cf_err_wraps_in_error_trace() {
    let trace = cf_err(CloudflareError::Http("timeout".into()));
    let display = format!("{trace}");
    assert!(display.contains("HTTP transport error: timeout"), "got: {display}");
    assert!(display.contains("ErrorTrace"), "should contain ErrorTrace header");
}

#[test]
fn test_cloudflare_client_from_env_missing() {
    // Should fail when env vars are not set
    std::env::remove_var("CLOUDFLARE_API_TOKEN");
    std::env::remove_var("CLOUDFLARE_ZONE_ID");
    let result = CloudflareClient::from_env();
    assert!(result.is_err());
}
