//! The support of the `Sandboxfile` builder tests.
//!
//! A harness owns one temporary cache directory, one simulated driver and one
//! scripted slave. The image store reads real files, and no sandbox runs.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use foundation_context::Ctx;
use foundation_sandbox::native::host::NativeFs;
use foundation_sandbox::native::images::build::executor::{
    BuildEvent, BuildProgress, ImageBuilder,
};
use foundation_sandbox::native::images::build::BuildOptions;
use foundation_sandbox::native::images::import::ImageImporter;
use foundation_sandbox::native::images::sandboxfile::Sandboxfile;
use foundation_sandbox::native::images::ImageStore;
use foundation_sandbox::shared::control::testing::{
    in_memory_connector, FakeProcessSpawner, ScriptedSlave, SimDriver,
};
use foundation_sandbox::shared::control::{
    ManagerConfig, MemoryRecordStore, SandboxManager, SandboxProvider,
};
use foundation_sandbox::shared::driver::{Capabilities, HostReport, SandboxDriver};
use foundation_sandbox::shared::error::SandboxResult;
use foundation_sandbox::shared::host::{HostConfig, SandboxHost, SlaveConnector};
use foundation_sandbox::shared::ids::DriverId;
use foundation_sandbox::shared::images::ImageManifest;
use foundation_sandbox::shared::limits::{ArtifactLimits, ImageLimits};
use foundation_sandbox::shared::spec::{GuestArch, ImageRef, RootMode};

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
        let path = std::env::temp_dir().join(format!(
            "ewe-sandboxfile-{tag}-{}-{count}",
            std::process::id()
        ));
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

/// A connector that fails every call: the harness never dials a real slave.
struct NoConnector;

impl SlaveConnector for NoConnector {
    fn connect(
        &self,
        _ctx: &Ctx,
        _endpoint: &foundation_sandbox::shared::driver::SlaveEndpoint,
        _limits: &foundation_sandbox::shared::limits::SandboxLimits,
    ) -> SandboxResult<Arc<dyn foundation_connectrpc::shared::transport::Transport>> {
        Err(foundation_sandbox::shared::error::SandboxError::Io {
            op: "connect".to_string(),
            message: "the builder tests start no real sandbox".to_string(),
        }
        .into())
    }
}

/// One build harness over a cache directory.
pub struct Harness {
    /// The injected context.
    pub ctx: Ctx,
    /// The simulated driver.
    pub driver: Arc<SimDriver>,
    /// The scripted slave.
    pub slave: Arc<ScriptedSlave>,
    /// The image store.
    pub store: ImageStore,
    /// The builder under test.
    pub builder: ImageBuilder,
    /// The host bundle.
    pub host: SandboxHost,
    /// The cache directory.
    pub cache: PathBuf,
    /// The owned temporary directory, when this harness owns one.
    _directory: Option<TempDir>,
}

impl Harness {
    /// A harness with a usable driver and a fresh cache directory.
    #[must_use]
    pub fn new(tag: &str) -> Self {
        let directory = TempDir::new(tag);
        let path = directory.path().to_path_buf();
        Self::in_dir(&path, true, Some(directory))
    }

    /// A harness over an existing directory, for two builds that share a cache.
    #[must_use]
    pub fn shared(directory: &TempDir) -> Self {
        Self::in_dir(directory.path(), true, None)
    }

    /// A harness whose driver has no guest network.
    #[must_use]
    pub fn without_network(tag: &str) -> Self {
        let directory = TempDir::new(tag);
        let path = directory.path().to_path_buf();
        Self::in_dir(&path, false, Some(directory))
    }

    /// A harness whose driver is not usable.
    #[must_use]
    pub fn unusable(tag: &str) -> Self {
        let mut harness = Self::new(tag);
        harness.driver = Arc::new(SimDriver::unusable(
            DriverId::Firecracker,
            "the test host has no kernel",
        ));
        harness.rebuild();
        harness
    }

    /// A harness over `directory`, with or without a guest network.
    fn in_dir(directory: &Path, network_host: bool, owned: Option<TempDir>) -> Self {
        let (ctx, _clock) = crate::control::support::ctx(0x14);
        let capabilities = Capabilities {
            rootfs_directory: true,
            rootfs_ext4: true,
            rootfs_squashfs: false,
            rootfs_container_image: false,
            root_modes: vec![RootMode::Overlay, RootMode::Writable],
            host_mounts: true,
            network_none: true,
            network_host,
            max_vcpus: 8,
        };
        let driver = Arc::new(
            SimDriver::with_report(HostReport {
                driver: DriverId::Firecracker,
                usable: true,
                checks: Vec::new(),
                capabilities,
            })
            .with_fresh_substrate(),
        );
        let slave = Arc::new(ScriptedSlave::new());
        slave.with_shutdown_exit(driver.substrate());
        let cache = directory.join("cache");
        let config = HostConfig {
            state_dir: directory.join("state"),
            cache_dir: cache.clone(),
            slave_path: Some(PathBuf::from("/bin/agent-slave")),
            firecracker_path: None,
            krun_runner_path: None,
            libkrun_path: None,
            mkfs_ext4_path: Some(PathBuf::from("/bin/mkfs.ext4")),
            driver_order: vec![DriverId::Firecracker],
            artifact_limits: ArtifactLimits::default(),
            image_limits: ImageLimits::default(),
        };
        let host = SandboxHost {
            processes: Arc::new(FakeProcessSpawner::new()),
            fs: Arc::new(NativeFs),
            connector: Arc::new(NoConnector),
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
        let importer = ImageImporter::new(
            ctx.clone(),
            host.clone(),
            store.clone(),
            Arc::clone(&manager),
            None,
        );
        let builder =
            ImageBuilder::new(ctx.clone(), host.clone(), store.clone(), importer, manager);
        Self {
            ctx,
            driver,
            slave,
            store,
            builder,
            host,
            cache,
            _directory: owned,
        }
    }

    /// Rebuilds the builder after the driver changed.
    fn rebuild(&mut self) {
        let manager: Arc<dyn SandboxProvider> = Arc::new(SandboxManager::new(
            self.ctx.clone(),
            vec![Arc::clone(&self.driver) as Arc<dyn SandboxDriver>],
            in_memory_connector(Arc::clone(&self.slave)),
            Arc::new(MemoryRecordStore::new()),
            ManagerConfig::default(),
        ));
        let importer = ImageImporter::new(
            self.ctx.clone(),
            self.host.clone(),
            self.store.clone(),
            Arc::clone(&manager),
            None,
        );
        self.builder = ImageBuilder::new(
            self.ctx.clone(),
            self.host.clone(),
            self.store.clone(),
            importer,
            manager,
        );
    }

    /// Commits one base image with the tag `tag`.
    ///
    /// # Panics
    /// When the commit fails.
    pub fn base(&self, tag: &str) {
        let work = self.cache.join("images/work/base.ext4");
        std::fs::create_dir_all(work.parent().expect("the work directory"))
            .expect("the work directory is made");
        std::fs::write(&work, b"base-rootfs").expect("the base work file is written");
        let manifest = ImageManifest::new(String::new(), GuestArch::host(), String::new(), 0);
        let store = self.store.clone();
        crate::pool::block_on(async move {
            store
                .commit_keyed(work, manifest, Some(tag), "a".repeat(32))
                .await
        })
        .expect("the base image commits");
    }

    /// The build file text with `FROM image:<base>` and `steps`.
    #[must_use]
    pub fn file(base: &str, steps: &str) -> String {
        format!("FROM image:{base}\n{steps}")
    }

    /// Builds one file and returns its result.
    ///
    /// # Panics
    /// When the file does not parse.
    pub fn build(&self, text: &str) -> SandboxResult<ImageRef> {
        self.build_options(text, BuildOptions::default())
    }

    /// Builds one file with `options`.
    ///
    /// # Panics
    /// When the file does not parse.
    pub fn build_options(&self, text: &str, options: BuildOptions) -> SandboxResult<ImageRef> {
        let file = Sandboxfile::parse(text).expect("the test build file parses");
        crate::pool::block_on(
            self.builder
                .build(&file, self.cache.parent().unwrap(), options),
        )
    }

    /// Builds one file and collects its progress.
    ///
    /// # Panics
    /// When the file does not parse.
    pub fn build_with_progress(&self, text: &str) -> (SandboxResult<ImageRef>, Vec<BuildProgress>) {
        let file = Sandboxfile::parse(text).expect("the test build file parses");
        let mut stream = self.builder.build_with_progress(
            file,
            self.cache.parent().unwrap().to_path_buf(),
            BuildOptions::default(),
        );
        let mut progress = Vec::new();
        let result = crate::pool::block_on(async {
            while let Some(event) = stream.next().await {
                match event {
                    BuildEvent::Progress(value) => progress.push(value),
                    BuildEvent::Done(result) => return *result,
                }
            }
            Err(foundation_sandbox::shared::error::SandboxError::StreamLost.into())
        });
        (result, progress)
    }
}

/// The key text of one image reference, for a comparison.
#[must_use]
pub fn key_of(image: &ImageRef) -> String {
    match image {
        ImageRef::Key(key) | ImageRef::Tag(key) => key.clone(),
    }
}

/// Writes one file below `root`, with its parent directories.
pub fn write_file(root: &Path, relative: &str, data: &[u8]) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("the parent directory"))
        .expect("the parent directory is made");
    std::fs::write(path, data).expect("the test file is written");
}

/// The tag of the base image of the tests.
#[must_use]
pub fn base_name() -> String {
    "base".to_string()
}

/// The text of a build file with one step.
#[must_use]
pub fn one_step(step: &str) -> String {
    format!("FROM image:{}\n{step}\n", base_name())
}
