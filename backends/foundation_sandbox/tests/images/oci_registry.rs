//! The registry client tests (feature 15, section 6.2).
//!
//! The tests use the fake registry on a local socket. They need no network, no
//! Docker and no virtual machine.

use std::path::PathBuf;
use std::sync::Arc;

use foundation_sandbox::native::artifacts::sha256_hex;
use foundation_sandbox::native::host::NativeFs;
use foundation_sandbox::native::images::registry::{
    check_image_arch, layer_kind, OciDescriptor, OciImageConfigDocument, OciReference,
    RegistryClient, RegistryOptions,
};
use foundation_sandbox::shared::error::SandboxError;
use foundation_sandbox::shared::spec::GuestArch;
use serde_json::{json, Value};

use super::registry_server::{BlobBody, FakeRegistry};
use super::support::TempDir;

/// The media type of a gzip layer.
const GZIP_LAYER: &str = "application/vnd.oci.image.layer.v1.tar+gzip";
/// The media type of an uncompressed layer.
const PLAIN_LAYER: &str = "application/vnd.oci.image.layer.v1.tar";
/// The media type of an image manifest.
const OCI_MANIFEST: &str = "application/vnd.oci.image.manifest.v1+json";
/// The media type of an image index.
const OCI_INDEX: &str = "application/vnd.oci.image.index.v1+json";
/// The media type of an image configuration.
const OCI_CONFIG: &str = "application/vnd.oci.image.config.v1+json";

/// A client over a new temporary cache directory.
fn client(registry: &FakeRegistry, options: RegistryOptions) -> (RegistryClient, TempDir) {
    let directory = TempDir::new("oci-client");
    let ctx = crate::control::support::ctx(0x0c1).0;
    let options = RegistryOptions {
        allow_insecure: vec![registry.address().to_string()],
        ..options
    };
    let client = RegistryClient::new(
        ctx,
        Arc::new(NativeFs),
        directory.path().join("cache"),
        options,
    );
    (client, directory)
}

/// One reference to `/test/image` with `text` as its tag or digest.
fn reference(registry: &FakeRegistry, text: &str) -> OciReference {
    let address = registry.address();
    OciReference::parse(&format!("{address}/test/image:{text}")).expect("the test reference parses")
}

/// One reference to `/test/image` by digest.
fn digest_reference(registry: &FakeRegistry, digest: &str) -> OciReference {
    let address = registry.address();
    OciReference::parse(&format!("{address}/test/image@{digest}"))
        .expect("the test digest reference parses")
}

/// One descriptor JSON object.
fn descriptor(media_type: &str, digest: &str, size: u64) -> Value {
    json!({"mediaType": media_type, "digest": digest, "size": size})
}

/// One index entry JSON object.
fn index_entry(digest: &str, size: u64, os: &str, arch: &str, variant: Option<&str>) -> Value {
    let mut platform = json!({"os": os, "architecture": arch});
    if let Some(variant) = variant {
        platform["variant"] = json!(variant);
    }
    json!({
        "mediaType": OCI_MANIFEST,
        "digest": digest,
        "size": size,
        "platform": platform,
    })
}

/// One image manifest body.
fn manifest_body(config: &Value, layers: &[Value]) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": 2,
        "mediaType": OCI_MANIFEST,
        "config": config,
        "layers": layers,
    }))
    .expect("the manifest encodes")
}

/// One image index body.
fn index_body(entries: &[Value]) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": 2,
        "mediaType": OCI_INDEX,
        "manifests": entries,
    }))
    .expect("the index encodes")
}

/// One image configuration body.
fn config_body(architecture: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "architecture": architecture,
        "os": "linux",
        "config": {"Cmd": ["/bin/sh"], "Env": ["A=1"]},
    }))
    .expect("the configuration encodes")
}

/// The `amd64` or `arm64` word of this host.
fn host_word() -> &'static str {
    match GuestArch::host() {
        GuestArch::X86_64 => "amd64",
        GuestArch::Aarch64 => "arm64",
    }
}

/// The word of the other architecture.
fn other_word() -> &'static str {
    match GuestArch::host() {
        GuestArch::X86_64 => "arm64",
        GuestArch::Aarch64 => "amd64",
    }
}

/// The fields of an `InvalidSpec` error.
fn invalid_spec(error: &foundation_errstacks::ErrorTrace<SandboxError>) -> (String, String) {
    match error.current_context() {
        SandboxError::InvalidSpec { field, reason } => (field.clone(), reason.clone()),
        other => panic!("the error is InvalidSpec: {other:?}"),
    }
}

#[test]
fn manifest_digest_checked() {
    let registry = FakeRegistry::start();
    let layer = registry.add_blob(b"layer".to_vec());
    let config = registry.add_blob(config_body("amd64"));
    let body = manifest_body(
        &descriptor(OCI_CONFIG, &config, 7),
        &[descriptor(PLAIN_LAYER, &layer, 5)],
    );
    // The registry serves the body under a reference that is not its digest.
    let wrong = format!("sha256:{}", "0".repeat(64));
    registry.add_manifest(&wrong, OCI_MANIFEST, body);
    let (client, _directory) = client(&registry, RegistryOptions::default());
    let error =
        crate::pool::block_on(client.resolve_manifest(&digest_reference(&registry, &wrong)))
            .expect_err("a manifest that does not hash to its digest fails");
    assert!(
        matches!(
            error.current_context(),
            SandboxError::ArtifactHashMismatch { .. }
        ),
        "the error is ArtifactHashMismatch: {error:?}"
    );
    assert_eq!(
        registry.manifest_serves(&wrong),
        1,
        "the manifest was read one time"
    );
}

#[test]
fn blob_digest_checked() {
    let registry = FakeRegistry::start();
    let good = b"the good layer".to_vec();
    let digest = format!("sha256:{}", sha256_hex(&good));
    registry.add_blob_at(&digest, BlobBody::Bytes(b"the bad layer".to_vec()));
    let (client, directory) = client(&registry, RegistryOptions::default());
    let descriptor = OciDescriptor {
        media_type: PLAIN_LAYER.to_string(),
        digest: digest.clone(),
        size: u64::try_from(good.len()).unwrap(),
    };
    let error = crate::pool::block_on(client.fetch_blob(
        &reference(&registry, "1"),
        &descriptor,
        &mut |_| {},
    ))
    .expect_err("a blob that does not hash to its digest fails");
    assert!(
        matches!(
            error.current_context(),
            SandboxError::ArtifactHashMismatch { .. }
        ),
        "the error is ArtifactHashMismatch: {error:?}"
    );
    let path = client.blob_path(&digest).expect("the blob path");
    assert!(!path.exists(), "no blob file is left");
    let blobs = directory.path().join("cache/oci/blobs/sha256");
    let leftovers =
        std::fs::read_dir(&blobs).map_or(0, |entries| entries.filter_map(Result::ok).count());
    assert_eq!(leftovers, 0, "no temporary blob file is left");
}

#[test]
fn blob_size_checked() {
    let registry = FakeRegistry::start();
    let good = b"the good layer".to_vec();
    let digest = registry.add_blob(good.clone());
    let (client, _directory) = client(&registry, RegistryOptions::default());
    let descriptor = OciDescriptor {
        media_type: PLAIN_LAYER.to_string(),
        digest: digest.clone(),
        size: u64::try_from(good.len()).unwrap() + 1,
    };
    let error = crate::pool::block_on(client.fetch_blob(
        &reference(&registry, "1"),
        &descriptor,
        &mut |_| {},
    ))
    .expect_err("a blob that is shorter than the manifest size fails");
    assert!(
        matches!(
            error.current_context(),
            SandboxError::ArtifactHashMismatch { .. }
        ),
        "the error is ArtifactHashMismatch: {error:?}"
    );
    assert!(
        !client.blob_path(&digest).expect("the blob path").exists(),
        "no blob file is left"
    );
}

#[test]
fn index_platform_choice() {
    let registry = FakeRegistry::start();
    let layer = registry.add_blob(b"layer".to_vec());
    let config = registry.add_blob(config_body(host_word()));
    let host_manifest = manifest_body(
        &descriptor(OCI_CONFIG, &config, 7),
        &[descriptor(GZIP_LAYER, &layer, 5)],
    );
    let host_digest = format!("sha256:{}", sha256_hex(&host_manifest));
    registry.add_manifest(&host_digest, OCI_MANIFEST, host_manifest);

    let other_config = registry.add_blob(config_body(other_word()));
    let other_manifest = manifest_body(
        &descriptor(OCI_CONFIG, &other_config, 7),
        &[descriptor(GZIP_LAYER, &layer, 5)],
    );
    let other_digest = format!("sha256:{}", sha256_hex(&other_manifest));
    registry.add_manifest(&other_digest, OCI_MANIFEST, other_manifest);

    let entries = [
        index_entry(&other_digest, 10, "linux", other_word(), None),
        index_entry(&host_digest, 10, "linux", host_word(), None),
    ];
    registry.add_manifest("1", OCI_INDEX, index_body(&entries));

    let (client, _directory) = client(&registry, RegistryOptions::default());
    let resolved = crate::pool::block_on(client.resolve_manifest(&reference(&registry, "1")))
        .expect("the index resolves");
    assert_eq!(resolved.digest, host_digest, "the host manifest was chosen");
    assert_eq!(
        resolved.manifest.layers.len(),
        1,
        "the chosen manifest has its layer"
    );
    assert_eq!(
        registry.manifest_serves(&other_digest),
        0,
        "the other platform's manifest was not read"
    );
}

#[test]
fn no_platform_match() {
    let registry = FakeRegistry::start();
    let config = registry.add_blob(config_body(other_word()));
    let manifest = manifest_body(&descriptor(OCI_CONFIG, &config, 7), &[]);
    let digest = format!("sha256:{}", sha256_hex(&manifest));
    registry.add_manifest(&digest, OCI_MANIFEST, manifest);
    let entries = [index_entry(&digest, 10, "linux", other_word(), None)];
    registry.add_manifest("1", OCI_INDEX, index_body(&entries));

    let (client, _directory) = client(&registry, RegistryOptions::default());
    let error = crate::pool::block_on(client.resolve_manifest(&reference(&registry, "1")))
        .expect_err("an index without this platform fails");
    let SandboxError::ImageArchMismatch { image, .. } = error.current_context() else {
        panic!("the error is ImageArchMismatch: {error:?}");
    };
    assert!(
        image.contains(&format!("linux/{}", other_word())),
        "the message lists the platforms that exist: {image}"
    );
}

#[test]
fn manifest_arch_checked() {
    let config: OciImageConfigDocument =
        serde_json::from_slice(&config_body(other_word())).expect("the configuration parses");
    let error = check_image_arch(&config, GuestArch::host(), "test/image:1")
        .expect_err("another architecture fails");
    assert!(
        matches!(
            error.current_context(),
            SandboxError::ImageArchMismatch { .. }
        ),
        "the error is ImageArchMismatch: {error:?}"
    );
}

#[test]
fn unknown_layer_type() {
    let media_type = "application/vnd.oci.image.layer.v1.tar+zstd";
    let error = layer_kind(media_type).expect_err("a zstd layer is not read");
    let (field, reason) = invalid_spec(&error);
    assert_eq!(field, "mediaType", "the error names the media type field");
    assert!(
        reason.contains(media_type),
        "the reason names the media type: {reason}"
    );

    // The two supported forms are named.
    assert_eq!(
        layer_kind("application/vnd.oci.image.layer.v1.tar").expect("plain"),
        foundation_sandbox::native::images::registry::LayerKind::Plain
    );
    assert_eq!(
        layer_kind("application/vnd.docker.image.rootfs.diff.tar.gzip").expect("gzip"),
        foundation_sandbox::native::images::registry::LayerKind::Gzip
    );
}

#[test]
fn blob_cache_reuse() {
    let registry = FakeRegistry::start();
    let bytes = b"a shared layer".to_vec();
    let digest = registry.add_blob(bytes.clone());
    let (client, _directory) = client(&registry, RegistryOptions::default());
    let descriptor = OciDescriptor {
        media_type: PLAIN_LAYER.to_string(),
        digest: digest.clone(),
        size: u64::try_from(bytes.len()).unwrap(),
    };
    let reference = reference(&registry, "1");
    let first = crate::pool::block_on(client.fetch_blob(&reference, &descriptor, &mut |_| {}))
        .expect("the first fetch");
    let second = crate::pool::block_on(client.fetch_blob(&reference, &descriptor, &mut |_| {}))
        .expect("the second fetch");
    assert_eq!(first, second, "both fetches give the same cache file");
    assert_eq!(
        registry.blob_serves(&digest),
        1,
        "the registry served the blob one time"
    );
}

#[test]
fn blob_path_is_below_the_cache() {
    let directory = PathBuf::from("/cache");
    let ctx = crate::control::support::ctx(0x0c2).0;
    let client = RegistryClient::new(
        ctx,
        Arc::new(NativeFs),
        directory,
        RegistryOptions::default(),
    );
    let digest = format!("sha256:{}", "a".repeat(64));
    assert_eq!(
        client.blob_path(&digest).expect("the path"),
        PathBuf::from(format!("/cache/oci/blobs/sha256/{}", "a".repeat(64)))
    );
    let error = client.blob_path("md5:abc").expect_err("a bad digest fails");
    let (field, _) = invalid_spec(&error);
    assert_eq!(field, "digest");
}
