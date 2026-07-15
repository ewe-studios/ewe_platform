//! Volume operations for `DockerClient` — list, inspect, create,
//! delete, update, prune.
//!
//! WHY: Wrappers over the generated async `*_request` functions for Docker
//! volume operations.
//!
//! WHAT: Methods on [`DockerClient`] that call the generated volume endpoints.
//!
//! HOW: Each method constructs the appropriate Args struct and delegates to
//! the generated function.

use foundation_netio::shared::http::SimpleHeader;
use foundation_netio::PreparedRequestBuilder;

use crate::error::DockerError;
use crate::generated::create::{volume_create_request, VolumeCreateArgs};
use crate::generated::prune::{volume_prune_request, VolumePruneArgs};
use crate::generated::shared::Volume;
use crate::generated::volumes::{
    volume_delete_request, volume_inspect_request, volume_list_request, volume_update_request,
    VolumeDeleteArgs, VolumeInspectArgs, VolumeListArgs, VolumeListResponse, VolumeUpdateArgs,
};

use super::{DockerClient, NoMod};

impl DockerClient {
    /// List volumes (`GET /volumes`).
    ///
    /// WHY: Returns all volumes known to the daemon, optionally filtered by
    /// driver, name, dangling status, etc.
    ///
    /// WHAT: Calls `volume_list_request` and returns the response with the
    /// `Volumes` array and optional `Warnings`.
    ///
    /// HOW: `GET /volumes?filters=...`.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn volume_list(
        &self,
        filters: Option<&str>,
    ) -> Result<VolumeListResponse, DockerError> {
        let args = VolumeListArgs {
            filters: filters.map(str::to_string),
        };
        let response = volume_list_request(self.http(), &args, &self.base_url(), None::<NoMod>).await?;
        Ok(response.body)
    }

    /// Inspect a volume (`GET /volumes/{name}`).
    ///
    /// WHY: Returns low-level information about a volume (driver, mountpoint,
    /// labels, scope, status).
    ///
    /// WHAT: Calls `volume_inspect_request`.
    ///
    /// HOW: `GET /volumes/{name}`.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn volume_inspect(&self, name: &str) -> Result<Volume, DockerError> {
        let args = VolumeInspectArgs {
            name: name.to_string(),
        };
        let response = volume_inspect_request(self.http(), &args, &self.base_url(), None::<NoMod>).await?;
        Ok(response.body)
    }

    /// Create a volume (`POST /volumes/create`).
    ///
    /// WHY: The generated `VolumeCreateArgs` carries the body as
    /// `VolumeCreateRequest`, but we use `builder_mod` to inject the config as
    /// `serde_json::Value` for consistency with the `create_container` /
    /// `network_create` patterns.
    ///
    /// WHAT: Serializes `config` as the JSON body and returns the created volume.
    ///
    /// HOW: `volume_create_request` with a `builder_mod` that sets the JSON body.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn volume_create(&self, config: serde_json::Value) -> Result<Volume, DockerError> {
        let args = VolumeCreateArgs {
            body: Default::default(),
        };
        let response = volume_create_request(
            self.http(),
            &args,
            &self.base_url(),
            Some(move |b: &mut PreparedRequestBuilder| {
                b.set_header(SimpleHeader::CONTENT_TYPE, "application/json");
                let _ = b.set_body_json(&config);
            }),
        )
        .await?;
        Ok(response.body)
    }

    /// Delete a volume (`DELETE /volumes/{name}`).
    ///
    /// WHY: Removes a volume by name, optionally forcing removal even if the
    /// volume is in use.
    ///
    /// WHAT: Calls `volume_delete_request`.
    ///
    /// HOW: `DELETE /volumes/{name}?force=...`.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status (e.g. 404
    /// if the volume does not exist, or 409 if it is still in use and force is
    /// not set).
    pub async fn volume_delete(
        &self,
        name: &str,
        force: Option<bool>,
    ) -> Result<(), DockerError> {
        let args = VolumeDeleteArgs {
            name: name.to_string(),
            force: force.map(|f| f.to_string()),
        };
        volume_delete_request(self.http(), &args, &self.base_url(), None::<NoMod>).await?;
        Ok(())
    }

    /// Update a volume (`PUT /volumes/{name}`).
    ///
    /// WHY: Updates a volume's configuration (ClusterVolumeSpec).
    ///
    /// WHAT: Sends the volume spec as the JSON body.
    ///
    /// HOW: `volume_update_request` with a `builder_mod` that sets the JSON body.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn volume_update(
        &self,
        name: &str,
        version: Option<i64>,
        config: serde_json::Value,
    ) -> Result<(), DockerError> {
        let args = VolumeUpdateArgs {
            name: name.to_string(),
            version: version.map(|v| v.to_string()),
        };
        volume_update_request(
            self.http(),
            &args,
            &self.base_url(),
            Some(move |b: &mut PreparedRequestBuilder| {
                b.set_header(SimpleHeader::CONTENT_TYPE, "application/json");
                let _ = b.set_body_json(&config);
            }),
        )
        .await?;
        Ok(())
    }

    /// Prune unused volumes (`POST /volumes/prune`).
    ///
    /// WHY: Deletes all volumes not currently in use by any container.
    ///
    /// WHAT: Calls `volume_prune_request` with optional filters.
    ///
    /// HOW: `POST /volumes/prune?filters=...`.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn volume_prune(&self, filters: Option<&str>) -> Result<(), DockerError> {
        let args = VolumePruneArgs {
            filters: filters.map(str::to_string),
        };
        volume_prune_request(self.http(), &args, &self.base_url(), None::<NoMod>).await?;
        Ok(())
    }
}
