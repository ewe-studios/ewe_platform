//! The build executor (feature 14, section 6.6).
//!
//! It runs the steps in file order. For each changing step it copies the
//! snapshot of the step before to a work image, boots one `Writable` sandbox on
//! that image, applies the step, stops the sandbox cleanly and moves the work
//! image to `<key>.ext4`. A failure stops the build at that step, kills its
//! sandbox and removes its work image; the snapshots of the steps that passed
//! stay, because they are the cache.
//!
//! The image key is `H(last step key, canonical metadata)`. The commit uses
//! that key, so an unchanged build gives the same image and starts no sandbox
//! (invariant I4).

use std::collections::HashSet;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context as TaskContext, Poll};

use foundation_context::Ctx;
use foundation_errstacks::ErrorTrace;
use futures::{Stream, StreamExt};

use crate::native::images::build::cache::{SnapshotName, StepCache, StepRecord};
use crate::native::images::build::keys::{self, image_key};
use crate::native::images::build::steps::{self, plan_add, plan_fetch, AddPlan, DownloadContext};
use crate::native::images::build::{BuildMetadata, BuildOptions};
use crate::native::images::import::{
    ImageImporter, ImageSource, ImportEvent, ImportOptions, ImportReport,
};
use crate::native::images::sandboxfile::{BaseRef, Sandboxfile, StepKind};
use crate::native::images::ImageStore;
use crate::shared::command::{Command, ExecEvent, StdinMode};
use crate::shared::control::{SandboxHandle, SandboxProvider};
use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::host::SandboxHost;
use crate::shared::ids::DriverId;
use crate::shared::images::{ImageManifest, IMAGE_SCHEMA};
use crate::shared::spec::{
    GuestArch, ImageRef, KernelSource, NetworkMode, RootFs, RootMode, SandboxSpec,
};
use crate::shared::unpack::UnpackOptions;
use crate::shared::BoxByteStream;

/// The line of a step, as an attachment on an error that has no line field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildStepLine(pub u32);

impl std::fmt::Display for BuildStepLine {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "line {}", self.0)
    }
}

/// One progress value of a build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildProgress {
    /// A step started. `text` is its canonical line.
    StepStarted {
        /// The step number, starting at 1.
        index: u32,
        /// The number of steps.
        total: u32,
        /// The build file line.
        line: u32,
        /// The canonical text of the step.
        text: String,
    },
    /// The step is a cache hit. It started no sandbox.
    CacheHit {
        /// The step number.
        index: u32,
    },
    /// One chunk of a step's output.
    Output {
        /// The step number.
        index: u32,
        /// Whether the chunk came from standard error.
        stderr: bool,
        /// The chunk.
        data: Vec<u8>,
    },
    /// The step ended.
    StepDone {
        /// The step number.
        index: u32,
        /// The time that the step took.
        millis: u64,
    },
    /// The builder commits the final image.
    Committing,
}

/// One item of a build stream.
#[derive(Debug)]
pub enum BuildEvent {
    /// One progress value.
    Progress(BuildProgress),
    /// The build ended with an image reference or an error.
    Done(Box<SandboxResult<ImageRef>>),
}

/// The progress stream of one build.
///
/// It ends after one [`BuildEvent::Done`]. The build runs on the valtron pool
/// to its end, so dropping the stream stops the reports but leaves the steps
/// and their snapshots in a consistent state.
pub struct BuildStream {
    events: futures::channel::mpsc::UnboundedReceiver<BuildEvent>,
}

impl std::fmt::Debug for BuildStream {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BuildStream")
            .finish_non_exhaustive()
    }
}

impl BuildStream {
    /// The next event, `None` after the stream ends.
    pub async fn next(&mut self) -> Option<BuildEvent> {
        StreamExt::next(self).await
    }
}

impl Stream for BuildStream {
    type Item = BuildEvent;

    fn poll_next(self: Pin<&mut Self>, context: &mut TaskContext<'_>) -> Poll<Option<Self::Item>> {
        self.get_mut().events.poll_next_unpin(context)
    }
}

/// The `Sandboxfile` builder of feature 14.
#[derive(Clone)]
pub struct ImageBuilder {
    ctx: Ctx,
    host: SandboxHost,
    store: ImageStore,
    importer: ImageImporter,
    control: Arc<dyn SandboxProvider>,
}

impl std::fmt::Debug for ImageBuilder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ImageBuilder")
            .field("store", &self.store)
            .finish_non_exhaustive()
    }
}

impl ImageBuilder {
    /// The builder over the store, the importer and the control plane.
    #[must_use]
    pub fn new(
        ctx: Ctx,
        host: SandboxHost,
        store: ImageStore,
        importer: ImageImporter,
        control: Arc<dyn SandboxProvider>,
    ) -> Self {
        Self {
            ctx,
            host,
            store,
            importer,
            control,
        }
    }

    /// The image store behind this builder.
    #[must_use]
    pub fn store(&self) -> &ImageStore {
        &self.store
    }

    /// Builds one image and returns its reference.
    ///
    /// # Errors
    /// The typed error of the failed step (feature 14, section 6.6).
    pub async fn build(
        &self,
        file: &Sandboxfile,
        context: &std::path::Path,
        options: BuildOptions,
    ) -> SandboxResult<ImageRef> {
        let mut stream = self.build_with_progress(file.clone(), context.to_path_buf(), options);
        while let Some(event) = stream.next().await {
            if let BuildEvent::Done(result) = event {
                return *result;
            }
        }
        Err(SandboxError::StreamLost.into())
    }

    /// Builds one image and reports its progress.
    #[must_use]
    pub fn build_with_progress(
        &self,
        file: Sandboxfile,
        context: PathBuf,
        options: BuildOptions,
    ) -> BuildStream {
        let (sender, events) = futures::channel::mpsc::unbounded();
        let on_failure = sender.clone();
        let builder = self.clone();
        let future = async move {
            let sink = ProgressSink(Some(sender.clone()));
            let result = builder.build_inner(file, context, options, sink).await;
            let _ = sender.unbounded_send(BuildEvent::Done(Box::new(result)));
        };
        if let Err(error) =
            foundation_core::valtron::send(foundation_core::valtron::from_future(future))
        {
            let error: ErrorTrace<SandboxError> = SandboxError::Io {
                op: "build".to_string(),
                message: format!("the valtron pool is not available: {error}"),
            }
            .into();
            let _ = on_failure.unbounded_send(BuildEvent::Done(Box::new(Err(error))));
        }
        BuildStream { events }
    }

    /// The build story of section 6.6.
    async fn build_inner(
        &self,
        file: Sandboxfile,
        context: PathBuf,
        options: BuildOptions,
        progress: ProgressSink,
    ) -> SandboxResult<ImageRef> {
        let total = u32::try_from(file.steps.len()).unwrap_or(u32::MAX);
        let driver = self.choose_driver(&file).await?;
        let cache = StepCache::new(
            self.ctx.clone(),
            self.host.fs.clone(),
            &self.host.config.cache_dir,
            options.limits.clone(),
        );
        let first = file
            .steps
            .first()
            .ok_or_else(|| SandboxError::InvalidSpec {
                field: "sandboxfile".to_string(),
                reason: "line 1: the build file has no steps".to_string(),
            })?;
        let StepKind::From(base) = &first.kind else {
            return Err(SandboxError::InvalidSpec {
                field: "sandboxfile".to_string(),
                reason: format!("line {}: the first instruction must be FROM", first.line),
            }
            .into());
        };

        // Step 1: FROM. The base image is already a complete root file system.
        progress.step_started(1, total, first.line, &file.lines()[0]);
        let started = self.started();
        let (_base_path, base_manifest, base_cached) = self.resolve_base(base, &options).await?;
        let metadata = metadata_from_manifest(&base_manifest);
        let previous_key = keys::base_key(&base_manifest.key, GuestArch::host(), options.size_mib);
        let snapshot = SnapshotName::Image(base_manifest.key.clone());
        let mut protected = HashSet::new();
        protected.insert(previous_key.clone());
        if base_cached {
            progress.cache_hit(1);
        }
        progress.step_done(1, self.elapsed(started));

        let mut state = BuildState {
            metadata,
            previous_key,
            snapshot,
            protected,
        };
        // Steps 2 and later: metadata or a changing step.
        for (position, step) in file.steps.iter().enumerate().skip(1) {
            self.run_step(
                &file, position, step, &context, &options, &cache, &driver, &progress, &mut state,
            )
            .await?;
        }
        self.commit(&file, &options, &cache, &progress, &mut state)
            .await
    }

    /// Runs one step after `FROM`.
    #[allow(
        clippy::too_many_arguments,
        reason = "one call carries the step, its state and its progress"
    )]
    async fn run_step(
        &self,
        file: &Sandboxfile,
        position: usize,
        step: &crate::native::images::sandboxfile::Step,
        context: &std::path::Path,
        options: &BuildOptions,
        cache: &StepCache,
        driver: &DriverId,
        progress: &ProgressSink,
        state: &mut BuildState,
    ) -> SandboxResult<()> {
        let total = u32::try_from(file.steps.len()).unwrap_or(u32::MAX);
        let index = u32::try_from(position + 1).unwrap_or(u32::MAX);
        let text = file.lines().get(position).cloned().unwrap_or_default();
        progress.step_started(index, total, step.line, &text);
        let started = self.started();
        let canonical = text.trim_end();
        let (key, plan) = match &step.kind {
            StepKind::Add {
                source,
                destination,
                mode,
            } => {
                let plan = plan_add(
                    &self.host.fs,
                    context,
                    source,
                    destination,
                    *mode,
                    &options.limits,
                )
                .map_err(|error| with_line(error, step.line))?;
                let key = keys::step_key(&state.previous_key, canonical, Some(&plan.tree_hash));
                (key, Some(Action::Add(plan)))
            }
            StepKind::Fetch {
                url,
                sha256,
                destination,
            } => {
                let key = keys::step_key(&state.previous_key, canonical, None);
                let plan = Action::Fetch {
                    url: url.clone(),
                    sha256: sha256.clone(),
                    destination: destination.clone(),
                };
                (key, Some(plan))
            }
            StepKind::Run { .. } => {
                let key = keys::step_key(&state.previous_key, canonical, None);
                (key, Some(Action::Run))
            }
            _ => (keys::step_key(&state.previous_key, canonical, None), None),
        };

        if !options.no_cache {
            if let Some(record) = cache.hit(&key, |image| {
                self.store.has(&ImageRef::Key(image.to_string()))
            }) {
                state.metadata = record.metadata;
                state.snapshot = record.snapshot;
                state.previous_key = key;
                state.protected.insert(state.previous_key.clone());
                cache.touch(&state.previous_key);
                progress.cache_hit(index);
                progress.step_done(index, self.elapsed(started));
                return Ok(());
            }
        }

        match plan {
            None => {
                apply_metadata(&mut state.metadata, &step.kind)?;
                let record = StepRecord {
                    step: record_text(&text),
                    metadata: state.metadata.clone(),
                    snapshot: state.snapshot.clone(),
                };
                cache.save_record(&key, &record)?;
            }
            Some(action) => {
                let snapshot = self
                    .run_changing_step(
                        cache,
                        driver,
                        &step.kind,
                        &state.metadata,
                        &state.snapshot,
                        &key,
                        action,
                        step.line,
                        index,
                        options,
                        progress,
                    )
                    .await
                    .map_err(|error| with_line(error, step.line))?;
                state.snapshot = snapshot;
                let record = StepRecord {
                    step: record_text(&text),
                    metadata: state.metadata.clone(),
                    snapshot: state.snapshot.clone(),
                };
                cache.save_record(&key, &record)?;
            }
        }
        state.previous_key = key;
        state.protected.insert(state.previous_key.clone());
        cache.touch(&state.previous_key);
        progress.step_done(index, self.elapsed(started));
        Ok(())
    }

    /// Commits the final image under the key of the last step and metadata.
    async fn commit(
        &self,
        file: &Sandboxfile,
        options: &BuildOptions,
        cache: &StepCache,
        progress: &ProgressSink,
        state: &mut BuildState,
    ) -> SandboxResult<ImageRef> {
        progress.committing();
        let final_key = image_key(&state.previous_key, &state.metadata.canonical_json());
        let last_snapshot = self.snapshot_path(&state.snapshot).await?;
        let work = self.store.snapshot(&last_snapshot).await?;
        let manifest = final_manifest(&final_key, &state.metadata, file);
        let image = match self
            .store
            .commit_keyed(work.clone(), manifest, options.tag.as_deref(), final_key)
            .await
        {
            Ok(image) => image,
            Err(error) => {
                let _ = self.host.fs.remove_file(&work);
                return Err(error);
            }
        };
        state.protected.insert(state.previous_key.clone());
        cache.evict(&state.protected);
        Ok(image)
    }

    /// The driver that can run every step of `file`.
    async fn choose_driver(&self, file: &Sandboxfile) -> SandboxResult<DriverId> {
        let reports = self.control.probe().await?;
        let network_line = file.steps.iter().find_map(|step| match &step.kind {
            StepKind::Run {
                network: NetworkMode::Host,
                ..
            } => Some(step.line),
            _ => None,
        });
        let mut writable = false;
        for report in &reports {
            if !report.usable || !has_writable(&report.capabilities) {
                continue;
            }
            writable = true;
            if network_line.is_some() && !report.capabilities.network_host {
                continue;
            }
            return Ok(report.driver);
        }
        if let Some(line) = network_line {
            if writable {
                return Err(
                    ErrorTrace::new(SandboxError::UnsupportedNetwork).attach(BuildStepLine(line))
                );
            }
        }
        Err(SandboxError::NoDriver { reports }.into())
    }

    /// Resolves the base of the `FROM` step, and whether it was cached.
    async fn resolve_base(
        &self,
        base: &BaseRef,
        options: &BuildOptions,
    ) -> SandboxResult<(PathBuf, ImageManifest, bool)> {
        match base {
            BaseRef::Image(reference) => {
                let (path, manifest) = self.store.resolve(reference).await?;
                Ok((path, manifest, true))
            }
            BaseRef::Alpine(version) => {
                let report = self
                    .import_base(ImageSource::Alpine(version.clone()), options)
                    .await?;
                let (path, manifest) = self.store.resolve(&report.image).await?;
                Ok((path, manifest, report.cached))
            }
            BaseRef::Docker(name) => {
                let report = self
                    .import_base(ImageSource::Docker(name.clone()), options)
                    .await?;
                let (path, manifest) = self.store.resolve(&report.image).await?;
                Ok((path, manifest, report.cached))
            }
            BaseRef::Tar(path) => {
                let report = self
                    .import_base(ImageSource::Tar(path.clone()), options)
                    .await?;
                let (path, manifest) = self.store.resolve(&report.image).await?;
                Ok((path, manifest, report.cached))
            }
        }
    }

    /// Imports one base image.
    async fn import_base(
        &self,
        source: ImageSource,
        options: &BuildOptions,
    ) -> SandboxResult<ImportReport> {
        let mut stream = self.importer.import_with_progress(
            source,
            ImportOptions {
                tag: None,
                size_mib: options.size_mib,
                cmd: None,
                env: Vec::new(),
                pull: true,
            },
        );
        while let Some(event) = stream.next().await {
            match event {
                ImportEvent::Progress(_) => {}
                ImportEvent::Done(result) => return *result,
            }
        }
        Err(SandboxError::StreamLost.into())
    }

    /// Runs one changing step and returns the snapshot after it.
    ///
    /// # Errors
    /// The error of the step. The sandbox is killed and the work image is
    /// removed on every failure.
    #[allow(
        clippy::too_many_arguments,
        reason = "one call carries the step, its state and its progress"
    )]
    async fn run_changing_step(
        &self,
        cache: &StepCache,
        driver: &DriverId,
        kind: &StepKind,
        metadata: &BuildMetadata,
        snapshot: &SnapshotName,
        key: &str,
        action: Action,
        line: u32,
        index: u32,
        options: &BuildOptions,
        progress: &ProgressSink,
    ) -> SandboxResult<SnapshotName> {
        let previous = self.snapshot_path(snapshot).await?;
        let work = self.store.snapshot(&previous).await?;
        let network = match kind {
            StepKind::Run { network, .. } => *network,
            _ => NetworkMode::None,
        };
        let mut spec = SandboxSpec::new(RootFs::Ext4Image(work.clone()));
        spec.driver = Some(*driver);
        spec.root_mode = RootMode::Writable;
        spec.vcpus = options.vcpus;
        spec.memory_mib = options.memory_mib;
        spec.network = network;
        spec.env = metadata.env.clone();
        spec.workdir = metadata.workdir.clone().unwrap_or_else(|| "/".to_string());
        spec.kernel = metadata
            .kernel
            .as_ref()
            .map(|name| KernelSource::Named(name.clone()));
        let sandbox = match self.control.create(spec).await {
            Ok(sandbox) => sandbox,
            Err(error) => {
                let _ = self.host.fs.remove_file(&work);
                return Err(error);
            }
        };

        let applied = self
            .apply_action(
                &sandbox, kind, action, metadata, line, index, options, progress,
            )
            .await;
        if let Err(error) = applied {
            let _ = sandbox.kill().await;
            let _ = self.host.fs.remove_file(&work);
            return Err(error);
        }
        let report = match sandbox.stop().await {
            Ok(report) => report,
            Err(error) => {
                let _ = sandbox.kill().await;
                let _ = self.host.fs.remove_file(&work);
                return Err(error);
            }
        };
        if report.killed {
            let _ = self.host.fs.remove_file(&work);
            return Err(SandboxError::Io {
                op: "unmount image".to_string(),
                message: "the sandbox was killed; the image may not be clean".to_string(),
            }
            .into());
        }
        cache.store_snapshot(key, &work)?;
        Ok(SnapshotName::Step(key.to_string()))
    }

    /// Applies the action of one changing step in its sandbox.
    #[allow(
        clippy::too_many_arguments,
        reason = "one call carries the step, its sandbox and its progress"
    )]
    async fn apply_action(
        &self,
        sandbox: &SandboxHandle,
        kind: &StepKind,
        action: Action,
        metadata: &BuildMetadata,
        line: u32,
        index: u32,
        options: &BuildOptions,
        progress: &ProgressSink,
    ) -> SandboxResult<()> {
        match action {
            Action::Add(plan) => self.unpack(sandbox, plan.tar).await,
            Action::Fetch {
                url,
                sha256,
                destination,
            } => {
                let context = DownloadContext {
                    ctx: &self.ctx,
                    fs: &self.host.fs,
                    limits: &options.limits,
                };
                let path =
                    steps::download(&context, &self.host.config.cache_dir, &url, &sha256).await?;
                let size = self.host.fs.file_size(&path)?;
                let plan = plan_fetch(&path, &destination, size);
                self.unpack(sandbox, plan).await
            }
            Action::Run => {
                let StepKind::Run { shell, script, .. } = kind else {
                    return Err(SandboxError::InvalidSpec {
                        field: "sandboxfile".to_string(),
                        reason: "the RUN action needs a RUN step".to_string(),
                    }
                    .into());
                };
                let mut command = Command::new(shell.clone());
                command.env = metadata.env.clone();
                command.cwd = Some(metadata.workdir.clone().unwrap_or_else(|| "/".to_string()));
                command.stdin = StdinMode::Piped;
                command.timeout = Some(options.limits.step_timeout);
                let mut handle = sandbox.exec(command).await?;
                let mut stdin = handle.stdin().ok_or(SandboxError::StreamLost)?;
                steps::write_script(
                    &mut stdin,
                    script,
                    steps::RunLimits {
                        chunk_bytes: self.host.config.artifact_limits.chunk_bytes,
                    },
                )
                .await?;
                stdin.close().await?;
                let mut events = handle.events();
                let mut exit = None;
                while let Some(item) = events.next().await {
                    match item? {
                        ExecEvent::Stdout(data) => {
                            progress.output(index, false, data);
                        }
                        ExecEvent::Stderr(data) => {
                            progress.output(index, true, data);
                        }
                        ExecEvent::Exit(status) => {
                            exit = Some(status);
                            break;
                        }
                    }
                }
                let status = match exit {
                    Some(status) => status,
                    None if handle.timed_out() => {
                        return Err(SandboxError::BuildStepFailed {
                            line,
                            exit: None,
                            signal: Some(9),
                        }
                        .into())
                    }
                    None => return Err(SandboxError::StreamLost.into()),
                };
                if status.exit_code() == Some(0) {
                    return Ok(());
                }
                Err(SandboxError::BuildStepFailed {
                    line,
                    exit: status.exit_code(),
                    signal: status.terminating_signal(),
                }
                .into())
            }
        }
    }

    /// Sends one tar plan to the sandbox root.
    async fn unpack(&self, sandbox: &SandboxHandle, plan: steps::TarPlan) -> SandboxResult<()> {
        let stream: BoxByteStream = plan.tar_stream(self.host.fs.clone());
        sandbox
            .unpack("/", stream, UnpackOptions::default())
            .await?;
        Ok(())
    }

    /// The host path of one snapshot name.
    async fn snapshot_path(&self, snapshot: &SnapshotName) -> SandboxResult<PathBuf> {
        match snapshot {
            SnapshotName::Image(key) => {
                let (path, _) = self.store.resolve(&ImageRef::Key(key.clone())).await?;
                Ok(path)
            }
            SnapshotName::Step(key) => {
                let path = self
                    .host
                    .config
                    .cache_dir
                    .join("images")
                    .join("steps")
                    .join(format!("{key}.ext4"));
                if self.host.fs.exists(&path) {
                    Ok(path)
                } else {
                    Err(SandboxError::NotFound {
                        what: format!("the snapshot of step {key}"),
                    }
                    .into())
                }
            }
        }
    }

    /// The monotonic time now, for one step's duration.
    fn started(&self) -> foundation_compact::Instant {
        self.ctx.clock().now()
    }

    /// The milliseconds since `started`.
    fn elapsed(&self, started: foundation_compact::Instant) -> u64 {
        let elapsed = self.ctx.clock().now().saturating_duration_since(started);
        u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
    }
}

/// The state that the steps of one build carry.
struct BuildState {
    /// The metadata so far.
    metadata: BuildMetadata,
    /// The key of the step before.
    previous_key: String,
    /// The snapshot of the step before.
    snapshot: SnapshotName,
    /// The keys that this build named. An eviction never removes one.
    protected: HashSet<String>,
}

/// The action that one changing step needs.
enum Action {
    /// Copy a tree from the build context.
    Add(AddPlan),
    /// Download and copy one file.
    Fetch {
        url: String,
        sha256: String,
        destination: String,
    },
    /// Run one script.
    Run,
}

/// The progress sink of one build.
#[derive(Clone, Default)]
struct ProgressSink(Option<futures::channel::mpsc::UnboundedSender<BuildEvent>>);

impl ProgressSink {
    fn send(&self, progress: BuildProgress) {
        if let Some(sender) = &self.0 {
            let _ = sender.unbounded_send(BuildEvent::Progress(progress));
        }
    }

    fn step_started(&self, index: u32, total: u32, line: u32, text: &str) {
        self.send(BuildProgress::StepStarted {
            index,
            total,
            line,
            text: text.lines().next().unwrap_or("").trim_end().to_string(),
        });
    }

    fn cache_hit(&self, index: u32) {
        self.send(BuildProgress::CacheHit { index });
    }

    fn output(&self, index: u32, stderr: bool, data: Vec<u8>) {
        self.send(BuildProgress::Output {
            index,
            stderr,
            data,
        });
    }

    fn step_done(&self, index: u32, millis: u64) {
        self.send(BuildProgress::StepDone { index, millis });
    }

    fn committing(&self) {
        self.send(BuildProgress::Committing);
    }
}

/// Whether one host has a writable ext4 root.
fn has_writable(caps: &crate::shared::driver::Capabilities) -> bool {
    caps.rootfs_ext4 && caps.root_modes.contains(&RootMode::Writable)
}

/// The metadata that a base image gives the build.
fn metadata_from_manifest(manifest: &ImageManifest) -> BuildMetadata {
    BuildMetadata {
        env: manifest.env.clone(),
        workdir: manifest.workdir.clone(),
        user: manifest.user.clone(),
        cmd: manifest.cmd.clone(),
        kernel: manifest.kernel.clone(),
    }
}

/// Applies one metadata-only step.
fn apply_metadata(metadata: &mut BuildMetadata, kind: &StepKind) -> SandboxResult<()> {
    match kind {
        StepKind::Env { key, value } => {
            if let Some(entry) = metadata.env.iter_mut().find(|entry| &entry.0 == key) {
                entry.1.clone_from(value);
            } else {
                metadata.env.push((key.clone(), value.clone()));
            }
        }
        StepKind::Workdir(path) => metadata.workdir = Some(path.clone()),
        StepKind::User(user) => metadata.user = Some(user.clone()),
        StepKind::Cmd(argv) => metadata.cmd = Some(argv.clone()),
        StepKind::Kernel(name) => metadata.kernel = Some(name.clone()),
        _ => {
            return Err(SandboxError::InvalidSpec {
                field: "sandboxfile".to_string(),
                reason: "the step changes the image".to_string(),
            }
            .into())
        }
    }
    Ok(())
}

/// The manifest of the final image.
fn final_manifest(key: &str, metadata: &BuildMetadata, file: &Sandboxfile) -> ImageManifest {
    ImageManifest {
        schema: IMAGE_SCHEMA,
        key: key.to_string(),
        arch: GuestArch::host(),
        created_unix_ms: 0,
        cmd: metadata.cmd.clone(),
        env: metadata.env.clone(),
        workdir: metadata.workdir.clone(),
        user: metadata.user.clone(),
        kernel: metadata.kernel.clone(),
        rootfs_sha256: String::new(),
        rootfs_size: 0,
        source: format!("sandboxfile:{key}"),
        steps: file.lines().iter().map(|line| record_text(line)).collect(),
    }
}

/// The one-line text of a canonical step.
fn record_text(text: &str) -> String {
    text.trim_end().to_string()
}

/// Adds the step line to an error that has no line field.
fn with_line(error: ErrorTrace<SandboxError>, line: u32) -> ErrorTrace<SandboxError> {
    let context = error.current_context().clone();
    if let SandboxError::InvalidSpec { field, reason } = context {
        if field == "sandboxfile" && !reason.starts_with("line ") {
            return error.change_context(SandboxError::InvalidSpec {
                field,
                reason: format!("line {line}: {reason}"),
            });
        }
    }
    error.attach(BuildStepLine(line))
}
