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

// ── DnsRecordInput ──

#[test]
fn test_dns_record_input_from_record() {
    let record = DnsRecord {
        id: "abc123".into(),
        zone_id: "zone_1".into(),
        name: "api.example.com".into(),
        r#type: DnsRecordType::A,
        content: "1.2.3.4".into(),
        ttl: 120,
        proxied: true,
        comment: Some("main api".into()),
        tags: vec!["production".into()],
        created_on: chrono::Utc::now(),
        modified_on: chrono::Utc::now(),
    };

    let input = DnsRecordInput::from(&record);

    assert_eq!(input.r#type, DnsRecordType::A);
    assert_eq!(input.name, "api.example.com");
    assert_eq!(input.content, "1.2.3.4");
    assert_eq!(input.ttl, 120);
    assert!(input.proxied);
    assert_eq!(input.comment.as_deref(), Some("main api"));
    assert_eq!(input.tags, vec!["production"]);
}

#[test]
fn test_dns_record_input_serialization() {
    let input = DnsRecordInput {
        r#type: DnsRecordType::A,
        name: "www.example.com".into(),
        content: "10.0.0.1".into(),
        ttl: 60,
        proxied: false,
        comment: None,
        tags: vec![],
    };

    let json = serde_json::to_value(&input).unwrap();
    assert_eq!(json["type"], "A");
    assert_eq!(json["name"], "www.example.com");
    assert_eq!(json["content"], "10.0.0.1");
    assert_eq!(json["ttl"], 60);
    assert_eq!(json["proxied"], false);
    // Optional fields with skip_serializing_if should be absent
    assert!(json.get("comment").is_none());
    assert!(json.get("tags").is_none());
}

// ── CloudflareResponse<T> (API envelope) ──

#[test]
fn test_cloudflare_response_success_deserialization() {
    let json = r#"{
        "success": true,
        "errors": [],
        "messages": [],
        "result": {"foo": "bar"}
    }"#;
    let resp: CloudflareResponse<serde_json::Value> = serde_json::from_str(json).unwrap();
    assert!(resp.success);
    assert!(resp.errors.is_empty());
    assert_eq!(resp.result["foo"], "bar");
    assert!(resp.result_info.is_none());
}

#[test]
fn test_cloudflare_response_error_deserialization() {
    let json = r#"{
        "success": false,
        "errors": [{"code": 1000, "message": "Invalid DNS record"}],
        "messages": ["Something went wrong"],
        "result": null
    }"#;
    let resp: CloudflareResponse<serde_json::Value> = serde_json::from_str(json).unwrap();
    assert!(!resp.success);
    assert_eq!(resp.errors.len(), 1);
    assert_eq!(resp.errors[0].code, 1000);
    assert_eq!(resp.errors[0].message, "Invalid DNS record");
    assert_eq!(resp.messages[0], "Something went wrong");
}

#[test]
fn test_cloudflare_response_with_pagination() {
    let json = r#"{
        "success": true,
        "errors": [],
        "messages": [],
        "result": [],
        "result_info": {
            "page": 1,
            "per_page": 20,
            "total_pages": 5,
            "count": 20,
            "total_count": 100
        }
    }"#;
    let resp: CloudflareResponse<Vec<serde_json::Value>> = serde_json::from_str(json).unwrap();
    assert!(resp.success);
    let info = resp.result_info.unwrap();
    assert_eq!(info.page, 1);
    assert_eq!(info.per_page, 20);
    assert_eq!(info.total_pages, 5);
    assert_eq!(info.count, 20);
    assert_eq!(info.total_count, 100);
}

// ── Zone deserialization ──

#[test]
fn test_zone_deserialization() {
    let json = r#"{
        "id": "zone_abc",
        "name": "example.com",
        "status": "active",
        "name_servers": ["ns1.cloudflare.com", "ns2.cloudflare.com"]
    }"#;
    let zone: Zone = serde_json::from_str(json).unwrap();
    assert_eq!(zone.id, "zone_abc");
    assert_eq!(zone.name, "example.com");
    assert!(matches!(zone.status, ZoneStatus::Active));
    assert_eq!(zone.name_servers.len(), 2);
}

// ── Generated types: what non-struct schemas, shared shapes and envelopes are ──

#[test]
fn test_a_free_form_object_schema_is_a_json_value_and_round_trips() {
    // `access_apps` has no properties: any JSON. It is `serde_json::Value`
    // itself, not a struct wrapping a map.
    use foundation_deployment_cloudflare::generated::zones::AccessApps;

    let json = serde_json::json!({ "code": 1000, "message": "ok", "nested": { "a": [1, 2] } });
    let parsed: AccessApps = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(parsed["code"], serde_json::json!(1000));
    assert_eq!(parsed["nested"], serde_json::json!({ "a": [1, 2] }));
    assert_eq!(serde_json::to_value(&parsed).unwrap(), json);
}

#[test]
fn test_named_list_and_number_schemas_read_what_cloudflare_sends() {
    // `access_messages` is a list of messages and `load_balancing_latitude` a
    // number. Both used to be generated as a struct around a JSON object, which
    // could deserialize neither.
    use foundation_deployment_cloudflare::generated::dns::AccessMessages;
    use foundation_deployment_cloudflare::generated::shared::LoadBalancingLatitude;

    let messages: AccessMessages =
        serde_json::from_value(serde_json::json!([{ "code": 1000, "message": "ok" }])).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["message"], serde_json::json!("ok"));
    assert!(serde_json::from_value::<AccessMessages>(serde_json::json!({ "code": 1000 })).is_err());

    let latitude: LoadBalancingLatitude = serde_json::from_value(serde_json::json!(37.77)).unwrap();
    assert!((latitude - 37.77).abs() < f64::EPSILON);
}

#[test]
fn test_a_deduplicated_alias_serialises_identically_to_its_canonical_type() {
    // Many response schemas share `{ errors, messages, success }`; they are
    // generated once (shared::ErrorsMessagesSuccessObject) and each keeps its
    // name as an alias, including struct-literal construction.
    use foundation_deployment_cloudflare::generated::dns::DnsFirewallApiResponseCommon;
    use foundation_deployment_cloudflare::generated::shared::ErrorsMessagesSuccessObject;

    let message = || {
        let mut m = std::collections::HashMap::new();
        m.insert("code".to_string(), serde_json::json!(1000));
        m
    };
    let via_alias = DnsFirewallApiResponseCommon {
        errors: vec![],
        messages: vec![message()],
        success: true,
    };
    let canonical = ErrorsMessagesSuccessObject {
        errors: vec![],
        messages: vec![message()],
        success: true,
    };
    let expected = serde_json::json!({ "errors": [], "messages": [{ "code": 1000 }], "success": true });
    assert_eq!(serde_json::to_value(&via_alias).unwrap(), expected);
    assert_eq!(serde_json::to_value(&canonical).unwrap(), expected);

    let back: DnsFirewallApiResponseCommon = serde_json::from_value(expected).unwrap();
    assert!(back.success);
    assert_eq!(back.messages.len(), 1);
}

#[test]
fn test_an_endpoint_response_reads_through_the_generic_envelope() {
    // `CacheApiResponseSingleId` is `ApiEnvelope<CacheResult>`: the shared
    // `{ errors, messages, result, success }` wrapper with a typed `result`.
    use foundation_deployment_cloudflare::generated::shared::ApiEnvelope;
    use foundation_deployment_cloudflare::generated::zones::{CacheApiResponseSingleId, CacheResult};

    let json = serde_json::json!({
        "errors": [],
        "messages": [],
        "success": true,
        "result": {
            "editable": true,
            "id": "cache_reserve",
            "modified_on": "2026-10-13T00:00:00Z",
            "value": "on"
        }
    });
    let response: CacheApiResponseSingleId = serde_json::from_value(json.clone()).unwrap();
    assert!(response.success);
    assert_eq!(response.result.id, "cache_reserve");
    assert!(response.result.editable);

    // The alias is the generic type, so code can be written against either.
    let generic: &ApiEnvelope<CacheResult> = &response;
    assert_eq!(serde_json::to_value(generic).unwrap()["result"]["id"], json["result"]["id"]);
}
