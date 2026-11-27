//! The image importer (feature 13): a Docker image, a root file system tar or
//! the Alpine archive becomes a sandbox image.
//!
//! The import has six steps (feature 13, section 6.2): identify the source,
//! check the cache, open the export stream, fill a blank image in a writable
//! sandbox, commit the filled image, and always remove the temporary container.
//!
//! Every body is streamed. The export goes to the guest in bounded chunks, so
//! the host never holds the whole image (invariant I1). The image key comes
//! from the source identity, the guest architecture and the image size, so a
//! second import of the same source finds the image before it exports anything
//! (invariant I3).

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context as TaskContext, Poll};

use foundation_compact::rng::Rng as _;
use foundation_context::Ctx;
use foundation_deployment_docker::client::body::{ByteBody, ProgressStream};
use foundation_deployment_docker::generated::create::CreateImageInfo;
use foundation_deployment_docker::generated::json::{
    ContainerConfig, HostConfig, ImageConfig, Resources,
};
use foundation_errstacks::ErrorTrace;
use futures::channel::mpsc::UnboundedSender;
use futures::future::{select, Either};
use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest as _, Sha256};

use crate::shared::control::{SandboxHandle, SandboxProvider};
use crate::shared::error::{SandboxError, SandboxResult, SlaveErrorCode};
use crate::shared::host::SandboxHost;
use crate::shared::ids::DriverId;
use crate::shared::images::{ImageManifest, IMAGE_SCHEMA};
use crate::shared::limits::ImageLimits;
use crate::shared::spec::{GuestArch, ImageRef, NetworkMode, RootFs, RootMode, SandboxSpec};
use crate::shared::unpack::UnpackOptions;
use crate::shared::BoxByteStream;

use crate::native::artifacts::ArtifactStore;
use crate::native::docker::api::{create_body, is_not_found, map_error, DockerApi, LABEL_IMPORT};
use crate::native::images::registry::{
    digest_hex, layer_kind, LayerKind, OciDescriptor, OciReference, RegistryClient, RegistryOptions,
};
use crate::native::images::ImageStore;

/// The prefix of the text that a source identity is hashed with.
const KEY_PREFIX: &str = "ewe-image-v1";

/// The name of the export entry that a sandbox root does not need.
///
/// `docker export` writes `/.dockerenv`; the slave drops the entry, so no
/// command runs in the guest for it (feature 13, section 6.4).
const SKIP_EXPORT: &str = ".dockerenv";

/// The most bytes of one read chunk.
const CHUNK_BYTES: usize = 64 * 1024;

/// The byte step at which one `Filling` progress value is sent.
const PROGRESS_STEP: u64 = 1024 * 1024;

/// Where an image comes from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageSource {
    /// An image name or identifier in the local Docker daemon.
    ///
    /// The importer pulls it when the daemon does not have it and
    /// [`ImportOptions::pull`] is true.
    Docker(String),
    /// An uncompressed or gzip tar file of a root file system.
    Tar(PathBuf),
    /// A version of feature 03's Alpine manifest, for example `"3.22"`.
    Alpine(String),
    /// A reference to a container image in a registry (feature 15).
    Oci(String),
}

impl ImageSource {
    /// Parses a command-line source: `docker:NAME`, `tar:PATH`,
    /// `alpine:VERSION` or `oci:REFERENCE`.
    ///
    /// # Errors
    /// The reason, as text, when the value has no known prefix or its registry
    /// reference is not valid.
    pub fn parse(value: &str) -> Result<Self, String> {
        if let Some(name) = value.strip_prefix("docker:") {
            if name.is_empty() {
                return Err("the Docker image name is empty".to_string());
            }
            return Ok(Self::Docker(name.to_string()));
        }
        if let Some(path) = value.strip_prefix("tar:") {
            if path.is_empty() {
                return Err("the tar path is empty".to_string());
            }
            return Ok(Self::Tar(PathBuf::from(path)));
        }
        if let Some(version) = value.strip_prefix("alpine:") {
            if version.is_empty() {
                return Err("the Alpine version is empty".to_string());
            }
            return Ok(Self::Alpine(version.to_string()));
        }
        if let Some(reference) = value.strip_prefix("oci:") {
            if reference.is_empty() {
                return Err("the registry reference is empty".to_string());
            }
            OciReference::parse(reference).map_err(|error| error.to_string())?;
            return Ok(Self::Oci(reference.to_string()));
        }
        Err(format!(
            "the image source must be docker:NAME, tar:PATH, alpine:VERSION or oci:REFERENCE: {value}"
        ))
    }
}

/// The options of one import.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportOptions {
    /// The tag that the image gets, when the caller wants one.
    pub tag: Option<String>,
    /// The size of the work image in mebibytes. `0` uses
    /// [`ImageLimits::default_size_mib`].
    pub size_mib: u32,
    /// A start command that replaces the source's command.
    ///
    /// A tar source has no command, so a start command needs this field.
    pub cmd: Option<Vec<String>>,
    /// Environment entries that the source's entries are added to.
    pub env: Vec<(String, String)>,
    /// Whether to pull a Docker image that the daemon does not have.
    pub pull: bool,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            tag: None,
            size_mib: ImageLimits::default().default_size_mib,
            cmd: None,
            env: Vec::new(),
            pull: true,
        }
    }
}

/// One step of an import.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportProgress {
    /// The importer reads the source identity.
    Identifying,
    /// The importer pulls an image, or one layer of it. `done` is the number of
    /// bytes that arrived of this body; `total` is its full size when the
    /// source names it.
    Pulling {
        /// The bytes that arrived.
        done: u64,
        /// The full body size, when the source gives it.
        total: Option<u64>,
    },
    /// The store already has the image.
    CacheHit,
    /// The guest unpacks the export; `bytes` is the number of export bytes
    /// that the importer has sent so far.
    Filling {
        /// The export bytes sent so far.
        bytes: u64,
    },
    /// The importer writes the manifest and commits the image.
    Committing,
}

/// One item of an import stream.
#[derive(Debug)]
pub enum ImportEvent {
    /// One step of the import.
    Progress(ImportProgress),
    /// The import ended.
    Done(Box<SandboxResult<ImportReport>>),
}

/// The result of one import.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportReport {
    /// The image, by tag when the caller gave one and by key otherwise.
    pub image: ImageRef,
    /// The manifest that the store now holds.
    pub manifest: ImageManifest,
    /// The container configuration fields that a sandbox cannot use.
    pub dropped_config: Vec<String>,
    /// Whether the store already had the image.
    pub cached: bool,
}

/// The progress stream of one import.
///
/// It ends after one [`ImportEvent::Done`]. The import itself runs on the
/// valtron pool to its end, so dropping the stream stops the progress reports
/// but never strands the temporary container (invariant I4).
pub struct ImportStream {
    /// The events that the import sends.
    events: futures::channel::mpsc::UnboundedReceiver<ImportEvent>,
}

impl std::fmt::Debug for ImportStream {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ImportStream")
            .finish_non_exhaustive()
    }
}

impl ImportStream {
    /// The next event, `None` after the stream ends.
    pub async fn next(&mut self) -> Option<ImportEvent> {
        StreamExt::next(self).await
    }
}

impl Stream for ImportStream {
    type Item = ImportEvent;

    fn poll_next(self: Pin<&mut Self>, context: &mut TaskContext<'_>) -> Poll<Option<Self::Item>> {
        self.get_mut().events.poll_next_unpin(context)
    }
}

/// The import of feature 13.
///
/// It reads the source, fills a blank image through the control plane and
/// commits the image to the store.
#[derive(Clone)]
pub struct ImageImporter {
    ctx: Ctx,
    host: SandboxHost,
    store: ImageStore,
    control: Arc<dyn SandboxProvider>,
    docker: Option<Arc<dyn DockerApi>>,
    registry: RegistryOptions,
}

impl std::fmt::Debug for ImageImporter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ImageImporter")
            .field("store", &self.store)
            .field("docker", &self.docker.is_some())
            .finish_non_exhaustive()
    }
}

impl ImageImporter {
    /// The importer over `store`, with `control` to start the fill sandbox.
    ///
    /// `docker` is the Engine connection of the Docker source. A host without
    /// Docker gives `None`; the Docker source then fails with
    /// [`SandboxError::DriverUnavailable`].
    #[must_use]
    pub fn new(
        ctx: Ctx,
        host: SandboxHost,
        store: ImageStore,
        control: Arc<dyn SandboxProvider>,
        docker: Option<Arc<dyn DockerApi>>,
    ) -> Self {
        Self {
            ctx,
            host,
            store,
            control,
            docker,
            registry: RegistryOptions::default(),
        }
    }

    /// The importer with the registry credentials and limits of feature 15.
    #[must_use]
    pub fn with_registry(mut self, options: RegistryOptions) -> Self {
        self.registry = options;
        self
    }

    /// The image store behind this importer.
    #[must_use]
    pub fn store(&self) -> &ImageStore {
        &self.store
    }

    /// Imports one source and returns its report.
    ///
    /// The import runs on the valtron pool, so a caller that drops this future
    /// still lets the import finish and remove its temporary container
    /// (invariant I4). The result is then lost.
    ///
    /// # Errors
    /// The typed error of the failed step (feature 13, section 6.2).
    pub async fn import(
        &self,
        source: ImageSource,
        options: ImportOptions,
    ) -> SandboxResult<ImportReport> {
        let mut stream = self.import_with_progress(source, options);
        while let Some(event) = stream.next().await {
            if let ImportEvent::Done(result) = event {
                return *result;
            }
        }
        Err(SandboxError::StreamLost.into())
    }

    /// Imports one source and reports its progress.
    ///
    /// The import runs on the valtron pool. A caller that drops the stream
    /// loses the progress values; the import still finishes and removes its
    /// temporary container.
    #[must_use]
    pub fn import_with_progress(
        &self,
        source: ImageSource,
        options: ImportOptions,
    ) -> ImportStream {
        let (sender, events) = futures::channel::mpsc::unbounded();
        // A clone survives a failed schedule, so the caller still gets a typed
        // error instead of a stream that ends with no result.
        let on_failure = sender.clone();
        let importer = self.clone();
        let future = async move {
            let sink = ProgressSink(Some(sender.clone()));
            let result = importer.import_inner(source, options, sink).await;
            let _ = sender.unbounded_send(ImportEvent::Done(Box::new(result)));
        };
        if let Err(error) =
            foundation_core::valtron::send(foundation_core::valtron::from_future(future))
        {
            let error: ErrorTrace<SandboxError> = SandboxError::Io {
                op: "import".to_string(),
                message: format!("the valtron pool is not available: {error}"),
            }
            .into();
            let _ = on_failure.unbounded_send(ImportEvent::Done(Box::new(Err(error))));
        }
        ImportStream { events }
    }

    /// The import story of section 6.2.
    async fn import_inner(
        &self,
        source: ImageSource,
        options: ImportOptions,
        progress: ProgressSink,
    ) -> SandboxResult<ImportReport> {
        progress.progress(ImportProgress::Identifying);
        let identified = self.identify(source, &options, &progress).await?;

        let size_mib = self.size_mib(&options);
        let key = image_key(&identified.identity, identified.arch, size_mib);

        // Step 2: the cache. A hit sets the tag and starts nothing.
        if let Some(manifest) = self.store.manifest_of(&key).await {
            progress.progress(ImportProgress::CacheHit);
            if let Some(tag) = &options.tag {
                self.store.set_tag(&key, tag).await?;
            }
            return Ok(ImportReport {
                image: reference_for(&key, options.tag.as_deref()),
                manifest,
                dropped_config: identified.dropped,
                cached: true,
            });
        }

        // Step 3: open the export stream. The temporary container comes first,
        // so a failure in a later step still removes it.
        let mut fill = Fill::new();
        let outcome = self
            .run_fill(&identified, &options, size_mib, &key, &mut fill, &progress)
            .await;

        // Step 6: remove the temporary container on every path.
        fill.remove_container().await;
        if outcome.is_err() {
            fill.discard(self.host.fs.as_ref()).await;
        } else if let SourcePlan::Oci(plan) = &identified.plan {
            self.prune_registry_cache(plan);
        }
        outcome
    }

    /// Steps 3 to 5: open the source, fill the work image, commit it.
    async fn run_fill(
        &self,
        identified: &Identified,
        options: &ImportOptions,
        size_mib: u32,
        key: &str,
        fill: &mut Fill,
        progress: &ProgressSink,
    ) -> SandboxResult<ImportReport> {
        if let SourcePlan::Oci(plan) = &identified.plan {
            return self
                .fill_oci(identified, plan, size_mib, key, fill, progress, options)
                .await;
        }
        let stream = self.open_export(identified, fill, progress).await?;
        let sandbox = self.start_fill(size_mib, fill).await?;
        sandbox
            .unpack("/", stream, export_unpack_options())
            .await
            .map_err(|error| map_unpack_error(error, size_mib))?;
        self.stop_and_commit(identified, options, key, fill, progress)
            .await
    }

    /// Steps 3 to 5 for a registry image: fetch each layer, then unpack it in
    /// manifest order (feature 15, section 6.4).
    #[allow(
        clippy::too_many_arguments,
        reason = "one call carries the import state, the plan and the progress"
    )]
    async fn fill_oci(
        &self,
        identified: &Identified,
        plan: &OciPlan,
        size_mib: u32,
        key: &str,
        fill: &mut Fill,
        progress: &ProgressSink,
        options: &ImportOptions,
    ) -> SandboxResult<ImportReport> {
        let sandbox = self.start_fill(size_mib, fill).await?;
        let max = self.host.config.image_limits.max_archive_bytes;
        for (index, (descriptor, kind)) in plan.layers.iter().enumerate() {
            let path = plan
                .client
                .fetch_blob(&plan.reference, descriptor, &mut |done| {
                    progress.progress(ImportProgress::Pulling {
                        done,
                        total: Some(descriptor.size),
                    });
                })
                .await?;
            let reader = self.layer_reader(&path, *kind)?;
            let stream = file_stream(reader, max, progress.clone());
            sandbox
                .unpack("/", stream, oci_unpack_options())
                .await
                .map_err(|error| map_layer_unpack_error(error, index, size_mib))?;
        }
        self.stop_and_commit(identified, options, key, fill, progress)
            .await
    }

    /// Creates the blank work image and starts the fill sandbox on it.
    async fn start_fill(&self, size_mib: u32, fill: &mut Fill) -> SandboxResult<SandboxHandle> {
        let work = self.store.create_blank(size_mib).await?;
        fill.work = Some(work.clone());
        let mut spec = SandboxSpec::new(RootFs::Ext4Image(work));
        spec.root_mode = RootMode::Writable;
        spec.network = NetworkMode::None;
        let sandbox = self.control.create(spec).await?;
        fill.sandbox = Some(sandbox.clone());
        Ok(sandbox)
    }

    /// Stops the fill sandbox and commits the work image.
    async fn stop_and_commit(
        &self,
        identified: &Identified,
        options: &ImportOptions,
        key: &str,
        fill: &mut Fill,
        progress: &ProgressSink,
    ) -> SandboxResult<ImportReport> {
        let Some(sandbox) = fill.sandbox.clone() else {
            return Err(SandboxError::Io {
                op: "fill an image".to_string(),
                message: "the fill sandbox is gone".to_string(),
            }
            .into());
        };
        let report = sandbox.stop().await?;
        fill.sandbox = None;
        if report.killed {
            return Err(SandboxError::Io {
                op: "unmount image".to_string(),
                message: "the sandbox was killed; the image may not be clean".to_string(),
            }
            .into());
        }
        let Some(work) = fill.work.take() else {
            return Err(SandboxError::Io {
                op: "fill an image".to_string(),
                message: "the work image is gone".to_string(),
            }
            .into());
        };

        progress.progress(ImportProgress::Committing);
        let image = self
            .store
            .commit_keyed(
                work,
                identified.manifest.clone(),
                options.tag.as_deref(),
                key.to_string(),
            )
            .await?;
        // The committed manifest has the size and the hash that the store read.
        let manifest = self
            .store
            .manifest_of(key)
            .await
            .ok_or_else(|| SandboxError::NotFound {
                what: format!("image key {key}"),
            })?;
        Ok(ImportReport {
            image,
            manifest,
            dropped_config: identified.dropped.clone(),
            cached: false,
        })
    }

    /// The decompressing reader of one cached layer.
    fn layer_reader(
        &self,
        path: &Path,
        kind: LayerKind,
    ) -> SandboxResult<Box<dyn std::io::Read + Send>> {
        let reader = self
            .host
            .fs
            .read_stream(path)
            .map_err(|error| SandboxError::Io {
                op: "read a cached layer".to_string(),
                message: format!("{}: {error}", path.display()),
            })?;
        Ok(match kind {
            LayerKind::Plain => reader,
            LayerKind::Gzip => Box::new(flate2::read::GzDecoder::new(reader)),
        })
    }

    /// Removes the oldest blobs when the blob cache passes its size limit.
    ///
    /// It never removes a blob of the image that this import just named.
    fn prune_registry_cache(&self, plan: &OciPlan) {
        let limit = self.host.config.image_limits.blob_cache_max_bytes;
        if let Err(error) = plan.client.prune_blob_cache(limit, &plan.keep_hexes()) {
            tracing::warn!(%error, "the registry blob cache cannot be pruned");
        }
    }

    /// The source identity, architecture and manifest of one import.
    async fn identify(
        &self,
        source: ImageSource,
        options: &ImportOptions,
        progress: &ProgressSink,
    ) -> SandboxResult<Identified> {
        match source {
            ImageSource::Docker(name) => {
                let api = self.docker()?;
                let inspect = match api.image_inspect(&name).await {
                    Ok(inspect) => inspect,
                    Err(error) if is_not_found(&error) && options.pull => {
                        self.pull(api, &name, progress).await?;
                        api.image_inspect(&name).await.map_err(map_error)?
                    }
                    Err(error) => return Err(map_error(error).into()),
                };
                let arch = image_arch(&name, inspect.architecture.as_deref())?;
                let image_id = inspect.id.clone().unwrap_or_default();
                let identity = format!("docker:{name}@{image_id}");
                let config = inspect.config.clone().unwrap_or_default();
                let (manifest, dropped) = config_to_manifest(&config, options, arch, &identity);
                Ok(Identified {
                    identity,
                    arch,
                    plan: SourcePlan::Docker { name },
                    manifest,
                    dropped,
                })
            }
            ImageSource::Tar(path) => {
                let hash = self.hash_file(&path)?;
                let identity = format!("tar:sha256:{hash}");
                let arch = GuestArch::host();
                let manifest = options_manifest(options, arch, &identity);
                Ok(Identified {
                    identity,
                    arch,
                    plan: SourcePlan::File { path },
                    manifest,
                    dropped: Vec::new(),
                })
            }
            ImageSource::Alpine(version) => {
                check_alpine_version(&version)?;
                let arch = GuestArch::host();
                let artifacts = ArtifactStore::new(
                    self.ctx.clone(),
                    self.host.fs.clone(),
                    self.host.config.cache_dir.clone(),
                );
                let path = artifacts.alpine_rootfs_archive(arch).await?;
                let hash = self.hash_file(&path)?;
                let identity = format!("alpine:{hash}");
                let manifest = options_manifest(options, arch, &identity);
                Ok(Identified {
                    identity,
                    arch,
                    plan: SourcePlan::File { path },
                    manifest,
                    dropped: Vec::new(),
                })
            }
            ImageSource::Oci(text) => self.identify_oci(&text, options).await,
        }
    }

    /// The source identity, architecture and manifest of one registry image
    /// (feature 15, section 6.4).
    async fn identify_oci(&self, text: &str, options: &ImportOptions) -> SandboxResult<Identified> {
        let reference = OciReference::parse(text)?;
        let client = RegistryClient::new(
            self.ctx.clone(),
            self.host.fs.clone(),
            self.host.config.cache_dir.clone(),
            self.registry.clone(),
        );
        let resolved = client.resolve_manifest(&reference).await?;
        let Some(config_descriptor) = resolved.manifest.config.clone() else {
            return Err(SandboxError::InvalidSpec {
                field: "manifest".to_string(),
                reason: format!("the manifest of {reference} has no configuration"),
            }
            .into());
        };
        let config = client.fetch_config(&reference, &config_descriptor).await?;
        client.check_arch(&config, &reference)?;
        let arch = GuestArch::host();
        let identity = format!(
            "oci:{}/{}@{}",
            reference.registry, reference.repository, resolved.digest
        );
        let inner = config.config.clone().unwrap_or_default();
        let (manifest, dropped) = config_to_manifest(&inner, options, arch, &identity);
        let mut layers = Vec::with_capacity(resolved.manifest.layers.len());
        for layer in &resolved.manifest.layers {
            let kind = layer_kind(&layer.media_type)?;
            layers.push((layer.clone(), kind));
        }
        Ok(Identified {
            identity,
            arch,
            plan: SourcePlan::Oci(Box::new(OciPlan {
                client,
                reference,
                config_digest: config_descriptor.digest,
                layers,
            })),
            manifest,
            dropped,
        })
    }

    /// Pulls one Docker image to its last progress line.
    async fn pull(
        &self,
        api: &Arc<dyn DockerApi>,
        name: &str,
        progress: &ProgressSink,
    ) -> SandboxResult<()> {
        progress.progress(ImportProgress::Pulling {
            done: 0,
            total: None,
        });
        let stream = api.image_pull(name).await.map_err(map_error)?;
        let timeout = self.host.config.image_limits.pull_timeout;
        read_pull(stream, &self.ctx, timeout).await
    }

    /// Step 3: the export stream of one source.
    async fn open_export(
        &self,
        identified: &Identified,
        fill: &mut Fill,
        progress: &ProgressSink,
    ) -> SandboxResult<BoxByteStream> {
        match &identified.plan {
            SourcePlan::Docker { name } => {
                let api = self.docker()?;
                let container = ContainerConfig {
                    image: Some(name.clone()),
                    labels: Some(json!({ LABEL_IMPORT: "1" })),
                    ..ContainerConfig::default()
                };
                let host = HostConfig {
                    auto_remove: Some(false),
                    network_mode: Some("none".to_string()),
                    ..HostConfig::default()
                };
                let body = create_body(container, host, Resources::default());
                let container_name = format!("ewe-import-{}", self.random_name()?);
                let created = api
                    .create_container(&body, Some(&container_name))
                    .await
                    .map_err(map_error)?;
                fill.container = Some((Arc::clone(api), created.id.clone()));
                let export = api.container_export(&created.id).await.map_err(map_error)?;
                Ok(docker_stream(
                    export,
                    self.host.config.image_limits.max_archive_bytes,
                    progress.clone(),
                ))
            }
            SourcePlan::File { path } => {
                let reader = self.file_reader(path)?;
                Ok(file_stream(
                    reader,
                    self.host.config.image_limits.max_archive_bytes,
                    progress.clone(),
                ))
            }
            SourcePlan::Oci(_) => Err(SandboxError::InvalidSpec {
                field: "source".to_string(),
                reason: "a registry image has one stream for each layer".to_string(),
            }
            .into()),
        }
    }

    /// The decompressing reader of one tar file.
    fn file_reader(&self, path: &Path) -> SandboxResult<Box<dyn std::io::Read + Send>> {
        let mut reader = self
            .host
            .fs
            .open_read(path)
            .map_err(|error| SandboxError::Io {
                op: "read the image source".to_string(),
                message: format!("{}: {error}", path.display()),
            })?;
        let mut first = [0_u8; 2];
        let mut read = 0;
        while read < first.len() {
            match reader.read(&mut first[read..]) {
                Ok(0) => break,
                Ok(count) => read += count,
                Err(error) => {
                    return Err(SandboxError::Io {
                        op: "read the image source".to_string(),
                        message: format!("{}: {error}", path.display()),
                    }
                    .into())
                }
            }
        }
        let gzip = read == 2 && first == [0x1f, 0x8b];
        let chained = std::io::Cursor::new(first[..read].to_vec()).chain(reader);
        if gzip {
            Ok(Box::new(flate2::read::GzDecoder::new(chained)))
        } else {
            Ok(Box::new(chained))
        }
    }

    /// The Docker Engine connection, or the typed error of a host without one.
    fn docker(&self) -> SandboxResult<&Arc<dyn DockerApi>> {
        self.docker.as_ref().ok_or_else(|| {
            SandboxError::DriverUnavailable {
                driver: DriverId::Docker,
                reason: "no Docker Engine client is configured".to_string(),
            }
            .into()
        })
    }

    /// The resolved image size in mebibytes.
    fn size_mib(&self, options: &ImportOptions) -> u32 {
        if options.size_mib == 0 {
            self.host.config.image_limits.default_size_mib
        } else {
            options.size_mib
        }
    }

    /// A fresh random name for one temporary container.
    fn random_name(&self) -> SandboxResult<String> {
        let rng = self.ctx.rng();
        let mut rng = rng.lock().map_err(|_| SandboxError::Io {
            op: "name the import container".to_string(),
            message: "the random number generator lock is poisoned".to_string(),
        })?;
        Ok(format!("{:016x}", rng.next_u64()))
    }

    /// Streams the SHA-256 of one file.
    fn hash_file(&self, path: &Path) -> SandboxResult<String> {
        let mut reader = self
            .host
            .fs
            .open_read(path)
            .map_err(|error| SandboxError::Io {
                op: "read the image source".to_string(),
                message: format!("{}: {error}", path.display()),
            })?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0_u8; CHUNK_BYTES];
        loop {
            let read = reader.read(&mut buffer).map_err(|error| SandboxError::Io {
                op: "read the image source".to_string(),
                message: format!("{}: {error}", path.display()),
            })?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        Ok(hex_lower(&hasher.finalize()))
    }
}

/// One identified source: its identity, architecture, plan and manifest.
struct Identified {
    /// The text that names the exact content.
    identity: String,
    /// The architecture of the programs in the image.
    arch: GuestArch,
    /// How the export stream is opened.
    plan: SourcePlan,
    /// The manifest, before the commit fills the size and the hash.
    manifest: ImageManifest,
    /// The container configuration fields that a sandbox cannot use.
    dropped: Vec<String>,
}

/// How one source gives its file tree.
enum SourcePlan {
    /// A container of the local Docker daemon.
    Docker { name: String },
    /// A tar file on the host.
    File { path: PathBuf },
    /// A container image in a registry (feature 15).
    Oci(Box<OciPlan>),
}

/// The registry image that one import reads.
struct OciPlan {
    /// The client, with the token that the manifest read got.
    client: RegistryClient,
    /// The reference that the import read.
    reference: OciReference,
    /// The digest of the configuration blob.
    config_digest: String,
    /// The layers with their readers, in manifest order.
    layers: Vec<(OciDescriptor, LayerKind)>,
}

impl OciPlan {
    /// The hex names of the blobs that this import named.
    ///
    /// The blob cache never removes one of them (feature 15, section 6.4).
    fn keep_hexes(&self) -> Vec<String> {
        let mut keep = Vec::with_capacity(self.layers.len() + 1);
        if let Ok(hex) = digest_hex(&self.config_digest) {
            keep.push(hex.to_string());
        }
        for (descriptor, _) in &self.layers {
            if let Ok(hex) = digest_hex(&descriptor.digest) {
                keep.push(hex.to_string());
            }
        }
        keep
    }
}

/// The state that a failed import must undo.
struct Fill {
    /// The temporary container, until it is removed.
    container: Option<(Arc<dyn DockerApi>, String)>,
    /// The work image, until it is committed or removed.
    work: Option<PathBuf>,
    /// The fill sandbox, until it stops.
    sandbox: Option<SandboxHandle>,
}

impl Fill {
    fn new() -> Self {
        Self {
            container: None,
            work: None,
            sandbox: None,
        }
    }

    /// Removes the temporary container (step 6). A missing container is `Ok`.
    async fn remove_container(&mut self) {
        if let Some((api, id)) = self.container.take() {
            let _ = api.remove_container(&id, true).await;
        }
    }

    /// Stops the fill sandbox and removes the work image.
    async fn discard(&mut self, fs: &dyn crate::shared::host::HostFs) {
        if let Some(sandbox) = self.sandbox.take() {
            let _ = sandbox.stop().await;
        }
        if let Some(work) = self.work.take() {
            let _ = fs.remove_file(&work);
        }
    }
}

/// A progress sink: a channel in [`ImageImporter::import_with_progress`], or
/// none in [`ImageImporter::import`].
#[derive(Clone, Default)]
struct ProgressSink(Option<UnboundedSender<ImportEvent>>);

impl ProgressSink {
    fn progress(&self, progress: ImportProgress) {
        if let Some(sender) = &self.0 {
            let _ = sender.unbounded_send(ImportEvent::Progress(progress));
        }
    }
}

/// The image key of a source identity (feature 13, section 4.1):
/// `SHA-256("ewe-image-v1\n" + identity + "\n" + arch + "\n" + size_mib)`,
/// first 32 hexadecimal characters.
#[must_use]
pub fn image_key(identity: &str, arch: GuestArch, size_mib: u32) -> String {
    let mut hasher = Sha256::new();
    hasher.update(KEY_PREFIX.as_bytes());
    hasher.update(b"\n");
    hasher.update(identity.as_bytes());
    hasher.update(b"\n");
    hasher.update(arch.as_str().as_bytes());
    hasher.update(b"\n");
    hasher.update(size_mib.to_string().as_bytes());
    hex_lower(&hasher.finalize())[..32].to_string()
}

/// Maps the container configuration to a manifest (feature 13, section 6.3).
///
/// It returns the manifest and the container fields that a sandbox cannot use.
/// A sandbox has no port publishing and no volume in this spec.
#[must_use]
pub fn config_to_manifest(
    config: &ImageConfig,
    options: &ImportOptions,
    arch: GuestArch,
    source: &str,
) -> (ImageManifest, Vec<String>) {
    let mut argv = Vec::new();
    if let Some(entrypoint) = &config.entrypoint {
        argv.extend(entrypoint.iter().cloned());
    }
    if let Some(cmd) = &config.cmd {
        argv.extend(cmd.iter().cloned());
    }
    let cmd = if let Some(replaced) = &options.cmd {
        Some(replaced.clone())
    } else if argv.is_empty() {
        None
    } else {
        Some(argv)
    };

    let mut env = Vec::new();
    if let Some(entries) = &config.env {
        for entry in entries {
            let (key, value) = entry.split_once('=').unwrap_or((entry.as_str(), ""));
            env.push((key.to_string(), value.to_string()));
        }
    }
    env.extend(options.env.iter().cloned());

    let workdir = config.working_dir.clone().filter(|value| !value.is_empty());
    let user = config.user.clone().filter(|value| !value.is_empty());

    let mut dropped = Vec::new();
    for (set, name) in [
        (json_is_set(config.exposed_ports.as_ref()), "ExposedPorts"),
        (json_is_set(config.volumes.as_ref()), "Volumes"),
        (config.healthcheck.is_some(), "Healthcheck"),
        (
            config
                .stop_signal
                .as_ref()
                .is_some_and(|value| !value.is_empty()),
            "StopSignal",
        ),
        (json_is_set(config.labels.as_ref()), "Labels"),
        (
            config
                .on_build
                .as_ref()
                .is_some_and(|list| !list.is_empty()),
            "OnBuild",
        ),
        (
            config.shell.as_ref().is_some_and(|list| !list.is_empty()),
            "Shell",
        ),
    ] {
        if set {
            dropped.push(name.to_string());
        }
    }

    let manifest = ImageManifest {
        schema: IMAGE_SCHEMA,
        key: String::new(),
        arch,
        created_unix_ms: 0,
        cmd,
        env,
        workdir,
        user,
        kernel: None,
        rootfs_sha256: String::new(),
        rootfs_size: 0,
        source: source.to_string(),
        steps: Vec::new(),
    };
    (manifest, dropped)
}

/// The manifest of a source without a container configuration.
#[must_use]
pub fn options_manifest(options: &ImportOptions, arch: GuestArch, source: &str) -> ImageManifest {
    ImageManifest {
        schema: IMAGE_SCHEMA,
        key: String::new(),
        arch,
        created_unix_ms: 0,
        cmd: options.cmd.clone(),
        env: options.env.clone(),
        workdir: None,
        user: None,
        kernel: None,
        rootfs_sha256: String::new(),
        rootfs_size: 0,
        source: source.to_string(),
        steps: Vec::new(),
    }
}

/// The unpack options of a container export (feature 13, section 6.4).
///
/// The export is a flat file tree, so it has no whiteouts. `/.dockerenv` is
/// the one entry that a sandbox root does not need; `/etc/hosts`,
/// `/etc/hostname`, `/etc/resolv.conf` and `/dev` stay, and the slave replaces
/// them when the sandbox has a network or uses `devtmpfs`.
#[must_use]
pub fn export_unpack_options() -> UnpackOptions {
    UnpackOptions {
        oci_whiteouts: false,
        skip: vec![SKIP_EXPORT.to_string()],
        keep_existing_dirs: false,
    }
}

/// The unpack options of one registry layer (feature 15, section 6.4).
///
/// A layer carries whiteouts, so a later layer removes or replaces the files of
/// an earlier one. No entry is skipped: the layer is the image.
#[must_use]
pub fn oci_unpack_options() -> UnpackOptions {
    UnpackOptions {
        oci_whiteouts: true,
        skip: Vec::new(),
        keep_existing_dirs: false,
    }
}

/// Maps one architecture word of the daemon to a guest architecture.
///
/// # Errors
/// [`SandboxError::ImageArchMismatch`] when the image is not for this guest,
/// including an architecture that this platform does not name.
pub fn image_arch(name: &str, architecture: Option<&str>) -> SandboxResult<GuestArch> {
    let host = GuestArch::host();
    let arch = match architecture {
        Some("amd64" | "x86_64") => Some(GuestArch::X86_64),
        Some("arm64" | "aarch64") => Some(GuestArch::Aarch64),
        _ => None,
    };
    match arch {
        Some(arch) if arch == host => Ok(arch),
        _ => Err(SandboxError::ImageArchMismatch {
            image: format!("{name} (arch {})", architecture.unwrap_or("unknown")),
            host: host.as_str().to_string(),
        }
        .into()),
    }
}

/// Refuses an Alpine version that feature 03's manifest does not pin.
fn check_alpine_version(version: &str) -> SandboxResult<()> {
    let pinned = crate::native::artifacts::manifest::ALPINE_VERSION;
    let short = pinned
        .rsplit_once('.')
        .map_or(pinned, |(major_minor, _patch)| major_minor);
    if version == pinned || version == short {
        Ok(())
    } else {
        Err(SandboxError::InvalidSpec {
            field: "version".to_string(),
            reason: format!("the Alpine version {version} is not the pinned {pinned}"),
        }
        .into())
    }
}

/// Maps an unpack failure to the size limit that the caller can change.
fn map_unpack_error(error: ErrorTrace<SandboxError>, size_mib: u32) -> ErrorTrace<SandboxError> {
    let limit = u64::from(size_mib) * 1_048_576;
    match error.current_context() {
        SandboxError::SlaveError {
            code: SlaveErrorCode::Io,
            errno: Some(errno),
            ..
        } if *errno == libc::ENOSPC => SandboxError::ArtifactTooLarge {
            limit,
            message: format!("the image is larger than size_mib = {size_mib}; pass a larger size"),
        }
        .into(),
        _ => error,
    }
}

/// Adds the layer index to the error of one layer unpack.
///
/// The fault table of feature 15 says the caller sees the unpack error with the
/// layer index.
fn map_layer_unpack_error(
    error: ErrorTrace<SandboxError>,
    index: usize,
    size_mib: u32,
) -> ErrorTrace<SandboxError> {
    match map_unpack_error(error, size_mib).current_context() {
        SandboxError::SlaveError {
            code,
            errno,
            message,
        } => SandboxError::SlaveError {
            code: *code,
            errno: *errno,
            message: format!("layer {index}: {message}"),
        }
        .into(),
        SandboxError::ArtifactTooLarge { limit, message } => SandboxError::ArtifactTooLarge {
            limit: *limit,
            message: format!("layer {index}: {message}"),
        }
        .into(),
        other => other.clone().into(),
    }
}

/// The reference that one import returns.
fn reference_for(key: &str, tag: Option<&str>) -> ImageRef {
    match tag {
        Some(tag) => ImageRef::Tag(tag.to_string()),
        None => ImageRef::Key(key.to_string()),
    }
}

/// Whether one JSON configuration value is set and not empty.
fn json_is_set(value: Option<&serde_json::Value>) -> bool {
    match value {
        None | Some(serde_json::Value::Null) => false,
        Some(serde_json::Value::Object(map)) => !map.is_empty(),
        Some(serde_json::Value::Array(list)) => !list.is_empty(),
        Some(_) => true,
    }
}

/// Reads a pull to its last progress line, with the pull time limit.
async fn read_pull(
    mut stream: ProgressStream<CreateImageInfo>,
    ctx: &Ctx,
    timeout: std::time::Duration,
) -> SandboxResult<()> {
    let read = async {
        while let Some(item) = stream.next().await {
            item.map_err(map_error)?;
        }
        Ok(())
    };
    let timer = match ctx.require_env() {
        Ok(env) => Some(env.timer()),
        Err(_) => None,
    };
    match timer {
        Some(timer) => {
            let deadline = ctx.clock().now() + timeout;
            match select(Box::pin(read), timer.sleep_until(ctx.clone(), deadline)).await {
                Either::Left((result, _)) => result,
                Either::Right(_) => Err(SandboxError::ApiTimeout {
                    timeout_ms: u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX),
                }
                .into()),
            }
        }
        None => read.await,
    }
}

/// The export of one container as a byte stream, with the byte count and the
/// progress values.
fn docker_stream(body: ByteBody, max_bytes: u64, progress: ProgressSink) -> BoxByteStream {
    Box::pin(futures::stream::unfold(
        DockerRead {
            body,
            total: 0,
            reported: 0,
            max_bytes,
            progress,
        },
        |mut state| async move {
            match state.body.next().await {
                None => None,
                Some(Err(error)) => Some((Err(map_error(error).into()), state)),
                Some(Ok(bytes)) => {
                    state.total = state.total.saturating_add(bytes.len() as u64);
                    if state.total > state.max_bytes {
                        let error: ErrorTrace<SandboxError> = SandboxError::ArtifactTooLarge {
                            limit: state.max_bytes,
                            message: format!(
                                "the image export is larger than {} bytes",
                                state.max_bytes
                            ),
                        }
                        .into();
                        return Some((Err(error), state));
                    }
                    if state.total - state.reported >= PROGRESS_STEP {
                        state.reported = state.total;
                        state
                            .progress
                            .progress(ImportProgress::Filling { bytes: state.total });
                    }
                    Some((Ok(bytes.to_vec()), state))
                }
            }
        },
    ))
}

/// The counting state of one Docker export stream.
struct DockerRead {
    body: ByteBody,
    total: u64,
    reported: u64,
    max_bytes: u64,
    progress: ProgressSink,
}

/// One tar file as a byte stream, with the byte count and the progress values.
///
/// The reader blocks for one chunk at a time. This is the import boundary: the
/// read happens between two guest calls, so it never blocks a sandbox.
fn file_stream(
    reader: Box<dyn std::io::Read + Send>,
    max_bytes: u64,
    progress: ProgressSink,
) -> BoxByteStream {
    Box::pin(futures::stream::unfold(
        FileRead {
            reader,
            total: 0,
            reported: 0,
            max_bytes,
            progress,
        },
        |mut state| async move {
            let mut buffer = vec![0_u8; CHUNK_BYTES];
            match state.reader.read(&mut buffer) {
                Ok(0) => None,
                Err(error) => {
                    let error: ErrorTrace<SandboxError> = SandboxError::Io {
                        op: "read the image source".to_string(),
                        message: error.to_string(),
                    }
                    .into();
                    Some((Err(error), state))
                }
                Ok(count) => {
                    buffer.truncate(count);
                    state.total = state.total.saturating_add(count as u64);
                    if state.total > state.max_bytes {
                        let error: ErrorTrace<SandboxError> = SandboxError::ArtifactTooLarge {
                            limit: state.max_bytes,
                            message: format!(
                                "the image source is larger than {} bytes",
                                state.max_bytes
                            ),
                        }
                        .into();
                        return Some((Err(error), state));
                    }
                    if state.total - state.reported >= PROGRESS_STEP {
                        state.reported = state.total;
                        state
                            .progress
                            .progress(ImportProgress::Filling { bytes: state.total });
                    }
                    Some((Ok(buffer), state))
                }
            }
        },
    ))
}

/// The counting state of one tar file stream.
struct FileRead {
    reader: Box<dyn std::io::Read + Send>,
    total: u64,
    reported: u64,
    max_bytes: u64,
    progress: ProgressSink,
}

/// The lower-case hexadecimal text of `bytes`.
fn hex_lower(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(char::from(DIGITS[usize::from(byte >> 4)]));
        text.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    text
}
