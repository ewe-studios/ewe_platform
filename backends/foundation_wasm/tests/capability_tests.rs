//! WHY: F23 — WasmCapability trait + CapabilityRegistry must work on native
//! and carry the correct payloads through register → invoke → response.
//!
//! WHAT: registry registration, lookup, invoke (ok + error), name validation.
//!
//! HOW: mock capability that echoes the request action in its response.

use foundation_wasm::{
    CapabilityContentType, CapabilityError, CapabilityRegistry, CapabilityRequest,
    CapabilityResponse, WasmCapability, is_valid_capability_name,
};

// ── Mock capability ─────────────────────────────────────────────────────

struct EchoCapability {
    name: &'static str,
}

impl WasmCapability for EchoCapability {
    fn name(&self) -> &str {
        self.name
    }

    fn invoke_capability(
        &self,
        request: &CapabilityRequest,
    ) -> Result<CapabilityResponse, CapabilityError> {
        Ok(CapabilityResponse {
            capability: request.capability.clone(),
            action: request.action.clone(),
            payload: request.payload.clone(),
            content_type: request.content_type,
        })
    }
}

struct FailingCapability;

impl WasmCapability for FailingCapability {
    fn name(&self) -> &str {
        "fail"
    }

    fn invoke_capability(
        &self,
        _request: &CapabilityRequest,
    ) -> Result<CapabilityResponse, CapabilityError> {
        Err(CapabilityError::ExecutionFailed("intentional failure".into()))
    }
}

// ── Registry tests ──────────────────────────────────────────────────────

#[test]
fn registry_register_and_invoke() {
    let reg = CapabilityRegistry::new();
    reg.register(EchoCapability { name: "echo" });

    let req = CapabilityRequest {
        capability: "echo".into(),
        action: "ping".into(),
        payload: b"hello".to_vec(),
        content_type: CapabilityContentType::Json,
    };

    let resp = reg.invoke(&req).unwrap();
    assert_eq!(resp.capability, "echo");
    assert_eq!(resp.action, "ping");
    assert_eq!(resp.payload, b"hello");
}

#[test]
fn registry_unknown_capability() {
    let reg = CapabilityRegistry::new();
    let req = CapabilityRequest {
        capability: "nonexistent".into(),
        action: "do".into(),
        payload: vec![],
        content_type: CapabilityContentType::Json,
    };

    match reg.invoke(&req) {
        Err(CapabilityError::UnknownCapability(name)) => assert_eq!(name, "nonexistent"),
        other => panic!("expected UnknownCapability, got {:?}", other),
    }
}

#[test]
fn registry_execution_error() {
    let reg = CapabilityRegistry::new();
    reg.register(FailingCapability);

    let req = CapabilityRequest {
        capability: "fail".into(),
        action: "boom".into(),
        payload: vec![],
        content_type: CapabilityContentType::Json,
    };

    match reg.invoke(&req) {
        Err(CapabilityError::ExecutionFailed(msg)) => assert!(msg.contains("intentional")),
        other => panic!("expected ExecutionFailed, got {:?}", other),
    }
}

#[test]
fn registry_get_and_names() {
    let reg = CapabilityRegistry::new();
    reg.register(EchoCapability { name: "echo" });
    reg.register(FailingCapability);

    assert!(reg.get("echo").is_some());
    assert!(reg.get("fail").is_some());
    assert!(reg.get("nonexistent").is_none());

    let mut names = reg.names();
    names.sort();
    assert_eq!(names, vec!["echo".to_string(), "fail".to_string()]);
}

#[test]
fn registry_replacement() {
    let reg = CapabilityRegistry::new();
    let old = reg.register(EchoCapability { name: "dup" });
    assert!(old.is_none());

    let old = reg.register(EchoCapability { name: "dup" });
    assert!(old.is_some());
}

// ── Name validation ─────────────────────────────────────────────────────

#[test]
fn valid_capability_names() {
    assert!(is_valid_capability_name("camera"));
    assert!(is_valid_capability_name("biometric_auth"));
    assert!(is_valid_capability_name("file-system"));
    assert!(is_valid_capability_name("a"));  // single char
    assert!(is_valid_capability_name(&"x".repeat(64)));  // max length
}

#[test]
fn invalid_capability_names() {
    assert!(!is_valid_capability_name(""));   // empty
    assert!(!is_valid_capability_name(&"x".repeat(65)));  // too long
    assert!(!is_valid_capability_name("has spaces"));
    assert!(!is_valid_capability_name("has/slash"));
    assert!(!is_valid_capability_name("has.dot"));
}

// ── Content type roundtrip ──────────────────────────────────────────────

#[test]
fn content_type_roundtrip() {
    // CapabilityContentType is Copy + Eq
    let json = CapabilityContentType::Json;
    let arrow = CapabilityContentType::Arrow;

    assert_eq!(json, json.clone());
    assert_ne!(json, arrow);
    assert_eq!(format!("{:?}", json), "Json");
}
