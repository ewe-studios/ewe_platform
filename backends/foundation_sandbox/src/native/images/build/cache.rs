//! The step cache of a `Sandboxfile` build (feature 14, section 6.2).
//!
//! A step is a cache hit when its `<key>.json` exists and the snapshot that the
//! record names exists. The snapshot of a changing step is
//! `<cache_dir>/images/steps/<key>.ext4`; a metadata-only step names the
//! snapshot of the step before it.
//!
//! A snapshot is written to `<key>.tmp-<random>` and renamed into place, so a
//! reader never sees a half-written file. When the cache passes
//! [`BuildLimits::cache_max_bytes`], the builder removes snapshots by oldest
//! use, and never one that the running build named.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use foundation_compact::rng::Rng as _;
use foundation_context::Ctx;
use serde::{Deserialize, Serialize};

use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::host::HostFs;
use crate::shared::limits::BuildLimits;

use super::BuildMetadata;

/// The metadata and snapshot that one stored step key names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepRecord {
    /// The canonical text of the step.
    pub step: String,
    /// The metadata after the step.
    pub metadata: BuildMetadata,
    /// The snapshot after the step.
    pub snapshot: SnapshotName,
}

/// The snapshot that a step record names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapshotName {
    /// The rootfs of an image in the store, by image key.
    Image(String),
    /// The snapshot of a step, by step key.
    Step(String),
}

/// The step cache over one cache directory.
#[derive(Clone)]
pub struct StepCache {
    ctx: Ctx,
    fs: Arc<dyn HostFs>,
    steps_dir: PathBuf,
    fetch_dir: PathBuf,
    limits: BuildLimits,
}

impl std::fmt::Debug for StepCache {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StepCache")
            .field("steps_dir", &self.steps_dir)
            .field("fetch_dir", &self.fetch_dir)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

impl StepCache {
    /// The cache below `cache_dir`.
    #[must_use]
    pub fn new(ctx: Ctx, fs: Arc<dyn HostFs>, cache_dir: &Path, limits: BuildLimits) -> Self {
        let steps_dir = cache_dir.join("images").join("steps");
        let fetch_dir = cache_dir.join("fetch");
        let _ = fs.create_dir_all(&steps_dir, 0o755);
        let _ = fs.create_dir_all(&fetch_dir, 0o755);
        Self {
            ctx,
            fs,
            steps_dir,
            fetch_dir,
            limits,
        }
    }

    /// The limits of this cache.
    #[must_use]
    pub fn limits(&self) -> &BuildLimits {
        &self.limits
    }

    /// The directory of the step snapshots.
    #[must_use]
    pub fn steps_dir(&self) -> &Path {
        &self.steps_dir
    }

    /// The directory of the verified downloads.
    #[must_use]
    pub fn fetch_dir(&self) -> &Path {
        &self.fetch_dir
    }

    /// The snapshot file of one step key.
    #[must_use]
    pub fn snapshot_path(&self, key: &str) -> PathBuf {
        self.steps_dir.join(format!("{key}.ext4"))
    }

    /// The record file of one step key.
    #[must_use]
    pub fn record_path(&self, key: &str) -> PathBuf {
        self.steps_dir.join(format!("{key}.json"))
    }

    /// The access-time file of one step key.
    fn used_path(&self, key: &str) -> PathBuf {
        self.steps_dir.join(format!("{key}.used"))
    }

    /// Reads the record of `key`, when it is there and readable.
    #[must_use]
    pub fn load(&self, key: &str) -> Option<StepRecord> {
        let body = self.fs.read(&self.record_path(key), 1024 * 1024).ok()?;
        serde_json::from_slice(&body).ok()
    }

    /// The record of a cache hit: its `.json` and its snapshot exist.
    ///
    /// `image_exists` says whether the store holds the image of a
    /// [`SnapshotName::Image`].
    #[must_use]
    pub fn hit(&self, key: &str, image_exists: impl Fn(&str) -> bool) -> Option<StepRecord> {
        let record = self.load(key)?;
        let exists = match &record.snapshot {
            SnapshotName::Image(image) => image_exists(image),
            SnapshotName::Step(step) => self.fs.exists(&self.snapshot_path(step)),
        };
        exists.then_some(record)
    }

    /// Writes the record of one step.
    ///
    /// # Errors
    /// [`SandboxError::Io`] when the record cannot be written.
    pub fn save_record(&self, key: &str, record: &StepRecord) -> SandboxResult<()> {
        let json = serde_json::to_vec_pretty(record).map_err(|error| SandboxError::Io {
            op: "encode a step record".to_string(),
            message: error.to_string(),
        })?;
        let temporary = self
            .steps_dir
            .join(format!("{key}.json.tmp-{}", self.random_name()?));
        self.fs.write(&temporary, &json)?;
        self.fs.rename(&temporary, &self.record_path(key))
    }

    /// Moves one work image to `<key>.ext4` through a temporary name.
    ///
    /// A snapshot that already exists keeps the first file (invariant I11).
    ///
    /// # Errors
    /// [`SandboxError::Io`] when the file cannot be renamed.
    pub fn store_snapshot(&self, key: &str, work: &Path) -> SandboxResult<()> {
        let target = self.snapshot_path(key);
        if self.fs.exists(&target) {
            let _ = self.fs.remove_file(work);
            return Ok(());
        }
        let temporary = self
            .steps_dir
            .join(format!("{key}.tmp-{}", self.random_name()?));
        self.fs.rename(work, &temporary)?;
        match self.fs.rename(&temporary, &target) {
            Ok(()) => Ok(()),
            // Another builder stored the same key first. Its file is complete.
            Err(_) if self.fs.exists(&target) => {
                let _ = self.fs.remove_file(&temporary);
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    /// Records that the running build used one step key.
    ///
    /// The content is the wall-clock time as Unix milliseconds, so a later
    /// eviction orders the keys by use without a second metadata call.
    pub fn touch(&self, key: &str) {
        let millis = self
            .ctx
            .clock()
            .wall_now()
            .duration_since(foundation_compact::SystemTime::UNIX_EPOCH)
            .map_or(0, |duration| {
                u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
            });
        let _ = self
            .fs
            .write(&self.used_path(key), millis.to_string().as_bytes());
    }

    /// Removes the oldest snapshots until the cache fits `cache_max_bytes`.
    ///
    /// A key in `protected` is never removed: the running build named it.
    pub fn evict(&self, protected: &HashSet<String>) {
        let mut entries: Vec<(String, u64, u64)> = Vec::new();
        let mut total = 0_u64;
        for path in self.fs.read_dir(&self.steps_dir).unwrap_or_default() {
            let Some(name) = path.file_name().and_then(std::ffi::OsStr::to_str) else {
                continue;
            };
            let Some(key) = name.strip_suffix(".ext4") else {
                continue;
            };
            let Ok(size) = self.fs.file_size(&path) else {
                continue;
            };
            total = total.saturating_add(size);
            entries.push((key.to_string(), size, self.used_millis(key)));
        }
        if total <= self.limits.cache_max_bytes {
            return;
        }
        entries.sort_by_key(|entry| entry.2);
        for (key, size, _) in entries {
            if total <= self.limits.cache_max_bytes {
                break;
            }
            if protected.contains(&key) {
                continue;
            }
            if self.fs.remove_file(&self.snapshot_path(&key)).is_ok() {
                total = total.saturating_sub(size);
            }
            let _ = self.fs.remove_file(&self.record_path(&key));
            let _ = self.fs.remove_file(&self.used_path(&key));
        }
    }

    /// The access time of one key, in Unix milliseconds.
    fn used_millis(&self, key: &str) -> u64 {
        self.fs
            .read(&self.used_path(key), 64)
            .ok()
            .and_then(|body| String::from_utf8_lossy(&body).trim().parse().ok())
            .unwrap_or(0)
    }

    /// A fresh random name for one cache file.
    fn random_name(&self) -> SandboxResult<String> {
        let rng = self.ctx.rng();
        let mut rng = rng.lock().map_err(|_| SandboxError::Io {
            op: "name a cache file".to_string(),
            message: "the random number generator lock is poisoned".to_string(),
        })?;
        Ok(format!("{:016x}", rng.next_u64()))
    }
}
