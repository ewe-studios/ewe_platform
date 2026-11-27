//! The registry pull in the import story (feature 15, section 6.4).
//!
//! The tests use the fake registry, the simulated driver and the scripted
//! slave, so no registry, no Docker daemon and no virtual machine run.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use foundation_context::Ctx;
use foundation_sandbox::native::artifacts::sha256_hex;
use foundation_sandbox::native::host::NativeFs;
use foundation_sandbox::native::images::import::{
    ImageImporter, ImageSource, ImportOptions, ImportReport,
};
use foundation_sandbox::native::images::registry::{
    digest_hex, OciDescriptor, OciReference, RegistryClient, RegistryOptions,
};
use foundation_sandbox::native::images::ImageStore;
use foundation_sandbox::shared::control::testing::{
    in_memory_connector, FakeHostFs, FakeProcessSpawner, ScriptedSlave, SimDriver,
};
use foundation_sandbox::shared::control::{
    ManagerConfig, MemoryRecordStore, SandboxManager, SandboxProvider,
};
use foundation_sandbox::shared::driver::SandboxDriver;
use foundation_sandbox::shared::error::{SandboxError, SandboxResult, SlaveErrorCode};
use foundation_sandbox::shared::host::{FileLock, HostConfig, HostFs, SandboxHost};
use foundation_sandbox::shared::ids::DriverId;
use foundation_sandbox::shared::limits::{ArtifactLimits, ImageLimits};
use foundation_sandbox::shared::spec::GuestArch;
use serde_json::{json, Value};

use super::registry_server::{sha256_repeat, BlobBody, FakeRegistry};
use super::support::TempDir;

/// The media type of an uncompressed layer.
const PLAIN_LAYER: &str = "application/vnd.oci.image.layer.v1.tar";
/// The media type of a gzip layer.
const GZIP_LAYER: &str = "application/vnd.oci.image.layer.v1.tar+gzip";
/// The media type that this feature does not read.
const ZSTD_LAYER: &str = "application/vnd.oci.image.layer.v1.tar+zstd";
/// The media type of an image manifest.
const OCI_MANIFEST: &str = "application/vnd.oci.image.manifest.v1+json";
/// The media type of an image configuration.
const OCI_CONFIG: &str = "application/vnd.oci.image.config.v1+json";

/// The parts of one test image.
struct Fixture {
    /// The digest of the manifest.
    manifest: String,
    /// The digest of the configuration.
    configuration: String,
    /// The digest of the layer.
    layer: String,
}

/// The `amd64` or `arm64` word of this host.
fn host_word() -> &'static str {
    match GuestArch::host() {
        GuestArch::X86_64 => "amd64",
        GuestArch::Aarch64 => "arm64",
    }
}

/// One descriptor JSON object.
fn descriptor(media_type: &str, digest: &str, size: u64) -> Value {
    json!({"mediaType": media_type, "digest": digest, "size": size})
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

/// The inner container configuration of the test image.
fn config() -> Value {
    json!({
        "Entrypoint": ["/app"],
        "Cmd": ["--serve"],
        "Env": ["A=1"],
        "WorkingDir": "/srv",
        "User": "1000",
        "ExposedPorts": {"80/tcp": {}},
    })
}

/// One tar file with one regular file.
fn tar_with(path: &str, contents: &[u8]) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_path(path).expect("the header path is set");
    header.set_size(u64::try_from(contents.len()).unwrap());
    header.set_mode(0o644);
    header.set_cksum();
    builder
        .append(&header, contents)
        .expect("the file goes into the tar");
    builder.into_inner().expect("the tar is complete")
}

/// One gzip stream of `bytes`.
fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes).expect("the layer compresses");
    encoder.finish().expect("the layer compresses")
}

/// Adds one image with one layer and returns its parts.
fn add_image(
    registry: &FakeRegistry,
    tags: &[&str],
    layer: &[u8],
    media_type: &str,
    config: &Value,
) -> Fixture {
    let config_bytes = serde_json::to_vec(&json!({
        "architecture": host_word(),
        "os": "linux",
        "config": config,
    }))
    .expect("the configuration encodes");
    let config_digest = registry.add_blob(config_bytes.clone());
    let layer_digest = registry.add_blob(layer.to_vec());
    add_image_parts(
        registry,
        tags,
        &config_digest,
        u64::try_from(config_bytes.len()).unwrap(),
        &layer_digest,
        u64::try_from(layer.len()).unwrap(),
        media_type,
    )
}

/// Adds one image whose blobs the caller already added.
fn add_image_parts(
    registry: &FakeRegistry,
    tags: &[&str],
    config_digest: &str,
    config_size: u64,
    layer_digest: &str,
    layer_size: u64,
    media_type: &str,
) -> Fixture {
    let body = manifest_body(
        &descriptor(OCI_CONFIG, config_digest, config_size),
        &[descriptor(media_type, layer_digest, layer_size)],
    );
    let manifest_digest = format!("sha256:{}", sha256_hex(&body));
    for tag in tags {
        registry.add_manifest(tag, OCI_MANIFEST, body.clone());
    }
    Fixture {
        manifest: manifest_digest,
        configuration: config_digest.to_string(),
        layer: layer_digest.to_string(),
    }
}

/// One import harness over the fake registry.
struct Harness {
    importer: ImageImporter,
    registry: FakeRegistry,
    driver: Arc<SimDriver>,
    slave: Arc<ScriptedSlave>,
    fs: FakeHostFs,
    store: ImageStore,
    ctx: Ctx,
}

impl Harness {
    /// A harness over a new fake registry.
    fn new(seed: u64) -> Self {
        Self::with_limits(seed, ImageLimits::default())
    }

    /// A harness with the given image limits.
    fn with_limits(seed: u64, limits: ImageLimits) -> Self {
        let registry = FakeRegistry::start();
        let (ctx, _clock) = crate::control::support::ctx(seed);
        let driver = Arc::new(SimDriver::new(DriverId::Firecracker).with_fresh_substrate());
        let slave = Arc::new(ScriptedSlave::new());
        slave.with_shutdown_exit(driver.substrate());
        let fs = FakeHostFs::new();
        fs.insert_file("/bin/agent-slave", b"slave");
        fs.insert_file("/bin/mkfs.ext4", b"mkfs");
        let spawner = Arc::new(FakeProcessSpawner::new());
        spawner.with_program_output(fs.clone(), "mkfs.ext4", "");
        let config = HostConfig {
            state_dir: "/state".into(),
            cache_dir: "/cache".into(),
            slave_path: Some(PathBuf::from("/bin/agent-slave")),
            firecracker_path: None,
            krun_runner_path: None,
            libkrun_path: None,
            mkfs_ext4_path: Some(PathBuf::from("/bin/mkfs.ext4")),
            driver_order: vec![DriverId::Firecracker],
            artifact_limits: ArtifactLimits::default(),
            image_limits: limits,
        };
        let host = SandboxHost {
            processes: spawner,
            fs: Arc::new(fs.clone()),
            connector: Arc::new(super::support::NoConnector),
            config: Arc::new(config),
        };
        let store = ImageStore::new(ctx.clone(), host.clone());
        let manager: Arc<dyn SandboxProvider> = Arc::new(SandboxManager::new(
            ctx.clone(),
            vec![Arc::clone(&driver) as Arc<dyn SandboxDriver>],
            in_memory_connector(Arc::clone(&slave)),
            Arc::new(MemoryRecordStore::new()),
            ManagerConfig::default(),
        ));
        let registry_options = RegistryOptions {
            allow_insecure: vec![registry.address().to_string()],
            ..RegistryOptions::default()
        };
        let importer = ImageImporter::new(ctx.clone(), host, store.clone(), manager, None)
            .with_registry(registry_options);
        Self {
            importer,
            registry,
            driver,
            slave,
            fs,
            store,
            ctx,
        }
    }

    /// The source text of one tag in the fake registry.
    fn source(&self, tag: &str) -> String {
        format!("{}/test/image:{tag}", self.registry.address())
    }

    /// Imports one tag with an optional local image tag.
    fn import(&self, tag: &str, local: Option<&str>) -> SandboxResult<ImportReport> {
        self.import_source(
            ImageSource::Oci(self.source(tag)),
            ImportOptions {
                tag: local.map(str::to_string),
                ..ImportOptions::default()
            },
        )
    }

    /// Imports one source.
    fn import_source(
        &self,
        source: ImageSource,
        options: ImportOptions,
    ) -> SandboxResult<ImportReport> {
        crate::pool::block_on(self.importer.import(source, options))
    }

    /// The number of work files that are left.
    fn work_files(&self) -> usize {
        self.fs
            .read_dir(Path::new("/cache/images/work"))
            .map_or(0, |entries| entries.len())
    }

    /// The number of images in the store.
    fn image_count(&self) -> usize {
        crate::pool::block_on(self.store.list())
            .expect("the store lists")
            .len()
    }

    /// Whether the cache holds the blob of `digest`.
    fn has_blob(&self, digest: &str) -> bool {
        let Ok(hex) = foundation_sandbox::native::images::registry::digest_hex(digest) else {
            return false;
        };
        self.fs
            .exists(&PathBuf::from(format!("/cache/oci/blobs/sha256/{hex}")))
    }
}

#[test]
fn pull_import_ok() {
    let harness = Harness::new(0x501);
    let layer = gzip(&tar_with("app", b"hello"));
    let fixture = add_image(&harness.registry, &["1"], &layer, GZIP_LAYER, &config());
    let report = harness
        .import("1", Some("oci-image"))
        .expect("the registry image imports");
    assert!(!report.cached);
    assert_eq!(
        report.image,
        foundation_sandbox::shared::spec::ImageRef::Tag("oci-image".to_string())
    );
    assert_eq!(
        report.manifest.cmd,
        Some(vec!["/app".to_string(), "--serve".to_string()])
    );
    assert_eq!(
        report.manifest.env,
        vec![("A".to_string(), "1".to_string())]
    );
    assert_eq!(report.manifest.workdir.as_deref(), Some("/srv"));
    assert_eq!(report.manifest.user.as_deref(), Some("1000"));
    assert_eq!(report.manifest.arch, GuestArch::host());
    assert_eq!(
        report.manifest.source,
        format!(
            "oci:{}/test/image@{}",
            harness.registry.address(),
            fixture.manifest
        )
    );
    assert_eq!(report.dropped_config, vec!["ExposedPorts".to_string()]);
    assert_eq!(harness.driver.launch_calls(), 1, "one fill sandbox");
    assert_eq!(harness.image_count(), 1);
    assert_eq!(harness.work_files(), 0);
    // The store resolves the tag that the import set.
    let (rootfs, manifest) = crate::pool::block_on(harness.store.resolve(
        &foundation_sandbox::shared::spec::ImageRef::Tag("oci-image".to_string()),
    ))
    .expect("the imported tag resolves");
    assert!(rootfs.ends_with("rootfs.ext4"));
    assert_eq!(manifest.key, report.manifest.key);
}

#[test]
fn two_tags_one_image() {
    let harness = Harness::new(0x502);
    let layer = gzip(&tar_with("app", b"hello"));
    let fixture = add_image(
        &harness.registry,
        &["1", "2"],
        &layer,
        GZIP_LAYER,
        &config(),
    );
    let first = harness.import("1", Some("first")).expect("the first tag");
    let second = harness.import("2", Some("second")).expect("the second tag");
    assert!(!first.cached);
    assert!(second.cached, "the second tag is a cache hit");
    assert_eq!(
        first.manifest.key, second.manifest.key,
        "two tags give one image"
    );
    assert_eq!(
        harness.driver.launch_calls(),
        1,
        "a cache hit fills nothing"
    );
    assert_eq!(
        harness.registry.blob_serves(&fixture.configuration),
        1,
        "the configuration came from the cache"
    );
    assert_eq!(
        harness.registry.blob_serves(&fixture.layer),
        1,
        "the layer came from the cache"
    );
}

#[test]
fn shared_layer_downloads_once() {
    let harness = Harness::new(0x503);
    let layer = gzip(&tar_with("app", b"shared"));
    let layer_digest = harness.registry.add_blob(layer.clone());
    let layer_size = u64::try_from(layer.len()).unwrap();
    let first_config = serde_json::to_vec(&json!({
        "architecture": host_word(), "os": "linux", "config": {"Cmd": ["/a"]}
    }))
    .expect("the configuration encodes");
    let second_config = serde_json::to_vec(&json!({
        "architecture": host_word(), "os": "linux", "config": {"Cmd": ["/b"]}
    }))
    .expect("the configuration encodes");
    let first_config_digest = harness.registry.add_blob(first_config.clone());
    let second_config_digest = harness.registry.add_blob(second_config.clone());
    add_image_parts(
        &harness.registry,
        &["a"],
        &first_config_digest,
        u64::try_from(first_config.len()).unwrap(),
        &layer_digest,
        layer_size,
        GZIP_LAYER,
    );
    add_image_parts(
        &harness.registry,
        &["b"],
        &second_config_digest,
        u64::try_from(second_config.len()).unwrap(),
        &layer_digest,
        layer_size,
        GZIP_LAYER,
    );
    harness
        .import("a", None)
        .unwrap_or_else(|error| panic!("the first image: {error:?}"));
    harness
        .import("b", None)
        .unwrap_or_else(|error| panic!("the second image: {error:?}"));
    assert_eq!(
        harness.registry.blob_serves(&layer_digest),
        1,
        "the shared layer downloads one time"
    );
}

// ── the fault table of section 6.4 ────────────────────────────────────────

#[test]
fn pull_fault_matrix() {
    let _serial = crate::control::support::serial();
    fault_reference();
    fault_unreachable();
    fault_status();
    fault_authentication();
    fault_digest();
    fault_media_type();
    fault_unpack();
}

/// A reference that does not parse names the reference field.
fn fault_reference() {
    let harness = Harness::new(0x511);
    let error = harness
        .import_source(
            ImageSource::Oci("not a reference".to_string()),
            ImportOptions::default(),
        )
        .expect_err("a bad reference fails");
    match error.current_context() {
        SandboxError::InvalidSpec { field, .. } => assert_eq!(field, "reference"),
        other => panic!("the error is InvalidSpec: {other:?}"),
    }
    assert_eq!(harness.image_count(), 0);
    assert_eq!(harness.work_files(), 0);
}

/// A registry that is not reachable is an I/O error of the registry.
fn fault_unreachable() {
    let harness = Harness::new(0x512);
    let error = harness
        .import_source(
            ImageSource::Oci("127.0.0.1:1/test/image:1".to_string()),
            ImportOptions::default(),
        )
        .expect_err("an unreachable registry fails");
    assert!(
        matches!(
            error.current_context(),
            SandboxError::Io { op, .. } if op == "registry"
        ),
        "the error is Io of the registry: {error:?}"
    );
    assert_eq!(harness.image_count(), 0);
}

/// A status that is not 200 names the status.
fn fault_status() {
    let harness = Harness::new(0x513);
    let layer = tar_with("app", b"hello");
    add_image(&harness.registry, &["1"], &layer, PLAIN_LAYER, &config());
    harness.registry.state().manifest_status = Some((500, "the registry failed".to_string()));
    let error = harness.import("1", None).expect_err("a 500 answer fails");
    match error.current_context() {
        SandboxError::Io { op, message } => {
            assert_eq!(op, "registry");
            assert!(
                message.contains("500"),
                "the error names the status: {message}"
            );
        }
        other => panic!("the error is Io: {other:?}"),
    }
    assert_eq!(harness.image_count(), 0);
    assert_eq!(harness.work_files(), 0);
}

/// An authentication that does not pass is Unauthorized.
fn fault_authentication() {
    let harness = Harness::new(0x514);
    let layer = tar_with("app", b"hello");
    add_image(&harness.registry, &["1"], &layer, PLAIN_LAYER, &config());
    harness.registry.state().always_unauthorized = true;
    let error = harness
        .import("1", None)
        .expect_err("an unauthorized registry fails");
    assert!(
        matches!(error.current_context(), SandboxError::Unauthorized),
        "the error is Unauthorized: {error:?}"
    );
    assert_eq!(harness.image_count(), 0);
}

/// A layer whose bytes do not match its digest leaves no partial blob.
fn fault_digest() {
    let harness = Harness::new(0x515);
    let layer = b"the good layer".to_vec();
    let fixture = add_image(&harness.registry, &["1"], &layer, PLAIN_LAYER, &config());
    harness
        .registry
        .add_blob_at(&fixture.layer, BlobBody::Bytes(b"the bad layer".to_vec()));
    let error = harness.import("1", None).expect_err("a bad layer fails");
    assert!(
        matches!(
            error.current_context(),
            SandboxError::ArtifactHashMismatch { .. }
        ),
        "the error is ArtifactHashMismatch: {error:?}"
    );
    assert_eq!(harness.image_count(), 0);
    assert_eq!(harness.work_files(), 0);
    assert!(
        !harness.has_blob(&fixture.layer),
        "no partial layer blob is left"
    );
}

/// A layer media type that this feature does not read is named.
fn fault_media_type() {
    let harness = Harness::new(0x516);
    let layer = b"a zstd layer".to_vec();
    let fixture = add_image(&harness.registry, &["1"], &layer, ZSTD_LAYER, &config());
    let error = harness.import("1", None).expect_err("a zstd layer fails");
    let SandboxError::InvalidSpec { reason, .. } = error.current_context() else {
        panic!("the error is InvalidSpec: {error:?}");
    };
    assert!(
        reason.contains(ZSTD_LAYER),
        "the error names the media type: {reason}"
    );
    assert_eq!(harness.image_count(), 0);
    assert_eq!(harness.work_files(), 0);
    assert!(
        harness.has_blob(&fixture.configuration),
        "the verified configuration blob stays in the cache"
    );
}

/// A layer that does not unpack names the layer index.
fn fault_unpack() {
    let harness = Harness::new(0x517);
    let layer = tar_with("app", b"hello");
    add_image(&harness.registry, &["1"], &layer, PLAIN_LAYER, &config());
    harness.slave.with_unpack_error(SandboxError::SlaveError {
        code: SlaveErrorCode::Io,
        errno: None,
        message: "the layer is bad".to_string(),
    });
    let error = harness
        .import("1", None)
        .expect_err("a failed unpack fails the import");
    let SandboxError::SlaveError { message, .. } = error.current_context() else {
        panic!("the error is a SlaveError: {error:?}");
    };
    assert!(
        message.starts_with("layer 0:"),
        "the error names the layer index: {message}"
    );
    assert_eq!(harness.image_count(), 0);
    assert_eq!(harness.work_files(), 0);
}

// ── the streaming bound (invariant I3) ────────────────────────────────────

#[test]
fn layer_streams() {
    // A 200 MiB layer arrives in 64 KiB pieces; the client's largest write is
    // one piece, not the whole layer.
    let registry = FakeRegistry::start();
    let chunk = vec![0x5a_u8; 64 * 1024];
    let total = 200 * 1024 * 1024_u64;
    let digest = format!("sha256:{}", sha256_repeat(&chunk, total));
    registry.add_blob_at(&digest, BlobBody::Repeat { chunk, total });
    let directory = TempDir::new("oci-layer-streams");
    let ctx = crate::control::support::ctx(0x521).0;
    let fs = RecordingFs::new();
    let options = RegistryOptions {
        allow_insecure: vec![registry.address().to_string()],
        ..RegistryOptions::default()
    };
    let client = RegistryClient::new(
        ctx,
        Arc::new(fs.clone()),
        directory.path().join("cache"),
        options,
    );
    let reference = OciReference::parse(&format!("{}/test/image:1", registry.address()))
        .expect("the reference parses");
    let descriptor = OciDescriptor {
        media_type: PLAIN_LAYER.to_string(),
        digest,
        size: total,
    };
    let path = crate::pool::block_on(client.fetch_blob(&reference, &descriptor, &mut |_| {}))
        .expect("the large layer arrives");
    let size = std::fs::metadata(&path).expect("the cached layer").len();
    assert_eq!(size, total, "the cached layer has every byte");
    assert!(
        fs.max_write() < 1024 * 1024,
        "the largest write stays under 1 MiB: {}",
        fs.max_write()
    );
    assert!(
        fs.written() >= total,
        "the writer saw every byte: {}",
        fs.written()
    );
}

// ── the blob cache limit (feature 15, section 6.4) ────────────────────────

/// A second image whose configuration differs from [`config`].
fn other_config() -> Value {
    let mut config = config();
    config["Cmd"] = json!(["/other"]);
    config
}

/// The cached path of one blob, and its use marker.
fn blob_paths(digest: &str) -> (PathBuf, PathBuf) {
    let hex = digest_hex(digest).expect("the digest is a sha256 digest");
    let blob = PathBuf::from(format!("/cache/oci/blobs/sha256/{hex}"));
    let used = PathBuf::from(format!("/cache/oci/blobs/sha256/{hex}.used"));
    (blob, used)
}

#[test]
fn blob_cache_keeps_a_recent_blob() {
    let _serial = crate::control::support::serial();
    let limits = ImageLimits::default()
        .with_blob_cache_max_bytes(1)
        .with_blob_cache_min_age(Duration::from_hours(1));
    let harness = Harness::with_limits(0x531, limits);
    let first = add_image(
        &harness.registry,
        &["a"],
        &gzip(&tar_with("a", b"the first image")),
        GZIP_LAYER,
        &config(),
    );
    let second = add_image(
        &harness.registry,
        &["b"],
        &gzip(&tar_with("b", b"the second image")),
        GZIP_LAYER,
        &other_config(),
    );
    // Two importers over the same cache directory.
    let importer_a = harness.importer.clone();
    let importer_b = harness.importer.clone();
    crate::pool::block_on(importer_a.import(
        ImageSource::Oci(harness.source("a")),
        ImportOptions::default(),
    ))
    .expect("the first image imports");

    // The first import used its blobs just now.
    let now = harness.ctx.clock().wall_now();
    for digest in [&first.configuration, &first.layer] {
        let (blob, used) = blob_paths(digest);
        harness.fs.set_modified(blob, now);
        harness.fs.set_modified(used, now);
    }

    crate::pool::block_on(importer_b.import(
        ImageSource::Oci(harness.source("b")),
        ImportOptions::default(),
    ))
    .expect("the second image imports");

    assert!(
        harness.has_blob(&first.configuration),
        "a recently used configuration is kept"
    );
    assert!(
        harness.has_blob(&first.layer),
        "a recently used layer is kept"
    );
    assert!(
        harness.has_blob(&second.layer),
        "the running import's layer is kept"
    );
}

#[test]
fn blob_cache_removes_an_old_blob() {
    let _serial = crate::control::support::serial();
    let limits = ImageLimits::default()
        .with_blob_cache_max_bytes(1)
        .with_blob_cache_min_age(Duration::ZERO);
    let harness = Harness::with_limits(0x532, limits);
    let first = add_image(
        &harness.registry,
        &["a"],
        &gzip(&tar_with("a", b"the first image")),
        GZIP_LAYER,
        &config(),
    );
    let second = add_image(
        &harness.registry,
        &["b"],
        &gzip(&tar_with("b", b"the second image")),
        GZIP_LAYER,
        &other_config(),
    );
    let importer_a = harness.importer.clone();
    let importer_b = harness.importer.clone();
    crate::pool::block_on(importer_a.import(
        ImageSource::Oci(harness.source("a")),
        ImportOptions::default(),
    ))
    .expect("the first image imports");

    // The first image's blobs are old, so the second import may remove them.
    for digest in [&first.configuration, &first.layer] {
        let (blob, used) = blob_paths(digest);
        harness
            .fs
            .set_modified(blob, foundation_compact::SystemTime::UNIX_EPOCH);
        harness
            .fs
            .set_modified(used, foundation_compact::SystemTime::UNIX_EPOCH);
    }

    crate::pool::block_on(importer_b.import(
        ImageSource::Oci(harness.source("b")),
        ImportOptions::default(),
    ))
    .expect("the second image imports");

    assert!(
        !harness.has_blob(&first.configuration),
        "an old configuration is removed"
    );
    assert!(!harness.has_blob(&first.layer), "an old layer is removed");
    assert!(
        harness.has_blob(&second.configuration),
        "the running import's configuration is kept"
    );
    assert!(
        harness.has_blob(&second.layer),
        "the running import's layer is kept"
    );
}

/// A host file system that records the largest single write.
#[derive(Clone, Debug, Default)]
struct RecordingFs {
    max_write: Arc<AtomicUsize>,
    written: Arc<AtomicU64>,
}

impl RecordingFs {
    /// An empty recorder.
    #[must_use]
    fn new() -> Self {
        Self::default()
    }

    /// The largest buffer of one write call.
    fn max_write(&self) -> usize {
        self.max_write.load(Ordering::SeqCst)
    }

    /// The bytes written.
    fn written(&self) -> u64 {
        self.written.load(Ordering::SeqCst)
    }
}

impl HostFs for RecordingFs {
    fn create_dir_all(&self, path: &Path, mode: u32) -> SandboxResult<()> {
        NativeFs.create_dir_all(path, mode)
    }

    fn remove_dir_all(&self, path: &Path) -> SandboxResult<()> {
        NativeFs.remove_dir_all(path)
    }

    fn remove_file(&self, path: &Path) -> SandboxResult<()> {
        NativeFs.remove_file(path)
    }

    fn open_read(&self, path: &Path) -> SandboxResult<Box<dyn std::io::Read + Send + Sync>> {
        NativeFs.open_read(path)
    }

    fn file_size(&self, path: &Path) -> SandboxResult<u64> {
        NativeFs.file_size(path)
    }

    fn write(&self, path: &Path, data: &[u8]) -> SandboxResult<()> {
        NativeFs.write(path, data)
    }

    fn read(&self, path: &Path, max: usize) -> SandboxResult<Vec<u8>> {
        NativeFs.read(path, max)
    }

    fn tail(&self, path: &Path, max: usize) -> SandboxResult<Vec<u8>> {
        NativeFs.tail(path, max)
    }

    fn exists(&self, path: &Path) -> bool {
        NativeFs.exists(path)
    }

    fn rename(&self, from: &Path, to: &Path) -> SandboxResult<()> {
        NativeFs.rename(from, to)
    }

    fn read_stream(&self, path: &Path) -> SandboxResult<Box<dyn std::io::Read + Send>> {
        NativeFs.read_stream(path)
    }

    fn clone_file(&self, from: &Path, to: &Path) -> SandboxResult<()> {
        NativeFs.clone_file(from, to)
    }

    fn lock_exclusive(&self, path: &Path) -> SandboxResult<FileLock> {
        NativeFs.lock_exclusive(path)
    }

    fn create_file(&self, path: &Path) -> SandboxResult<Box<dyn std::io::Write + Send>> {
        let inner = NativeFs.create_file(path)?;
        Ok(Box::new(RecordingWriter {
            inner,
            max_write: Arc::clone(&self.max_write),
            written: Arc::clone(&self.written),
        }))
    }
}

/// A writer that records its largest call and its byte count.
struct RecordingWriter {
    inner: Box<dyn std::io::Write + Send>,
    max_write: Arc<AtomicUsize>,
    written: Arc<AtomicU64>,
}

impl std::io::Write for RecordingWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.max_write.fetch_max(buffer.len(), Ordering::SeqCst);
        self.written.fetch_add(
            u64::try_from(buffer.len()).unwrap_or(u64::MAX),
            Ordering::SeqCst,
        );
        self.inner.write(buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}
