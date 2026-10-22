//! Container lifecycle operations for [`super::DockerClient`] — restart, kill,
//! rename, resize, top, changes, logs, stats, export, archive, attach, list, prune.
//!
//! WHY: These are the remaining container endpoints from the Docker Engine API
//! (v1.53) that complete the container lifecycle surface. Binary-stream endpoints
//! (logs, export, archive, attach) and live stats return their body unread, as
//! an async stream: a followed log or a stats feed never ends, and a
//! container's filesystem can be gigabytes.
//!
//! WHAT: One `impl DockerClient` block with a method per endpoint, following the
//! same pattern as `client/mod.rs` — construct the generated `Args` struct, call
//! into the transport, return the body.
//!
//! HOW: State-mutating commands (restart, kill, rename, resize, prune) use the
//! generated async functions that return `ApiResponse<()>`. Query endpoints (top,
//! changes, list) use the generated functions and return the deserialized
//! body. Streaming endpoints use the generated functions too, which check the
//! status and hand back the body unread; it is wrapped in a [`LogStream`]
//! (multiplexed stdout/stderr), a [`ByteBody`] (tar archives) or a
//! [`StatsStream`] (one stats sample per line; a single sample is the first
//! line of a `stream=false` answer).

use foundation_netio::shared::http::SimpleHeader;
use foundation_netio::PreparedRequestBuilder;

use crate::client::body::{ByteBody, LogStream, StatsStream};
use crate::error::DockerError;
use crate::generated::archive::{container_archive_request, ContainerArchiveArgs};
use crate::generated::attach::{container_attach_request, ContainerAttachArgs};
use crate::generated::export::{container_export_request, ContainerExportArgs};
use crate::generated::logs::{container_logs_request, ContainerLogsArgs};
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

/// What [`DockerClient::container_logs`] asks for (`GET
/// /containers/{id}/logs` query parameters).
///
/// The default reads everything the container has written so far, stdout
/// and stderr, without following — the daemon rejects a request that asks for
/// neither stream.
#[derive(Debug, Clone)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "one field per boolean query parameter of the endpoint"
)]
pub struct ContainerLogsOptions {
    /// Keep the stream open and send new output as the container writes it
    /// (`follow`). The stream then ends only when the container stops or the
    /// caller drops it.
    pub follow: bool,
    /// Include stdout (`stdout`).
    pub stdout: bool,
    /// Include stderr (`stderr`).
    pub stderr: bool,
    /// Only output written at or after this UNIX timestamp (`since`).
    pub since: Option<String>,
    /// Only output written before this UNIX timestamp (`until`).
    pub until: Option<String>,
    /// Prefix every line with its timestamp (`timestamps`).
    pub timestamps: bool,
    /// Only this many lines from the end (`tail`); `None` is all of them.
    pub tail: Option<String>,
}

impl Default for ContainerLogsOptions {
    fn default() -> Self {
        Self {
            follow: false,
            stdout: true,
            stderr: true,
            since: None,
            until: None,
            timestamps: false,
            tail: None,
        }
    }
}

/// What [`DockerClient::container_attach`] asks for (`POST
/// /containers/{id}/attach` query parameters).
///
/// The default attaches to the live stdout and stderr, as `docker attach
/// --no-stdin` does: no earlier output, no stdin.
#[derive(Debug, Clone)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "one field per boolean query parameter of the endpoint"
)]
pub struct ContainerAttachOptions {
    /// The key sequence that detaches from the container (`detachKeys`);
    /// `None` keeps the daemon's default.
    pub detach_keys: Option<String>,
    /// Replay the output written before attaching (`logs`).
    pub logs: bool,
    /// Stream output as the container writes it (`stream`).
    pub stream: bool,
    /// Attach stdin (`stdin`).
    pub stdin: bool,
    /// Attach stdout (`stdout`).
    pub stdout: bool,
    /// Attach stderr (`stderr`).
    pub stderr: bool,
}

impl Default for ContainerAttachOptions {
    fn default() -> Self {
        Self {
            detach_keys: None,
            logs: false,
            stream: true,
            stdin: false,
            stdout: true,
            stderr: true,
        }
    }
}

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
        container_restart_request(self.api(), &args, &self.api_url().await?, None::<NoMod>).await?;
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
        container_kill_request(self.api(), &args, &self.api_url().await?, None::<NoMod>).await?;
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
        container_rename_request(self.api(), &args, &self.api_url().await?, None::<NoMod>).await?;
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
        container_resize_request(self.api(), &args, &self.api_url().await?, None::<NoMod>).await?;
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
        let response = container_top_request(self.api(), &args, &self.api_url().await?, None::<NoMod>).await?;
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
        let response = container_changes_request(self.api(), &args, &self.api_url().await?, None::<NoMod>).await?;
        Ok(response.body)
    }

    // ── container_logs ──────────────────────────────────────────────────

    /// Get a container's logs (`GET /containers/{id}/logs`).
    ///
    /// WHY: With `follow`, the daemon keeps the stream open for as long as the
    /// container runs; even without it, a chatty container's log is unbounded.
    ///
    /// WHAT: Returns the log unread, as a [`LogStream`] of demultiplexed
    /// stdout/stderr frames (for a container without a TTY).
    ///
    /// [`ContainerLogsOptions::default`] reads stdout and stderr written so
    /// far, without following.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status (checked
    /// before the body is read); read errors come out of the stream.
    pub async fn container_logs(
        &self,
        id: &str,
        opts: &ContainerLogsOptions,
    ) -> Result<LogStream, DockerError> {
        let args = ContainerLogsArgs {
            id: id.to_string(),
            follow: Some(opts.follow.to_string()),
            stdout: Some(opts.stdout.to_string()),
            stderr: Some(opts.stderr.to_string()),
            since: opts.since.clone(),
            until: opts.until.clone(),
            timestamps: Some(opts.timestamps.to_string()),
            tail: opts.tail.clone(),
        };
        let response =
            container_logs_request(self.api(), &args, &self.api_url().await?, None::<NoMod>).await?;
        Ok(LogStream::new(response.body, self.limits().max_log_frame))
    }

    // ── container_stats ─────────────────────────────────────────────────

    /// One resource-usage sample of a container (`GET
    /// /containers/{id}/stats?stream=false`).
    ///
    /// WHAT: Asks for a single sample and reads the one line the daemon
    /// answers with. With `one_shot`, the daemon does not wait for a second
    /// CPU sample, so the answer comes back at once but `precpu_stats` is
    /// empty (no CPU percentage can be computed from it); without it the
    /// daemon takes about a second to fill `precpu_stats`. For a live feed use
    /// [`Self::container_stats_stream`].
    ///
    /// # Errors
    ///
    /// [`DockerError::Api`] for a non-2xx status (checked before the body is
    /// read), [`DockerError::Transport`] if sending or reading fails, and
    /// [`DockerError::JsonParse`] if the line is not a stats object or the
    /// daemon sends no line at all.
    pub async fn container_stats(
        &self,
        id: &str,
        one_shot: bool,
    ) -> Result<ContainerStatsResponse, DockerError> {
        let mut stats = self.send_container_stats(id, false, one_shot).await?;
        match stats.next().await {
            Some(sample) => sample,
            None => Err(DockerError::JsonParse(format!(
                "container {id} stats: the daemon answered with an empty body"
            ))),
        }
    }

    /// A container's live resource usage (`GET
    /// /containers/{id}/stats?stream=true`), one sample per line.
    ///
    /// WHAT: Returns the feed unread, as a [`StatsStream`] of the generated
    /// [`ContainerStatsResponse`]. The daemon writes a sample about once a
    /// second for as long as the container runs; the stream ends only when the
    /// caller drops it (which closes the connection) or the daemon closes the
    /// feed.
    ///
    /// There is no `one_shot` here: the API allows `one-shot` only with
    /// `stream=false`, and dockerd rejects the pair with a `400`.
    ///
    /// # Errors
    ///
    /// [`DockerError::Api`] for a non-2xx status (checked before the body is
    /// read) or [`DockerError::Transport`] if sending fails; malformed samples
    /// and read errors come out of the stream.
    pub async fn container_stats_stream(&self, id: &str) -> Result<StatsStream, DockerError> {
        self.send_container_stats(id, true, false).await
    }

    /// Send `GET /containers/{id}/stats` and hand back its body unread.
    async fn send_container_stats(
        &self,
        id: &str,
        stream: bool,
        one_shot: bool,
    ) -> Result<StatsStream, DockerError> {
        let args = ContainerStatsArgs {
            id: id.to_string(),
            stream: Some(stream.to_string()),
            one_shot: Some(one_shot.to_string()),
        };
        let response =
            container_stats_request(self.api(), &args, &self.api_url().await?, None::<NoMod>).await?;
        Ok(StatsStream::new(response.body))
    }

    // ── container_export ────────────────────────────────────────────────

    /// Export a container's filesystem as a tar archive (`GET
    /// /containers/{id}/export`).
    ///
    /// WHAT: Returns the archive unread, as a [`ByteBody`] of chunks — write it
    /// where it is going chunk by chunk; a filesystem can be gigabytes.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status (checked
    /// before the body is read); read errors come out of the stream.
    pub async fn container_export(&self, id: &str) -> Result<ByteBody, DockerError> {
        let args = ContainerExportArgs { id: id.to_string() };
        let response =
            container_export_request(self.api(), &args, &self.api_url().await?, None::<NoMod>).await?;
        Ok(ByteBody::new(response.body))
    }

    // ── container_archive ───────────────────────────────────────────────

    /// Get an archive of a file or directory in a container (`GET
    /// /containers/{id}/archive`).
    ///
    /// WHAT: Returns the tar archive unread, as a [`ByteBody`] of chunks.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status (checked
    /// before the body is read); read errors come out of the stream.
    pub async fn container_archive(&self, id: &str, path: &str) -> Result<ByteBody, DockerError> {
        let args = ContainerArchiveArgs {
            id: id.to_string(),
            path: Some(path.to_string()),
        };
        let response =
            container_archive_request(self.api(), &args, &self.api_url().await?, None::<NoMod>).await?;
        Ok(ByteBody::new(response.body))
    }

    // ── container_attach ────────────────────────────────────────────────

    /// Attach to a container's stdio (`POST /containers/{id}/attach`).
    ///
    /// WHY: With `stream`, the daemon keeps this open for as long as the
    /// container runs. The upgrade to WebSocket for interactive sessions is
    /// handled via the `ws` module.
    ///
    /// WHAT: Returns the output unread, as a [`LogStream`] of demultiplexed
    /// frames (for a container without a TTY).
    ///
    /// [`ContainerAttachOptions::default`] attaches to the live stdout and
    /// stderr.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status (checked
    /// before the body is read); read errors come out of the stream.
    pub async fn container_attach(
        &self,
        id: &str,
        opts: &ContainerAttachOptions,
    ) -> Result<LogStream, DockerError> {
        let args = ContainerAttachArgs {
            id: id.to_string(),
            detach_keys: opts.detach_keys.clone(),
            logs: Some(opts.logs.to_string()),
            stream: Some(opts.stream.to_string()),
            stdin: Some(opts.stdin.to_string()),
            stdout: Some(opts.stdout.to_string()),
            stderr: Some(opts.stderr.to_string()),
        };
        let response =
            container_attach_request(self.api(), &args, &self.api_url().await?, None::<NoMod>).await?;
        Ok(LogStream::new(response.body, self.limits().max_log_frame))
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
        let response = container_list_request(self.api(), &args, &self.api_url().await?, None::<NoMod>).await?;
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
        container_prune_request(self.api(), &args, &self.api_url().await?, None::<NoMod>).await?;
        Ok(())
    }

    // ── update_container ────────────────────────────────────────────────

    /// Update a container's resource-limit configuration (`POST
    /// /containers/{id}/update`).
    ///
    /// WHY: Matches bollard's `update_container` — lets callers change CPU,
    /// memory, restart-policy and other `HostConfig`-style limits on a running
    /// container without recreating it.
    ///
    /// WHAT: `config` is the Docker `ContainerUpdate` body (a `HostConfig`
    /// subset plus `RestartPolicy`), read into the generated
    /// `ContainerUpdateRequest`. Returns the `ContainerUpdateResponse` (any
    /// warnings emitted by the daemon).
    ///
    /// # Errors
    ///
    /// Returns [`DockerError::JsonParse`] if `config` does not fit the
    /// `ContainerUpdateRequest` schema, and [`DockerError`] on transport
    /// failure or non-2xx status.
    pub async fn update_container(
        &self,
        id: &str,
        config: serde_json::Value,
    ) -> Result<ContainerUpdateResponse, DockerError> {
        let args = ContainerUpdateArgs {
            id: id.to_string(),
            body: super::generated_body("container update", config)?,
        };
        let response =
            container_update_request(self.api(), &args, &self.api_url().await?, None::<NoMod>).await?;
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
        let endpoint_url = format!("{}/containers/{id}/archive", self.api_url().await?);

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
        let endpoint_url = format!("{}/containers/{id}/archive", self.api_url().await?);

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
