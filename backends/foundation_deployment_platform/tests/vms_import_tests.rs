//! Tests extracted from vms/import/ modules

#![cfg(feature = "vms")]

use std::io::{Read, Write};
use std::net::TcpListener;

use foundation_core::valtron::valtron_test;
use foundation_deployment_platform::config::TestbedError;
use foundation_deployment_platform::import::{
    vagrant_cloud_lookup, validate_image, MIN_IMAGE_SIZE,
};
use foundation_netio::HttpClientBuilder;

// From import/mod.rs
#[test]
fn test_min_image_size_constant() {
    assert_eq!(MIN_IMAGE_SIZE, 100 * 1_048_576);
}

#[test]
fn test_validate_image_rejects_small_file() {
    let path = std::path::PathBuf::from("/tmp/test_small_image.qcow2");
    std::fs::write(&path, b"tiny").unwrap();
    let err = validate_image(&path).unwrap_err();
    assert!(err.to_string().contains("failed download"));
    let _ = std::fs::remove_file(&path);
}

// From import/macos.rs
#[test]
fn test_config_plist_is_valid_xml() {
    let config = foundation_deployment_platform::import::macos::generate_config_plist();
    assert!(config.contains("<?xml"));
    assert!(config.contains("iMacPro1,1"));
    assert!(config.contains("boot-args"));
}

#[test]
fn test_config_plist_has_required_sections() {
    let config = foundation_deployment_platform::import::macos::generate_config_plist();
    assert!(config.contains("<key>PlatformInfo</key>"));
    assert!(config.contains("<key>SystemProductName</key>"));
    assert!(config.contains("<key>NVRAM</key>"));
    assert!(config.contains("<key>UEFI</key>"));
    assert!(config.contains("<key>Misc</key>"));
    assert!(config.contains("<key>Kernel</key>"));
}

// ── Vagrant Cloud lookup (E3) ───────────────────────────────────────────────

/// A Vagrant Cloud box metadata document with two providers. The libvirt
/// provider carries the download URL the lookup must return.
const VAGRANT_METADATA: &str = r#"{
  "tag": "test/ubuntu-24.04",
  "current_version": {
    "version": "1.0.0",
    "providers": [
      { "name": "virtualbox", "url": "https://unused.example/virtualbox.box" },
      {
        "name": "libvirt",
        "download_url": "https://dl.example/ubuntu-libvirt.box",
        "url": "https://dl.example/ubuntu-libvirt.box/fallback"
      }
    ]
  }
}"#;

/// Start a one-shot HTTP server on 127.0.0.1 that answers the first request
/// with `status` and `body`, and return its URL. The listener is moved into a
/// detached thread, so the server lives until it answers once.
fn serve_once(status: u16, body: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let addr = listener.local_addr().expect("local addr");
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept one request");
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf);
        let reason = if status == 200 { "OK" } else { "Not Found" };
        let head = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(head.as_bytes());
        let _ = stream.write_all(body.as_bytes());
        let _ = stream.flush();
    });
    format!("http://{addr}")
}

/// The metadata JSON is parsed into the typed struct and the libvirt
/// `download_url` is returned.
#[valtron_test]
async fn vagrant_lookup_parses_metadata() {
    let url = serve_once(200, VAGRANT_METADATA.to_string());
    let client = HttpClientBuilder::new().max_body_size(Some(64 * 1024)).build();

    let result = vagrant_cloud_lookup(&client, &url)
        .await
        .expect("the metadata should parse");

    assert_eq!(result.as_deref(), Some("https://dl.example/ubuntu-libvirt.box"));
}

/// A non-2xx response is a structured HTTP error, not a silent miss.
#[valtron_test]
async fn vagrant_lookup_reports_http_error() {
    let url = serve_once(404, "not found".to_string());
    let client = HttpClientBuilder::new().max_body_size(Some(64 * 1024)).build();

    let err = vagrant_cloud_lookup(&client, &url)
        .await
        .expect_err("a 404 should be reported");

    match err {
        TestbedError::DownloadFailed { status, url: failed_url } => {
            assert_eq!(status, 404);
            assert_eq!(failed_url, url);
        }
        other => panic!("expected DownloadFailed, got: {other:?}"),
    }
}
