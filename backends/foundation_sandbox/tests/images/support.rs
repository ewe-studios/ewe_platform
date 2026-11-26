//! The support of the sandbox image tests (feature 12).
//!
//! The store tests use a real temporary directory and the real host file
//! system. The manager tests use the simulated driver and the scripted slave of
//! the control-plane tests, with a resolver that answers a fixed image.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use foundation_context::Ctx;
use foundation_sandbox::native::host::NativeFs;
use foundation_sandbox::native::images::ImageStore;
use foundation_sandbox::shared::control::testing::{FakeProcessSpawner, SimDriver};
use foundation_sandbox::shared::control::{MemoryRecordStore, RecordStore, SandboxManager};
use foundation_sandbox::shared::driver::{SandboxDriver, SlaveEndpoint};
use foundation_sandbox::shared::error::SandboxResult;
use foundation_sandbox::shared::host::{HostConfig, SandboxHost, SlaveConnector};
use foundation_sandbox::shared::ids::DriverId;
use foundation_sandbox::shared::images::{ImageManifest, ImageResolver};
use foundation_sandbox::shared::limits::{ArtifactLimits, ImageLimits, SandboxLimits};
use foundation_sandbox::shared::spec::{GuestArch, ImageRef};
use foundation_sandbox::shared::BoxFuture;

/// A temporary directory that removes itself.
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// A new empty directory with `tag` in its name.
    ///
    /// # Panics
    /// When the directory cannot be made.
    #[must_use]
    pub fn new(tag: &str) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let count = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path =
            std::env::temp_dir().join(format!("ewe-image-{tag}-{}-{count}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("the test directory is made");
        Self { path }
    }

    /// The path of the directory.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// A connector that fails every call: the store tests never start a sandbox.
pub struct NoConnector;

impl SlaveConnector for NoConnector {
    fn connect(
        &self,
        _ctx: &Ctx,
        _endpoint: &SlaveEndpoint,
        _limits: &SandboxLimits,
    ) -> SandboxResult<Arc<dyn foundation_connectrpc::shared::transport::Transport>> {
        Err(foundation_sandbox::shared::error::SandboxError::Io {
            op: "connect".to_string(),
            message: "the store tests start no sandbox".to_string(),
        }
        .into())
    }
}

/// A host over a temporary cache directory and the real file system.
#[must_use]
pub fn host(dir: &TempDir, limits: ImageLimits) -> SandboxHost {
    SandboxHost {
        processes: Arc::new(FakeProcessSpawner::new()),
        fs: Arc::new(NativeFs),
        connector: Arc::new(NoConnector),
        config: Arc::new(HostConfig {
            state_dir: dir.path().join("state"),
            cache_dir: dir.path().join("cache"),
            slave_path: None,
            firecracker_path: None,
            krun_runner_path: None,
            libkrun_path: None,
            mkfs_ext4_path: None,
            driver_order: Vec::new(),
            artifact_limits: ArtifactLimits::default(),
            image_limits: limits,
        }),
    }
}

/// An image store over a new temporary cache directory.
#[must_use]
pub fn store(tag: &str) -> (ImageStore, TempDir, Ctx) {
    let dir = TempDir::new(tag);
    let ctx = crate::control::support::ctx(91).0;
    let store = ImageStore::new(ctx.clone(), host(&dir, ImageLimits::default()));
    (store, dir, ctx)
}

/// A manifest for this host with `arch`.
#[must_use]
pub fn manifest(arch: GuestArch) -> ImageManifest {
    ImageManifest {
        schema: 1,
        key: String::new(),
        arch,
        created_unix_ms: 0,
        cmd: Some(vec!["/bin/tool".to_string(), "--serve".to_string()]),
        env: vec![("A".to_string(), "1".to_string())],
        workdir: Some("/srv".to_string()),
        user: None,
        kernel: None,
        rootfs_sha256: String::new(),
        rootfs_size: 0,
        source: "test".to_string(),
        steps: Vec::new(),
    }
}

/// A resolver that answers one fixed image.
pub struct FixedResolver {
    /// The root file that the create sees.
    pub rootfs: PathBuf,
    /// The manifest that the create applies.
    pub manifest: ImageManifest,
}

impl ImageResolver for FixedResolver {
    fn resolve<'a>(
        &'a self,
        _image: &'a ImageRef,
    ) -> BoxFuture<'a, SandboxResult<(PathBuf, ImageManifest)>> {
        let answer = Ok((self.rootfs.clone(), self.manifest.clone()));
        Box::pin(async move { answer })
    }
}

/// A manager over one simulated driver, one scripted slave and a resolver.
#[must_use]
pub fn image_manager(
    ctx: Ctx,
    driver: Arc<SimDriver>,
    slave: Arc<foundation_sandbox::shared::control::testing::ScriptedSlave>,
    resolver: Arc<dyn ImageResolver>,
) -> SandboxManager {
    let store = Arc::new(MemoryRecordStore::new());
    SandboxManager::new(
        ctx,
        vec![driver as Arc<dyn SandboxDriver>],
        foundation_sandbox::shared::control::testing::in_memory_connector(slave),
        store as Arc<dyn RecordStore>,
        foundation_sandbox::shared::control::ManagerConfig::default(),
    )
    .with_image_resolver(resolver)
}

/// The architecture that this host does not run.
#[must_use]
pub fn other_arch() -> GuestArch {
    match GuestArch::host() {
        GuestArch::X86_64 => GuestArch::Aarch64,
        GuestArch::Aarch64 => GuestArch::X86_64,
    }
}

/// The fixed driver identity of the image tests.
#[must_use]
pub fn driver_id() -> DriverId {
    DriverId::Firecracker
}

/// An image limit set with a very short work age.
#[must_use]
pub fn tiny_age() -> ImageLimits {
    ImageLimits::default().with_work_max_age(Duration::from_secs(1))
}
