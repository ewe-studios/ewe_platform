//! Network operations for `DockerClient` — list, inspect, create,
//! delete, connect, disconnect, prune.
//!
//! WHY: Wrappers over the generated async `*_request` functions for Docker
//! network operations.
//!
//! WHAT: Methods on [`DockerClient`] that call the generated network endpoints.
//!
//! HOW: Each method constructs the appropriate Args struct and delegates to
//! the generated function.

use foundation_netio::shared::http::SimpleHeader;
use foundation_netio::PreparedRequestBuilder;

use crate::error::DockerError;
use crate::generated::connect::{network_connect_request, NetworkConnectArgs};
use crate::generated::create::{network_create_request, NetworkCreateArgs, NetworkCreateResponse};
use crate::generated::disconnect::{network_disconnect_request, NetworkDisconnectArgs};
use crate::generated::networks::{
    network_delete_request, network_inspect_request, network_list_request, NetworkDeleteArgs,
    NetworkInspect, NetworkInspectArgs, NetworkListArgs,
};
use crate::generated::prune::{network_prune_request, NetworkPruneArgs};

use super::{DockerClient, NoMod};

impl DockerClient {
    /// List networks (`GET /networks`).
    ///
    /// WHY: Returns all networks visible to the daemon, optionally filtered by
    /// driver, name, type, etc.
    ///
    /// WHAT: Calls `network_list_request` and returns the raw JSON body.
    ///
    /// HOW: `GET /networks?filters=...`.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn network_list(&self, filters: Option<&str>) -> Result<serde_json::Value, DockerError> {
        let args = NetworkListArgs {
            filters: filters.map(str::to_string),
        };
        let response = network_list_request(self.http(), &args, None::<NoMod>).await?;
        Ok(response.body)
    }

    /// Inspect a network (`GET /networks/{id}`).
    ///
    /// WHY: Returns low-level information about a network (IPAM config, connected
    /// containers, driver options).
    ///
    /// WHAT: Calls `network_inspect_request` with optional `verbose` and `scope`
    /// query parameters.
    ///
    /// HOW: `GET /networks/{id}?verbose=...&scope=...`.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn network_inspect(
        &self,
        id: &str,
        verbose: Option<bool>,
        scope: Option<&str>,
    ) -> Result<NetworkInspect, DockerError> {
        let args = NetworkInspectArgs {
            id: id.to_string(),
            verbose: verbose.map(|v| v.to_string()),
            scope: scope.map(str::to_string),
        };
        let response = network_inspect_request(self.http(), &args, None::<NoMod>).await?;
        Ok(response.body)
    }

    /// Delete a network (`DELETE /networks/{id}`).
    ///
    /// WHY: Removes a user-defined network by name or id.
    ///
    /// WHAT: Calls `network_delete_request`.
    ///
    /// HOW: `DELETE /networks/{id}`.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status (e.g. 404
    /// if the network does not exist, or 409 if it is still in use).
    pub async fn network_delete(&self, id: &str) -> Result<(), DockerError> {
        let args = NetworkDeleteArgs {
            id: id.to_string(),
        };
        network_delete_request(self.http(), &args, None::<NoMod>).await?;
        Ok(())
    }

    /// Create a network (`POST /networks/create`).
    ///
    /// WHY: The generated `NetworkCreateArgs` is empty (the request body was an
    /// inline `allOf` the generator did not name), so the network config is
    /// injected through the request builder.
    ///
    /// WHAT: Serializes `config` as the JSON body and returns the created network
    /// id and optional warning.
    ///
    /// HOW: `network_create_request` with a `builder_mod` that sets the JSON body.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn network_create(
        &self,
        config: serde_json::Value,
    ) -> Result<NetworkCreateResponse, DockerError> {
        let args = NetworkCreateArgs::default();
        let response = network_create_request(
            self.http(),
            &args,
            Some(move |b: &mut PreparedRequestBuilder| {
                b.set_header(SimpleHeader::CONTENT_TYPE, "application/json");
                let _ = b.set_body_json(&config);
            }),
        )
        .await?;
        Ok(response.body)
    }

    /// Connect a container to a network (`POST /networks/{id}/connect`).
    ///
    /// WHY: Attaches a container to a network, optionally with endpoint
    /// configuration (IP address, aliases, links).
    ///
    /// WHAT: Sends a `{"Container": "<container_id>"}` body.
    ///
    /// HOW: `network_connect_request` with a `builder_mod` that sets the JSON body.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn network_connect(&self, id: &str, container_id: &str) -> Result<(), DockerError> {
        let args = NetworkConnectArgs {
            id: id.to_string(),
            body: Default::default(),
        };
        let body = serde_json::json!({"Container": container_id});
        network_connect_request(
            self.http(),
            &args,
            Some(move |b: &mut PreparedRequestBuilder| {
                b.set_header(SimpleHeader::CONTENT_TYPE, "application/json");
                let _ = b.set_body_json(&body);
            }),
        )
        .await?;
        Ok(())
    }

    /// Disconnect a container from a network (`POST /networks/{id}/disconnect`).
    ///
    /// WHY: Detaches a container from a network, optionally forcing the
    /// disconnection.
    ///
    /// WHAT: Sends a `{"Container": "<container_id>", "Force": <bool>}` body.
    ///
    /// HOW: `network_disconnect_request` with a `builder_mod` that sets the JSON
    /// body.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn network_disconnect(
        &self,
        id: &str,
        container_id: &str,
        force: bool,
    ) -> Result<(), DockerError> {
        let args = NetworkDisconnectArgs {
            id: id.to_string(),
            body: Default::default(),
        };
        let body = serde_json::json!({"Container": container_id, "Force": force});
        network_disconnect_request(
            self.http(),
            &args,
            Some(move |b: &mut PreparedRequestBuilder| {
                b.set_header(SimpleHeader::CONTENT_TYPE, "application/json");
                let _ = b.set_body_json(&body);
            }),
        )
        .await?;
        Ok(())
    }

    /// Prune unused networks (`POST /networks/prune`).
    ///
    /// WHY: Deletes all networks not currently in use by any container.
    ///
    /// WHAT: Calls `network_prune_request` with optional filters.
    ///
    /// HOW: `POST /networks/prune?filters=...`.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn network_prune(&self, filters: Option<&str>) -> Result<(), DockerError> {
        let args = NetworkPruneArgs {
            filters: filters.map(str::to_string),
        };
        network_prune_request(self.http(), &args, None::<NoMod>).await?;
        Ok(())
    }
}
