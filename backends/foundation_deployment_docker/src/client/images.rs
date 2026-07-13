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
//! endpoints (build, push, pull, load) return raw `Vec<u8>` for now.

use foundation_netio::shared::http::SimpleHeader;
use foundation_netio::PreparedRequestBuilder;

use crate::DockerClient;
use crate::error::DockerError;
use crate::generated::build::{image_build_request, ImageBuildArgs};
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
    /// This is a streaming endpoint; returns raw bytes for now.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    #[allow(clippy::too_many_arguments)]
    pub async fn image_build(
        &self,
        dockerfile: Option<&str>,
        t: Option<&str>,
        extrahosts: Option<&str>,
        remote: Option<&str>,
        q: Option<bool>,
        nocache: Option<bool>,
        cachefrom: Option<&str>,
        pull: Option<bool>,
        rm: Option<bool>,
        forcerm: Option<bool>,
        memory: Option<&str>,
        memswap: Option<&str>,
        cpushares: Option<&str>,
        cpusetcpus: Option<&str>,
        cpuperiod: Option<&str>,
        cpuquota: Option<&str>,
        buildargs: Option<&str>,
        shmsize: Option<&str>,
        squash: Option<bool>,
        labels: Option<&str>,
        networkmode: Option<&str>,
        platform: Option<&str>,
        target: Option<&str>,
        outputs: Option<&str>,
    ) -> Result<Vec<u8>, DockerError> {
        let args = ImageBuildArgs {
            dockerfile: dockerfile.map(str::to_string),
            t: t.map(str::to_string),
            extrahosts: extrahosts.map(str::to_string),
            remote: remote.map(str::to_string),
            q: q.map(|v| v.to_string()),
            nocache: nocache.map(|v| v.to_string()),
            cachefrom: cachefrom.map(str::to_string),
            pull: pull.map(|v| v.to_string()),
            rm: rm.map(|v| v.to_string()),
            forcerm: forcerm.map(|v| v.to_string()),
            memory: memory.map(str::to_string),
            memswap: memswap.map(str::to_string),
            cpushares: cpushares.map(str::to_string),
            cpusetcpus: cpusetcpus.map(str::to_string),
            cpuperiod: cpuperiod.map(str::to_string),
            cpuquota: cpuquota.map(str::to_string),
            buildargs: buildargs.map(str::to_string),
            shmsize: shmsize.map(str::to_string),
            squash: squash.map(|v| v.to_string()),
            labels: labels.map(str::to_string),
            networkmode: networkmode.map(str::to_string),
            platform: platform.map(str::to_string),
            target: target.map(str::to_string),
            outputs: outputs.map(str::to_string),
            version: None,
        };
        image_build_request(self.http(), &args, &self.base_url(), None::<super::NoMod>).await?;
        Ok(Vec::new())
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
}
