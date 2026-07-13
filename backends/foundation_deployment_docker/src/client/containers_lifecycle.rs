//! Container lifecycle operations for [`super::DockerClient`] — restart, kill,
//! rename, resize, top, changes, logs, stats, export, archive, attach, list, prune.
//!
//! WHY: These are the remaining container endpoints from the Docker Engine API
//! (v1.53) that complete the container lifecycle surface. Binary-stream endpoints
//! (logs, export, archive, attach) return raw `Vec<u8>` because the generated
//! functions either parse the body as JSON or discard it — neither is correct for
//! tar/multiplexed log streams.
//!
//! WHAT: One `impl DockerClient` block with a method per endpoint, following the
//! same pattern as `client/mod.rs` — construct the generated `Args` struct, call
//! into the transport, return the body.
//!
//! HOW: State-mutating commands (restart, kill, rename, resize, prune) use the
//! generated async functions that return `ApiResponse<()>`. Query endpoints (top,
//! stats, changes, list) use the generated functions and return the deserialized
//! body. Binary-stream endpoints bypass the generated layer for raw byte collection.

use foundation_netio::shared::client::body_reader::collect_bytes_from_send_safe;
use foundation_netio::shared::http::SimpleHeader;
use foundation_netio::PreparedRequestBuilder;

use crate::error::DockerError;
use crate::generated::shared::ApiError;
use crate::generated::update::{container_update_request, ContainerUpdateArgs, ContainerUpdateResponse};

use crate::generated::changes::{container_changes_request, ContainerChangesArgs};
use crate::generated::json::{container_list_request, ContainerListArgs};
use crate::generated::kill::{container_kill_request, ContainerKillArgs};
use crate::generated::prune::{container_prune_request, ContainerPruneArgs};
use crate::generated::rename::{container_rename_request, ContainerRenameArgs};
use crate::generated::resize::{container_resize_request, ContainerResizeArgs};
use crate::generated::restart::{container_restart_request, ContainerRestartArgs};
use crate::generated::stats::{container_stats_request, ContainerStatsArgs, ContainerStatsResponse};
use crate::generated::top::{container_top_request, ContainerTopArgs, ContainerTopResponse};

use super::DockerClient;
use super::NoMod;

impl DockerClient {
    // ── restart_container ───────────────────────────────────────────────

    /// Restart a container (`POST /containers/{id}/restart`).
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn restart_container(
        &self,
        id: &str,
        signal: Option<&str>,
        t: Option<u32>,
    ) -> Result<(), DockerError> {
        let args = ContainerRestartArgs {
            id: id.to_string(),
            signal: signal.map(str::to_string),
            t: t.map(|v| v.to_string()),
        };
        container_restart_request(self.http(), &args, &self.base_url(), None::<NoMod>).await?;
        Ok(())
    }

    // ── kill_container ──────────────────────────────────────────────────

    /// Kill a container (`POST /containers/{id}/kill`).
    ///
    /// WHY: Sends a POSIX signal (default `SIGKILL`) to the container's main
    /// process, bypassing the normal stop sequence.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn kill_container(
        &self,
        id: &str,
        signal: Option<&str>,
    ) -> Result<(), DockerError> {
        let args = ContainerKillArgs {
            id: id.to_string(),
            signal: signal.map(str::to_string),
        };
        container_kill_request(self.http(), &args, &self.base_url(), None::<NoMod>).await?;
        Ok(())
    }

    // ── rename_container ────────────────────────────────────────────────

    /// Rename a container (`POST /containers/{id}/rename`).
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn rename_container(&self, id: &str, name: &str) -> Result<(), DockerError> {
        let args = ContainerRenameArgs {
            id: id.to_string(),
            name: Some(name.to_string()),
        };
        container_rename_request(self.http(), &args, &self.base_url(), None::<NoMod>).await?;
        Ok(())
    }

    // ── resize_container ────────────────────────────────────────────────

    /// Resize the TTY of a container (`POST /containers/{id}/resize`).
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn resize_container(
        &self,
        id: &str,
        h: Option<u32>,
        w: Option<u32>,
    ) -> Result<(), DockerError> {
        let args = ContainerResizeArgs {
            id: id.to_string(),
            h: h.map(|v| v.to_string()),
            w: w.map(|v| v.to_string()),
        };
        container_resize_request(self.http(), &args, &self.base_url(), None::<NoMod>).await?;
        Ok(())
    }

    // ── container_top ───────────────────────────────────────────────────

    /// List processes running inside a container (`GET /containers/{id}/top`).
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn container_top(
        &self,
        id: &str,
        ps_args: Option<&str>,
    ) -> Result<ContainerTopResponse, DockerError> {
        let args = ContainerTopArgs {
            id: id.to_string(),
            ps_args: ps_args.map(str::to_string),
        };
        let response = container_top_request(self.http(), &args, &self.base_url(), None::<NoMod>).await?;
        Ok(response.body)
    }

    // ── container_changes ───────────────────────────────────────────────

    /// Get changes on a container's filesystem (`GET /containers/{id}/changes`).
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn container_changes(
        &self,
        id: &str,
    ) -> Result<serde_json::Value, DockerError> {
        let args = ContainerChangesArgs {
            id: id.to_string(),
        };
        let response = container_changes_request(self.http(), &args, &self.base_url(), None::<NoMod>).await?;
        Ok(response.body)
    }

    // ── container_logs ──────────────────────────────────────────────────

    /// Get container logs (`GET /containers/{id}/logs`).
    ///
    /// WHY: Docker log output is a multiplexed binary stream (`[stream_id; 4 bytes]
    /// [length; 4 bytes] [payload; length bytes] ...`), not JSON. The generated
    /// function parses the body as `serde_json::Value`, so we bypass it and
    /// collect raw bytes directly.
    ///
    /// WHAT: Returns the raw multiplexed log stream bytes.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn container_logs(
        &self,
        id: &str,
        follow: bool,
        stdout: bool,
        stderr: bool,
        since: Option<&str>,
        until: Option<&str>,
        timestamps: bool,
        tail: Option<&str>,
    ) -> Result<Vec<u8>, DockerError> {
        let endpoint_url = format!("{}/containers/{id}/logs", self.base_url());

        let mut builder = PreparedRequestBuilder::get(&endpoint_url)
            .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))?;

        builder = builder.query("follow", Some(follow.to_string()).as_deref());
        builder = builder.query("stdout", Some(stdout.to_string()).as_deref());
        builder = builder.query("stderr", Some(stderr.to_string()).as_deref());
        builder = builder.query("since", since);
        builder = builder.query("until", until);
        builder = builder.query("timestamps", Some(timestamps.to_string()).as_deref());
        builder = builder.query("tail", tail);

        let response = self.http().send_async(builder.build()).await
            .map_err(|e| ApiError::RequestSendFailed(e.to_string()))?;

        let status: usize = response.get_status().into();
        let headers = response.get_headers_ref().clone();
        if status < 200 || status >= 300 {
            return Err(ApiError::HttpStatus { code: status as u16, headers, body: None }.into());
        }
        Ok(collect_bytes_from_send_safe(response.take_body()))
    }

    // ── container_stats ─────────────────────────────────────────────────

    /// Get container stats based on resource usage (`GET /containers/{id}/stats`).
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn container_stats(
        &self,
        id: &str,
        stream: bool,
        one_shot: bool,
    ) -> Result<ContainerStatsResponse, DockerError> {
        let args = ContainerStatsArgs {
            id: id.to_string(),
            stream: Some(stream.to_string()),
            one_shot: Some(one_shot.to_string()),
        };
        let response = container_stats_request(self.http(), &args, &self.base_url(), None::<NoMod>).await?;
        Ok(response.body)
    }

    // ── container_export ────────────────────────────────────────────────

    /// Export a container's filesystem as a tar archive (`GET
    /// /containers/{id}/export`).
    ///
    /// WHY: The generated function returns `ApiResponse<()>` (discarding the
    /// tar stream body). We bypass it to collect raw bytes.
    ///
    /// WHAT: Returns the raw tar stream bytes.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn container_export(&self, id: &str) -> Result<Vec<u8>, DockerError> {
        let endpoint_url = format!("{}/containers/{id}/export", self.base_url());

        let builder = PreparedRequestBuilder::get(&endpoint_url)
            .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))?;

        let response = self.http().send_async(builder.build()).await
            .map_err(|e| ApiError::RequestSendFailed(e.to_string()))?;

        let status: usize = response.get_status().into();
        let headers = response.get_headers_ref().clone();
        if status < 200 || status >= 300 {
            return Err(ApiError::HttpStatus { code: status as u16, headers, body: None }.into());
        }
        Ok(collect_bytes_from_send_safe(response.take_body()))
    }

    // ── container_archive ───────────────────────────────────────────────

    /// Get an archive of a file or directory in a container (`GET
    /// /containers/{id}/archive`).
    ///
    /// WHY: The generated function returns `ApiResponse<()>` (discarding the
    /// tar stream body). We bypass it to collect raw bytes.
    ///
    /// WHAT: Returns the raw tar archive bytes.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn container_archive(&self, id: &str, path: &str) -> Result<Vec<u8>, DockerError> {
        let endpoint_url = format!("{}/containers/{id}/archive", self.base_url());

        let mut builder = PreparedRequestBuilder::get(&endpoint_url)
            .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))?;

        builder = builder.query("path", Some(path));

        let response = self.http().send_async(builder.build()).await
            .map_err(|e| ApiError::RequestSendFailed(e.to_string()))?;

        let status: usize = response.get_status().into();
        let headers = response.get_headers_ref().clone();
        if status < 200 || status >= 300 {
            return Err(ApiError::HttpStatus { code: status as u16, headers, body: None }.into());
        }
        Ok(collect_bytes_from_send_safe(response.take_body()))
    }

    // ── container_attach ────────────────────────────────────────────────

    /// Attach to a container's stdio (`POST /containers/{id}/attach`).
    ///
    /// WHY: The generated function returns `ApiResponse<()>` (discarding the
    /// multiplexed stream body). We bypass it to collect raw bytes. The upgrade
    /// to WebSocket for interactive sessions is handled via the `ws` module.
    ///
    /// WHAT: Returns the raw multiplexed stream bytes.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn container_attach(
        &self,
        id: &str,
        detach_keys: Option<&str>,
        logs: bool,
        stream: bool,
        stdin: bool,
        stdout: bool,
        stderr: bool,
    ) -> Result<Vec<u8>, DockerError> {
        let endpoint_url = format!("{}/containers/{id}/attach", self.base_url());

        let mut builder = PreparedRequestBuilder::post(&endpoint_url)
            .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))?;

        builder = builder.query("detachKeys", detach_keys);
        builder = builder.query("logs", Some(logs.to_string()).as_deref());
        builder = builder.query("stream", Some(stream.to_string()).as_deref());
        builder = builder.query("stdin", Some(stdin.to_string()).as_deref());
        builder = builder.query("stdout", Some(stdout.to_string()).as_deref());
        builder = builder.query("stderr", Some(stderr.to_string()).as_deref());

        let response = self.http().send_async(builder.build()).await
            .map_err(|e| ApiError::RequestSendFailed(e.to_string()))?;

        let status: usize = response.get_status().into();
        let headers = response.get_headers_ref().clone();
        if status < 200 || status >= 300 {
            return Err(ApiError::HttpStatus { code: status as u16, headers, body: None }.into());
        }
        Ok(collect_bytes_from_send_safe(response.take_body()))
    }

    // ── container_list ──────────────────────────────────────────────────

    /// List containers (`GET /containers/json`).
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn container_list(
        &self,
        all: bool,
        limit: Option<u32>,
        size: bool,
        filters: Option<&str>,
    ) -> Result<serde_json::Value, DockerError> {
        let args = ContainerListArgs {
            all: Some(all.to_string()),
            limit: limit.map(|v| v.to_string()),
            size: Some(size.to_string()),
            filters: filters.map(str::to_string),
        };
        let response = container_list_request(self.http(), &args, &self.base_url(), None::<NoMod>).await?;
        Ok(response.body)
    }

    // ── container_prune ─────────────────────────────────────────────────

    /// Delete stopped containers (`POST /containers/prune`).
    ///
    /// WHY: The generated function returns `ApiResponse<()>` — the prune
    /// response body (deleted IDs + space reclaimed) is not yet modelled as a
    /// typed struct by the generator.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn container_prune(
        &self,
        filters: Option<&str>,
    ) -> Result<(), DockerError> {
        let args = ContainerPruneArgs {
            filters: filters.map(str::to_string),
        };
        container_prune_request(self.http(), &args, &self.base_url(), None::<NoMod>).await?;
        Ok(())
    }

    // ── update_container ────────────────────────────────────────────────

    /// Update a container's resource-limit configuration (`POST
    /// /containers/{id}/update`).
    ///
    /// WHY: Matches bollard's `update_container` — lets callers change CPU,
    /// memory, restart-policy and other `HostConfig`-style limits on a running
    /// container without recreating it. The generated function models the
    /// request body only via `builder_mod`, so we inject the JSON config here.
    ///
    /// WHAT: `config` is the Docker `ContainerUpdate` body (a `HostConfig`
    /// subset plus `RestartPolicy`). Returns the `ContainerUpdateResponse`
    /// (any warnings emitted by the daemon).
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn update_container(
        &self,
        id: &str,
        config: serde_json::Value,
    ) -> Result<ContainerUpdateResponse, DockerError> {
        let args = ContainerUpdateArgs { id: id.to_string() };
        let response = container_update_request(
            self.http(),
            &args,
            &self.base_url(),
            Some(move |b: &mut PreparedRequestBuilder| {
                let _ = b.set_body_json(&config);
            }),
        )
        .await?;
        Ok(response.body)
    }

    // ── upload_to_container ─────────────────────────────────────────────

    /// Upload a tar archive of files into a container (`PUT
    /// /containers/{id}/archive`).
    ///
    /// WHY: Matches bollard's `upload_to_container` — the inverse of
    /// [`Self::container_archive`]. The generated `put_container_archive`
    /// function only wires query parameters (it never sets the request body),
    /// so we build the request directly to attach the raw tar bytes with the
    /// `application/x-tar` content type Docker requires.
    ///
    /// WHAT: `tar` is a raw (optionally gzip/bzip2/xz-compressed) tar stream.
    /// `path` is the destination directory inside the container. When
    /// `no_overwrite_dir_non_dir` is set, Docker refuses to replace a
    /// directory with a non-directory (and vice-versa); `copy_uidgid` copies
    /// the archive's UID/GID instead of remapping to the container user.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn upload_to_container(
        &self,
        id: &str,
        path: &str,
        tar: Vec<u8>,
        no_overwrite_dir_non_dir: Option<bool>,
        copy_uidgid: Option<bool>,
    ) -> Result<(), DockerError> {
        let endpoint_url = format!("{}/containers/{id}/archive", self.base_url());

        let mut builder = PreparedRequestBuilder::put(&endpoint_url)
            .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))?;

        builder = builder.query("path", Some(path));
        builder = builder.query(
            "noOverwriteDirNonDir",
            no_overwrite_dir_non_dir.map(|v| v.to_string()).as_deref(),
        );
        builder = builder.query("copyUIDGID", copy_uidgid.map(|v| v.to_string()).as_deref());
        builder = builder.header(SimpleHeader::custom("Content-Type"), "application/x-tar");
        builder = builder.body_bytes(tar);

        let response = self.http().send_async(builder.build()).await
            .map_err(|e| ApiError::RequestSendFailed(e.to_string()))?;

        let status: usize = response.get_status().into();
        let headers = response.get_headers_ref().clone();
        if status < 200 || status >= 300 {
            return Err(ApiError::HttpStatus { code: status as u16, headers, body: None }.into());
        }
        Ok(())
    }

    // ── get_container_archive_info ──────────────────────────────────────

    /// Retrieve metadata about a path inside a container (`HEAD
    /// /containers/{id}/archive`).
    ///
    /// WHY: Matches bollard's `get_container_archive_info` — the cheap `HEAD`
    /// companion to [`Self::container_archive`]. Docker returns the stat as a
    /// base64-encoded JSON blob in the `X-Docker-Container-Path-Stat` header
    /// rather than the response body.
    ///
    /// WHAT: Returns the raw (still base64-encoded) header value, or an error
    /// if the path does not exist. Callers decode it into a `FileInfo`
    /// (`name`, `size`, `mode`, `mtime`, `linkTarget`).
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure, non-2xx status, or a
    /// missing stat header.
    pub async fn get_container_archive_info(
        &self,
        id: &str,
        path: &str,
    ) -> Result<String, DockerError> {
        let endpoint_url = format!("{}/containers/{id}/archive", self.base_url());

        let mut builder = PreparedRequestBuilder::head(&endpoint_url)
            .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))?;
        builder = builder.query("path", Some(path));

        let response = self.http().send_async(builder.build()).await
            .map_err(|e| ApiError::RequestSendFailed(e.to_string()))?;

        let status: usize = response.get_status().into();
        let headers = response.get_headers_ref().clone();
        if status < 200 || status >= 300 {
            return Err(ApiError::HttpStatus { code: status as u16, headers, body: None }.into());
        }
        // Docker returns the stat blob in `X-Docker-Container-Path-Stat`.
        // HTTP header names are case-insensitive and the transport preserves
        // the daemon's original casing, so scan case-insensitively rather than
        // assuming an exact key.
        let stat = headers
            .iter()
            .find(|(k, _)| k.to_string().eq_ignore_ascii_case("x-docker-container-path-stat"))
            .and_then(|(_, v)| v.first())
            .cloned();
        stat.ok_or_else(|| {
            ApiError::RequestSendFailed("missing X-Docker-Container-Path-Stat header".to_string())
                .into()
        })
    }
}
