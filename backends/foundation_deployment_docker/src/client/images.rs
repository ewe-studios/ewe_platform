//! Images group methods for [`DockerClient`].
//!
//! WHY: Docker images operations — listing, inspecting, tagging, deleting,
//! searching, building, pushing, pulling, loading, committing, pruning, and
//! history — are called through the generated async `*_request` functions.
//!
//! WHAT: Hand-written wrapper methods over the generated per-endpoint functions
//! for the Images API group.
//!
//! HOW: Each method constructs the generated Args struct and calls the matching
//! `*_request` function. A small JSON answer is returned parsed. An answer that
//! is a stream — pull, push, build and load progress, image tarballs — is
//! returned unread, as an async stream of typed items ([`ProgressStream`] of the
//! generated [`CreateImageInfo`], [`PushImageInfo`] or [`BuildInfo`]) or of
//! bytes ([`ByteBody`]). The generated function checks the status before the
//! body is touched, reading at most a bounded amount of an error body. The
//! daemon does a pull, push, build or load *while* it writes the progress, and
//! reports a failure *inside* the `200` stream, so the caller reads the stream
//! to its end (e.g. [`ProgressStream::finish`]) to learn the outcome.

/// What to build, and how (`POST /build` query parameters).
///
/// The build **context** is passed separately as a [`ContextTar`] — it is the
/// request body, not a parameter.
#[derive(Debug, Clone, Default)]
pub struct ImageBuildOptions {
    /// `name:tag` to apply to the built image.
    pub tag: Option<String>,
    /// Path of the Dockerfile *within the context* (default: `Dockerfile`).
    pub dockerfile: Option<String>,
    /// `ARG` values for the build.
    pub build_args: Vec<(String, String)>,
    /// Labels to set on the built image.
    pub labels: Vec<(String, String)>,
    /// Target platform, e.g. `linux/amd64`.
    pub platform: Option<String>,
    /// Stage to stop at, for a multi-stage Dockerfile.
    pub target: Option<String>,
    /// Ignore the build cache.
    pub no_cache: bool,
    /// Always attempt to pull a newer base image.
    pub pull: bool,
    /// Also remove intermediate containers when the build **fails**.
    ///
    /// Intermediates from a successful build are always removed. A failed one is
    /// kept by default (matching `docker build`), since it is the only way to
    /// inspect what went wrong — at the cost of a leftover container per failure.
    /// Set this when nothing will go looking, e.g. in tests.
    pub force_rm: bool,
}

/// What [`DockerClient::image_commit`] commits, and as what (`POST /commit`
/// query parameters). Every field is optional; the default commits nothing
/// in particular and the daemon rejects it — set at least `container`.
#[derive(Debug, Clone, Default)]
pub struct ImageCommitOptions {
    /// The container to commit, by id or name (`container`).
    pub container: Option<String>,
    /// Repository name for the new image (`repo`).
    pub repo: Option<String>,
    /// Tag for the new image (`tag`).
    pub tag: Option<String>,
    /// Commit message (`comment`).
    pub comment: Option<String>,
    /// Author of the image, e.g. `Name <email>` (`author`).
    pub author: Option<String>,
    /// Pause the container while committing (`pause`); `None` keeps the
    /// daemon's default, which pauses.
    pub pause: Option<bool>,
    /// Dockerfile instructions to apply while committing (`changes`).
    pub changes: Option<String>,
}

use foundation_netio::shared::http::{SendSafeBody, SimpleHeader};
use foundation_netio::PreparedRequestBuilder;

use crate::client::body::{ByteBody, ProgressStream};
use crate::client::build_context::ContextTar;
use crate::error::DockerError;
use crate::generated::build::{image_build_request, ImageBuildArgs};
use crate::generated::commit::{image_commit_request, ContainerConfig, ImageCommitArgs};
use crate::generated::create::{image_create_request, CreateImageInfo, ImageCreateArgs};
use crate::generated::get::{image_get_all_request, image_get_request, ImageGetAllArgs, ImageGetArgs};
use crate::generated::history::{image_history_request, ImageHistoryArgs};
use crate::generated::images::{image_delete_request, ImageDeleteArgs};
use crate::generated::json::{image_inspect_request, image_list_request, ImageInspect, ImageInspectArgs, ImageListArgs};
use crate::generated::load::{image_load_request, ImageLoadArgs};
use crate::generated::prune::{image_prune_request, ImagePruneArgs};
use crate::generated::push::{image_push_request, ImagePushArgs, PushImageInfo};
use crate::generated::search::{image_search_request, ImageSearchArgs};
use crate::generated::shared::{BuildInfo, IDResponse};
use crate::generated::tag::{image_tag_request, ImageTagArgs};
use crate::DockerClient;

impl DockerClient {
    /// List images (`GET /images/json`).
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn image_list(
        &self,
        all: Option<bool>,
        filters: Option<&str>,
        shared_size: Option<bool>,
        digests: Option<bool>,
    ) -> Result<serde_json::Value, DockerError> {
        let args = ImageListArgs {
            all: all.map(|v| v.to_string()),
            filters: filters.map(str::to_string),
            shared_size: shared_size.map(|v| v.to_string()),
            digests: digests.map(|v| v.to_string()),
            manifests: None,
        };
        let response =
            image_list_request(self.api(), &args, &self.api_url().await?, None::<super::NoMod>).await?;
        Ok(response.body)
    }

    /// Inspect an image (`GET /images/{name}/json`).
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn image_inspect(&self, name: &str) -> Result<ImageInspect, DockerError> {
        let args = ImageInspectArgs {
            name: name.to_string(),
            manifests: None,
            platform: None,
        };
        let response =
            image_inspect_request(self.api(), &args, &self.api_url().await?, None::<super::NoMod>).await?;
        Ok(response.body)
    }

    /// Get image history (`GET /images/{name}/history`).
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn image_history(&self, name: &str) -> Result<serde_json::Value, DockerError> {
        let args = ImageHistoryArgs {
            name: name.to_string(),
            platform: None,
        };
        let response =
            image_history_request(self.api(), &args, &self.api_url().await?, None::<super::NoMod>).await?;
        Ok(response.body)
    }

    /// Tag an image (`POST /images/{name}/tag`).
    ///
    /// Returns `201 No Content` on success.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn image_tag(
        &self,
        name: &str,
        repo: Option<&str>,
        tag: Option<&str>,
    ) -> Result<(), DockerError> {
        let args = ImageTagArgs {
            name: name.to_string(),
            repo: repo.map(str::to_string),
            tag: tag.map(str::to_string),
        };
        image_tag_request(self.api(), &args, &self.api_url().await?, None::<super::NoMod>).await?;
        Ok(())
    }

    /// Delete an image (`DELETE /images/{name}`).
    ///
    /// Returns the list of deleted layers as a JSON value.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn image_delete(
        &self,
        name: &str,
        force: Option<bool>,
        noprune: Option<bool>,
    ) -> Result<serde_json::Value, DockerError> {
        let args = ImageDeleteArgs {
            name: name.to_string(),
            force: force.map(|v| v.to_string()),
            noprune: noprune.map(|v| v.to_string()),
            platforms: Vec::new(),
        };
        let response =
            image_delete_request(self.api(), &args, &self.api_url().await?, None::<super::NoMod>).await?;
        Ok(response.body)
    }

    /// Search images on a registry (`GET /images/search`).
    ///
    /// Returns search results as a JSON value.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn image_search(
        &self,
        term: Option<&str>,
        limit: Option<u32>,
        filters: Option<&str>,
    ) -> Result<serde_json::Value, DockerError> {
        let args = ImageSearchArgs {
            term: term.map(str::to_string),
            limit: limit.map(|v| v.to_string()),
            filters: filters.map(str::to_string),
        };
        let response =
            image_search_request(self.api(), &args, &self.api_url().await?, None::<super::NoMod>).await?;
        Ok(response.body)
    }

    /// Load images from a tar archive (`POST /images/load`).
    ///
    /// WHAT: Uploads `tarball` (as made by [`Self::image_save`] or
    /// `docker save`) and returns the daemon's progress as a stream of
    /// [`BuildInfo`]; the images are loaded once it has been read to its end.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status. A
    /// failure reported in the stream comes out of it as
    /// [`DockerError::LoadFailed`].
    pub async fn image_load(
        &self,
        tarball: SendSafeBody,
        quiet: Option<bool>,
    ) -> Result<ProgressStream<BuildInfo>, DockerError> {
        let args = ImageLoadArgs {
            quiet: quiet.map(|v| v.to_string()),
            platform: Vec::new(),
        };
        let response = image_load_request(
            self.api(),
            &args,
            &self.api_url().await?,
            Some(tarball),
            None::<super::NoMod>,
        )
        .await?;
        Ok(ProgressStream::new(response.body, DockerError::LoadFailed))
    }

    /// Commit a container as a new image (`POST /commit`).
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn image_commit(&self, opts: &ImageCommitOptions) -> Result<IDResponse, DockerError> {
        let args = ImageCommitArgs {
            container: opts.container.clone(),
            repo: opts.repo.clone(),
            tag: opts.tag.clone(),
            comment: opts.comment.clone(),
            author: opts.author.clone(),
            pause: opts.pause.map(|v| v.to_string()),
            changes: opts.changes.clone(),
            body: ContainerConfig::default(),
        };
        let response =
            image_commit_request(self.api(), &args, &self.api_url().await?, None::<super::NoMod>).await?;
        Ok(response.body)
    }

    /// Prune unused images (`POST /images/prune`).
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn image_prune(&self, filters: Option<&str>) -> Result<(), DockerError> {
        let args = ImagePruneArgs {
            filters: filters.map(str::to_string),
        };
        image_prune_request(self.api(), &args, &self.api_url().await?, None::<super::NoMod>).await?;
        Ok(())
    }

    /// Build an image from a Dockerfile (`POST /build`).
    ///
    /// WHY: `context` is the build context as a tar stream — the daemon cannot
    /// read the caller's disk, so the Dockerfile and everything `COPY`/`ADD`
    /// touches must be uploaded with the request. Use [`ContextTar`] to make one.
    ///
    /// WHAT: Returns the build's output as a stream of [`BuildInfo`]. The
    /// daemon builds while it writes it, and reports **a failed build inside
    /// the `200` stream**; read it to its end — [`ProgressStream::image_id`]
    /// does, and returns the built image — to learn the outcome.
    ///
    /// HOW: The generated `image_build_request`, with the context as its raw
    /// body (sent as `application/x-tar`).
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status; a
    /// failed build comes out of the stream as [`DockerError::BuildFailed`].
    pub async fn image_build(
        &self,
        context: &ContextTar,
        opts: &ImageBuildOptions,
    ) -> Result<ProgressStream<BuildInfo>, DockerError> {
        let json_map = |what: &str, pairs: &[(String, String)]| {
            if pairs.is_empty() {
                return Ok(None);
            }
            let map: std::collections::HashMap<&str, &str> =
                pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
            serde_json::to_string(&map)
                .map(Some)
                .map_err(|e| DockerError::JsonParse(format!("{what}: {e}")))
        };
        let args = ImageBuildArgs {
            dockerfile: opts.dockerfile.clone(),
            t: opts.tag.clone(),
            target: opts.target.clone(),
            platform: opts.platform.clone(),
            nocache: Some(opts.no_cache.to_string()),
            pull: Some(opts.pull.to_string()),
            // Remove intermediate containers from a successful build; `forcerm`
            // also removes them when it fails (see `ImageBuildOptions::force_rm`).
            rm: Some("true".to_string()),
            forcerm: opts.force_rm.then(|| "true".to_string()),
            buildargs: json_map("buildargs", &opts.build_args)?,
            labels: json_map("labels", &opts.labels)?,
            ..ImageBuildArgs::default()
        };
        let response = image_build_request(
            self.api(),
            &args,
            &self.api_url().await?,
            Some(SendSafeBody::Bytes(context.as_bytes().to_vec())),
            // The spec declares the body as `application/octet-stream`; it is a
            // tar archive, and the daemon is told so.
            Some(|b: &mut PreparedRequestBuilder| {
                b.set_header(SimpleHeader::CONTENT_TYPE, "application/x-tar");
            }),
        )
        .await?;
        Ok(ProgressStream::new(response.body, DockerError::BuildFailed))
    }

    /// Push an image to a registry (`POST /images/{name}/push`).
    ///
    /// Accepts an optional `X-Registry-Auth` header for authentication.
    ///
    /// WHAT: Returns the push progress as a stream of [`PushImageInfo`]. The
    /// daemon pushes while it writes the stream: read it to its end
    /// ([`ProgressStream::finish`]) before relying on the push.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status; a failed
    /// push comes out of the stream as [`DockerError::PushFailed`].
    pub async fn image_push(
        &self,
        name: &str,
        tag: Option<&str>,
        x_registry_auth: Option<&str>,
    ) -> Result<ProgressStream<PushImageInfo>, DockerError> {
        let args = ImagePushArgs {
            name: name.to_string(),
            tag: tag.map(str::to_string),
            platform: None,
        };
        let auth = x_registry_auth.map(str::to_string);
        let response = image_push_request(
            self.api(),
            &args,
            &self.api_url().await?,
            Some(move |b: &mut PreparedRequestBuilder| {
                if let Some(auth) = auth {
                    b.set_header(SimpleHeader::custom("X-Registry-Auth"), auth);
                }
            }),
        )
        .await?;
        Ok(ProgressStream::new(response.body, DockerError::PushFailed))
    }

    /// Pull an image from a registry (`POST /images/create`).
    ///
    /// WHAT: Returns the pull progress as a stream of [`CreateImageInfo`]. The
    /// daemon pulls while it writes the stream: read it to its end
    /// ([`ProgressStream::finish`]) before using the image — dropping it early
    /// closes the connection, and the daemon may abandon the pull.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status; a failed
    /// pull (no such image, access denied) comes out of the stream as
    /// [`DockerError::PullFailed`].
    pub async fn image_pull(
        &self,
        from_image: Option<&str>,
        from_src: Option<&str>,
        repo: Option<&str>,
        tag: Option<&str>,
        message: Option<&str>,
        platform: Option<&str>,
    ) -> Result<ProgressStream<CreateImageInfo>, DockerError> {
        let args = ImageCreateArgs {
            from_image: from_image.map(str::to_string),
            from_src: from_src.map(str::to_string),
            repo: repo.map(str::to_string),
            tag: tag.map(str::to_string),
            message: message.map(str::to_string),
            changes: Vec::new(),
            platform: platform.map(str::to_string),
        };
        let response = image_create_request(
            self.api(),
            &args,
            &self.api_url().await?,
            // No body: this pulls. (An import from `fromSrc=-` would upload one.)
            None,
            None::<super::NoMod>,
        )
        .await?;
        Ok(ProgressStream::new(response.body, DockerError::PullFailed))
    }

    /// Save (export) an image as a tar archive (`GET /images/{name}/get`).
    ///
    /// Bollard calls this `export_image`. Returns the archive unread, as a
    /// stream of byte chunks: an image can be gigabytes, so write it where it
    /// is going chunk by chunk rather than collecting it.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status (checked
    /// before the body is read); a failed read comes out of the stream.
    pub async fn image_save(&self, name: &str) -> Result<ByteBody, DockerError> {
        let args = ImageGetArgs {
            name: name.to_string(),
            platform: Vec::new(),
        };
        let response =
            image_get_request(self.api(), &args, &self.api_url().await?, None::<super::NoMod>).await?;
        Ok(ByteBody::new(response.body))
    }

    /// Inspect an image on a registry (`GET /distribution/{name}/json`).
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn distribution_inspect(
        &self,
        name: &str,
    ) -> Result<crate::generated::json::DistributionInspect, DockerError> {
        use crate::generated::json::{distribution_inspect_request, DistributionInspectArgs};

        let args = DistributionInspectArgs {
            name: name.to_string(),
        };
        let response = distribution_inspect_request(self.api(), &args, &self.api_url().await?, None::<super::NoMod>).await?;
        Ok(response.body)
    }

    /// Prune the build cache (`POST /build/prune`).
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn build_prune(
        &self,
        reserved_space: Option<u32>,
        max_used_space: Option<u32>,
        all: Option<bool>,
        filters: Option<&str>,
    ) -> Result<(), DockerError> {
        use crate::generated::prune::{build_prune_request, BuildPruneArgs};

        let args = BuildPruneArgs {
            reserved_space: reserved_space.map(|v| v.to_string()),
            max_used_space: max_used_space.map(|v| v.to_string()),
            min_free_space: None,
            all: all.map(|v| v.to_string()),
            filters: filters.map(str::to_string),
        };
        build_prune_request(self.api(), &args, &self.api_url().await?, None::<super::NoMod>).await?;
        Ok(())
    }

    /// Export multiple images as a single tar archive (`GET /images/get`).
    ///
    /// WHY: Matches bollard's `export_images` — the multi-image companion to
    /// [`Self::image_save`] (`export_image`).
    ///
    /// WHAT: `names` are the image references to export, sent as repeated
    /// `names=` parameters. Returns the archive unread, as a stream of byte
    /// chunks.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status (checked
    /// before the body is read); a failed read comes out of the stream.
    pub async fn export_images(&self, names: &[&str]) -> Result<ByteBody, DockerError> {
        let args = ImageGetAllArgs {
            names: names.iter().map(|n| (*n).to_string()).collect(),
            platform: Vec::new(),
        };
        let response =
            image_get_all_request(self.api(), &args, &self.api_url().await?, None::<super::NoMod>).await?;
        Ok(ByteBody::new(response.body))
    }
}
