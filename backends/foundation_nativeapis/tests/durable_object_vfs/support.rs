//! Shared test support for the object-store VFS suites.

use foundation_context::Ctx;
use foundation_core::durable::{DurableStream, StreamName};
use foundation_core::valtron::SimulatedClock;
use foundation_nativeapis::native::vfs::r2::{R2Settings, R2Vfs};
use foundation_nativeapis::native::vfs::s3::{S3Settings, S3Vfs};
use foundation_nativeapis::shared::vfs::durable::{
    MemoryObjectStore, ObjectLayout, ObjectVfsConfig,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// A temporary directory that removes itself.
pub struct TempDir(PathBuf);

impl TempDir {
    /// Create a fresh temporary directory.
    pub fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "durable_object_vfs_{name}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after the epoch")
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }

    /// Return the directory path.
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A test VFS and the store it uses.
pub struct Fixture {
    /// The VFS.
    pub vfs: Arc<S3Vfs>,
    /// The object store.
    pub store: Arc<MemoryObjectStore>,
    /// The simulated clock.
    pub clock: Arc<SimulatedClock>,
    /// The context that holds the clock.
    pub ctx: Ctx,
    /// The temporary directory that holds the spool.
    pub temp: TempDir,
    /// The configuration in use.
    pub config: ObjectVfsConfig,
}

/// Build a VFS with the given configuration.
pub fn fixture(name: &str, config: ObjectVfsConfig) -> Fixture {
    let temp = TempDir::new(name);
    let config = config.with_spool_root(temp.path().join("spool"));
    let clock = Arc::new(SimulatedClock::new(core::time::Duration::ZERO));
    let ctx = Ctx::fixed(clock.clone());
    let store = Arc::new(MemoryObjectStore::new());
    let vfs = Arc::new(
        S3Vfs::new(
            store.clone(),
            &ctx,
            config.clone(),
            S3Settings::default().with_path_style(true),
        )
        .expect("vfs"),
    );
    Fixture {
        vfs,
        store,
        clock,
        ctx,
        temp,
        config,
    }
}

/// Build an R2 VFS with the default settings.
pub fn r2_fixture(name: &str) -> (Arc<R2Vfs>, Arc<MemoryObjectStore>, TempDir) {
    let temp = TempDir::new(name);
    let config = ObjectVfsConfig::default().with_spool_root(temp.path().join("spool"));
    let clock = Arc::new(SimulatedClock::new(core::time::Duration::ZERO));
    let ctx = Ctx::fixed(clock);
    let store = Arc::new(MemoryObjectStore::new());
    let vfs =
        Arc::new(R2Vfs::new(store.clone(), &ctx, config, R2Settings::default()).expect("r2 vfs"));
    (vfs, store, temp)
}

/// A testkit factory for fresh S3 stores.
pub fn s3_factory(
    root: PathBuf,
    layout: ObjectLayout,
) -> impl Fn() -> (DurableStream, Arc<SimulatedClock>) {
    let root = Arc::new(root);
    let counter = Arc::new(AtomicUsize::new(0));
    move || {
        let index = counter.fetch_add(1, Ordering::SeqCst);
        let dir = root.join(format!("store-{index}"));
        let clock = Arc::new(SimulatedClock::new(core::time::Duration::ZERO));
        let ctx = Ctx::fixed(clock.clone());
        let config = ObjectVfsConfig::default()
            .with_layout(layout)
            .with_spool_root(dir.join("spool"));
        let store = Arc::new(MemoryObjectStore::new());
        let vfs = S3Vfs::new(store, &ctx, config, S3Settings::default()).expect("s3 vfs");
        (DurableStream::with_ctx(Arc::new(vfs), &ctx), clock)
    }
}

/// A testkit factory for fresh R2 stores.
pub fn r2_factory(root: PathBuf) -> impl Fn() -> (DurableStream, Arc<SimulatedClock>) {
    let root = Arc::new(root);
    let counter = Arc::new(AtomicUsize::new(0));
    move || {
        let index = counter.fetch_add(1, Ordering::SeqCst);
        let dir = root.join(format!("r2-store-{index}"));
        let clock = Arc::new(SimulatedClock::new(core::time::Duration::ZERO));
        let ctx = Ctx::fixed(clock.clone());
        let config = ObjectVfsConfig::default().with_spool_root(dir.join("spool"));
        let store = Arc::new(MemoryObjectStore::new());
        let vfs = R2Vfs::new(store, &ctx, config, R2Settings::default()).expect("r2 vfs");
        (DurableStream::with_ctx(Arc::new(vfs), &ctx), clock)
    }
}

/// Parse a stream name.
pub fn stream(name: &str) -> StreamName {
    StreamName::parse(name).expect("stream name")
}
