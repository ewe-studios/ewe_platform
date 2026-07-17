//! Images group methods for [`DockerClient`].
//!
//! WHY: Docker images operations — listing, inspecting, tagging, deleting,
//! searching, building, pushing, pulling, loading, committing, pruning, and
//! history — are called through the generated async `*_request` functions.
//!
//! WHAT: Hand-written wrapper methods over the generated per-endpoint functions
//! for the Images API group.
//!
//! HOW: Each method constructs the generated Args struct, calls the matching
//! `*_request` function, and returns the parsed response body. Streaming
//! endpoints (push, pull, load) return raw `Vec<u8>` for now; `build` parses its
//! stream, since the daemon reports build failures inside a 200 response.

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
}

/// The result of a successful build.
#[derive(Debug, Clone, Default)]
pub struct ImageBuildOutcome {
    /// The build log, as the daemon streamed it (`Step 1/3 : FROM …`).
    pub logs: String,
    /// The built image's id, when the daemon reported one.
    pub image_id: Option<String>,
}

use foundation_netio::shared::client::body_reader::collect_bytes_from_send_safe;
use foundation_netio::shared::http::SimpleHeader;
use foundation_netio::PreparedRequestBuilder;

use crate::client::build_context::ContextTar;
use crate::streaming::decoder::JsonLineDecoder;
use crate::DockerClient;
use crate::error::DockerError;
use crate::generated::shared::ApiError;
use crate::generated::commit::{image_commit_request, ContainerConfig, ImageCommitArgs};
use crate::generated::create::{image_create_request, ImageCreateArgs};
use crate::generated::history::{image_history_request, ImageHistoryArgs};
use crate::generated::images::{image_delete_request, ImageDeleteArgs};
use crate::generated::json::{image_inspect_request, image_list_request, ImageInspect, ImageInspectArgs, ImageListArgs};
use crate::generated::load::{image_load_request, ImageLoadArgs};
use crate::generated::prune::{image_prune_request, ImagePruneArgs};
use crate::generated::push::{image_push_request, ImagePushArgs};
use crate::generated::search::{image_search_request, ImageSearchArgs};
use crate::generated::shared::IDResponse;
use crate::generated::tag::{image_tag_request, ImageTagArgs};

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
            image_list_request(self.http(), &args, &self.base_url(), None::<super::NoMod>).await?;
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
            image_inspect_request(self.http(), &args, &self.base_url(), None::<super::NoMod>).await?;
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
            image_history_request(self.http(), &args, &self.base_url(), None::<super::NoMod>).await?;
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
        image_tag_request(self.http(), &args, &self.base_url(), None::<super::NoMod>).await?;
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
            platforms: None,
        };
        let response =
            image_delete_request(self.http(), &args, &self.base_url(), None::<super::NoMod>).await?;
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
            image_search_request(self.http(), &args, &self.base_url(), None::<super::NoMod>).await?;
        Ok(response.body)
    }

    /// Load images from a tar archive (`POST /images/load`).
    ///
    /// The tar body is set via `builder_mod`. The response is raw bytes
    /// (streaming endpoint).
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn image_load(
        &self,
        body: serde_json::Value,
        quiet: Option<bool>,
    ) -> Result<Vec<u8>, DockerError> {
        let args = ImageLoadArgs {
            quiet: quiet.map(|v| v.to_string()),
            platform: None,
        };
        image_load_request(
            self.http(),
            &args,
            &self.base_url(),
            Some(move |b: &mut PreparedRequestBuilder| {
                let _ = b.set_body_json(&body);
            }),
        )
        .await?;
        Ok(Vec::new())
    }

    /// Commit a container as a new image (`POST /commit`).
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn image_commit(
        &self,
        container: Option<&str>,
        repo: Option<&str>,
        tag: Option<&str>,
        comment: Option<&str>,
        author: Option<&str>,
        pause: Option<bool>,
        changes: Option<&str>,
    ) -> Result<IDResponse, DockerError> {
        let args = ImageCommitArgs {
            container: container.map(str::to_string),
            repo: repo.map(str::to_string),
            tag: tag.map(str::to_string),
            comment: comment.map(str::to_string),
            author: author.map(str::to_string),
            pause: pause.map(|v| v.to_string()),
            changes: changes.map(str::to_string),
            body: ContainerConfig::default(),
        };
        let response =
            image_commit_request(self.http(), &args, &self.base_url(), None::<super::NoMod>).await?;
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
        image_prune_request(self.http(), &args, &self.base_url(), None::<super::NoMod>).await?;
        Ok(())
    }

    /// Build an image from a Dockerfile (`POST /build`).
    ///
    /// WHY: `context` is the build context as a tar stream — the daemon cannot
    /// read the caller's disk, so the Dockerfile and everything `COPY`/`ADD`
    /// touches must be uploaded with the request. Use [`ContextTar`] to make one.
    ///
    /// WHAT: Returns the build's progress log. The daemon reports **build
    /// failures inside a 200 response** (a `{"error": …}` object part-way down
    /// the stream), so the stream is parsed and such a failure is surfaced as
    /// [`DockerError::BuildFailed`] rather than looking like success.
    ///
    /// HOW: Sent directly rather than via the generated `image_build_request`,
    /// which sets query parameters but **no body** (so `/build` had no context to
    /// build and every call failed or hung) and discarded the response.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure, non-2xx status, or a build
    /// error reported in the stream.
    pub async fn image_build(
        &self,
        context: &ContextTar,
        opts: &ImageBuildOptions,
    ) -> Result<ImageBuildOutcome, DockerError> {
        let endpoint_url = format!("{}/build", self.base_url());
        let mut builder = PreparedRequestBuilder::post(&endpoint_url)
            .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))?;

        builder = builder.query("dockerfile", opts.dockerfile.as_deref());
        builder = builder.query("t", opts.tag.as_deref());
        builder = builder.query("target", opts.target.as_deref());
        builder = builder.query("platform", opts.platform.as_deref());
        builder = builder.query("nocache", Some(opts.no_cache.to_string()).as_deref());
        builder = builder.query("pull", Some(opts.pull.to_string()).as_deref());
        // Always clean up intermediate containers; leaving them is never what a
        // caller wants and they are invisible to the returned handle.
        builder = builder.query("rm", Some("true"));

        if !opts.build_args.is_empty() {
            let map: std::collections::HashMap<&str, &str> = opts
                .build_args
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            let encoded = serde_json::to_string(&map)
                .map_err(|e| DockerError::JsonParse(format!("buildargs: {e}")))?;
            builder = builder.query("buildargs", Some(encoded).as_deref());
        }
        if !opts.labels.is_empty() {
            let map: std::collections::HashMap<&str, &str> = opts
                .labels
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            let encoded = serde_json::to_string(&map)
                .map_err(|e| DockerError::JsonParse(format!("labels: {e}")))?;
            builder = builder.query("labels", Some(encoded).as_deref());
        }

        // `body_bytes` sets Content-Length; the daemon needs the tar content type.
        builder = builder.body_bytes(context.as_bytes().to_vec());
        builder.set_header(SimpleHeader::CONTENT_TYPE, "application/x-tar");

        let response = self
            .http()
            .send_async(builder.build())
            .await
            .map_err(|e| ApiError::RequestSendFailed(e.to_string()))?;

        let status: usize = response.get_status().into();
        let headers = response.get_headers_ref().clone();
        if !(200..300).contains(&status) {
            return Err(ApiError::HttpStatus {
                code: status as u16,
                headers,
                body: None,
            }
            .into());
        }

        let body = collect_bytes_from_send_safe(response.take_body());
        Self::parse_build_stream(&body)
    }

    /// Read the `/build` progress stream: collect the log, pick out the built
    /// image id, and fail on a reported build error.
    fn parse_build_stream(body: &[u8]) -> Result<ImageBuildOutcome, DockerError> {
        let mut decoder = JsonLineDecoder::new();
        decoder.feed(body);

        let mut logs = String::new();
        let mut image_id = None;

        while let Some(line) = decoder.decode() {
            let Ok(value) = serde_json::from_slice::<serde_json::Value>(&line) else {
                continue;
            };

            if let Some(err) = value.get("error").and_then(serde_json::Value::as_str) {
                return Err(DockerError::BuildFailed(err.to_string()));
            }
            if let Some(text) = value.get("stream").and_then(serde_json::Value::as_str) {
                logs.push_str(text);
            }
            // The final `aux` object carries the built image's id.
            if let Some(id) = value
                .get("aux")
                .and_then(|aux| aux.get("ID"))
                .and_then(serde_json::Value::as_str)
            {
                image_id = Some(id.to_string());
            }
        }

        Ok(ImageBuildOutcome { logs, image_id })
    }

    /// Push an image to a registry (`POST /images/{name}/push`).
    ///
    /// Accepts an optional `X-Registry-Auth` header for authentication.
    /// This is a streaming endpoint; returns raw bytes for now.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn image_push(
        &self,
        name: &str,
        tag: Option<&str>,
        x_registry_auth: Option<&str>,
    ) -> Result<Vec<u8>, DockerError> {
        let args = ImagePushArgs {
            name: name.to_string(),
            tag: tag.map(str::to_string),
            platform: None,
        };
        let auth = x_registry_auth.map(str::to_string);
        image_push_request(
            self.http(),
            &args,
            &self.base_url(),
            Some(move |b: &mut PreparedRequestBuilder| {
                if let Some(ref auth) = auth {
                    b.set_header(SimpleHeader::custom("X-Registry-Auth"), auth.clone());
                }
            }),
        )
        .await?;
        Ok(Vec::new())
    }

    /// Pull an image from a registry (`POST /images/create`).
    ///
    /// This is a streaming endpoint; returns raw bytes for now.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn image_pull(
        &self,
        from_image: Option<&str>,
        from_src: Option<&str>,
        repo: Option<&str>,
        tag: Option<&str>,
        message: Option<&str>,
        platform: Option<&str>,
    ) -> Result<Vec<u8>, DockerError> {
        let args = ImageCreateArgs {
            from_image: from_image.map(str::to_string),
            from_src: from_src.map(str::to_string),
            repo: repo.map(str::to_string),
            tag: tag.map(str::to_string),
            message: message.map(str::to_string),
            changes: None,
            platform: platform.map(str::to_string),
        };
        image_create_request(self.http(), &args, &self.base_url(), None::<super::NoMod>).await?;
        Ok(Vec::new())
    }

    /// Save (export) an image as a tar archive (`GET /images/{name}/get`).
    ///
    /// Bollard calls this `export_image`. Returns raw tar bytes.
    /// Manual request builder — the generated function tries to JSON-parse
    /// the tar stream, which fails.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn image_save(
        &self,
        name: &str,
    ) -> Result<Vec<u8>, DockerError> {
        use foundation_netio::shared::client::body_reader::collect_bytes_from_send_safe;

        let endpoint_url = format!("{}/images/{}/get", self.base_url(), name);
        let builder = PreparedRequestBuilder::get(&endpoint_url)
            .map_err(|e| crate::generated::shared::ApiError::RequestBuildFailed(e.to_string()))?;

        let response = self.http().send_async(builder.build()).await
            .map_err(|e| crate::generated::shared::ApiError::RequestSendFailed(e.to_string()))?;

        let status: usize = response.get_status().into();
        if status < 200 || status >= 300 {
            return Err(DockerError::Api { status: status as u16, message: "(no body)".into() });
        }
        Ok(collect_bytes_from_send_safe(response.take_body()))
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
        let response = distribution_inspect_request(self.http(), &args, &self.base_url(), None::<super::NoMod>).await?;
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
        build_prune_request(self.http(), &args, &self.base_url(), None::<super::NoMod>).await?;
        Ok(())
    }

    /// Export multiple images as a single tar archive (`GET /images/get`).
    ///
    /// WHY: Matches bollard's `export_images` — the multi-image companion to
    /// [`Self::image_save`] (`export_image`). The generated `image_get_all`
    /// function tries to JSON-parse the tar stream, so we build the request
    /// directly and collect raw bytes.
    ///
    /// WHAT: `names` is the set of image references to export (repeated
    /// `names=` query params joined by the caller as a comma-free repetition
    /// is not supported by the query helper, so pass a single comma-separated
    /// value as Docker accepts repeated `names`). Returns the raw tar bytes.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn export_images(&self, names: &[&str]) -> Result<Vec<u8>, DockerError> {
        use foundation_netio::shared::client::body_reader::collect_bytes_from_send_safe;

        let endpoint_url = format!("{}/images/get", self.base_url());
        let mut builder = PreparedRequestBuilder::get(&endpoint_url)
            .map_err(|e| crate::generated::shared::ApiError::RequestBuildFailed(e.to_string()))?;
        for name in names {
            builder = builder.query("names", Some(*name));
        }

        let response = self.http().send_async(builder.build()).await
            .map_err(|e| crate::generated::shared::ApiError::RequestSendFailed(e.to_string()))?;

        let status: usize = response.get_status().into();
        if status < 200 || status >= 300 {
            return Err(DockerError::Api { status: status as u16, message: "(no body)".into() });
        }
        Ok(collect_bytes_from_send_safe(response.take_body()))
    }
}
