//! Overlay capability tests moved out of `src/overlay.rs` (spec 70
//! feature 59, rule 7). Every item the tests use is public.

use foundation_platform::{OverlayCapability, OverlayConfig};
use foundation_wasm::{CapabilityContentType, CapabilityRequest, WasmCapability};

#[test]
fn overlay_config_default_to_js() {
    let config = OverlayConfig::default();
    let js = config.to_js();
    assert!(js.contains("bottom"));
    assert!(js.contains("show()"));
}

#[test]
fn overlay_config_hidden() {
    let config = OverlayConfig {
        visible: false,
        ..Default::default()
    };
    let js = config.to_js();
    assert!(js.contains("hide()"));
}

#[test]
fn overlay_config_with_title() {
    let config = OverlayConfig {
        title: Some("Settings".into()),
        ..Default::default()
    };
    let js = config.to_js();
    assert!(js.contains("Settings"));
}

#[test]
fn overlay_capability_show_action() {
    let cap = OverlayCapability;
    let req = CapabilityRequest {
        capability: "overlay".into(),
        action: "show".into(),
        payload: vec![],
        content_type: CapabilityContentType::Json,
    };
    let resp = cap.invoke_capability(&req).unwrap();
    let js = String::from_utf8(resp.payload).unwrap();
    assert!(js.contains("__eweNav.show()"));
}

#[test]
fn overlay_capability_hide_action() {
    let cap = OverlayCapability;
    let req = CapabilityRequest {
        capability: "overlay".into(),
        action: "hide".into(),
        payload: vec![],
        content_type: CapabilityContentType::Json,
    };
    let resp = cap.invoke_capability(&req).unwrap();
    let js = String::from_utf8(resp.payload).unwrap();
    assert!(js.contains("__eweNav.hide()"));
}

#[test]
fn overlay_capability_set_title() {
    let cap = OverlayCapability;
    let req = CapabilityRequest {
        capability: "overlay".into(),
        action: "set_title".into(),
        payload: b"Home".to_vec(),
        content_type: CapabilityContentType::Json,
    };
    let resp = cap.invoke_capability(&req).unwrap();
    let js = String::from_utf8(resp.payload).unwrap();
    assert!(js.contains("Home"));
}
