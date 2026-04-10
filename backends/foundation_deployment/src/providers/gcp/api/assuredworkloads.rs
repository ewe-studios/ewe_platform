//! AssuredworkloadsProvider - State-aware assuredworkloads API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       assuredworkloads API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::assuredworkloads::{
    assuredworkloads_organizations_locations_operations_get_builder, assuredworkloads_organizations_locations_operations_get_task,
    assuredworkloads_organizations_locations_operations_list_builder, assuredworkloads_organizations_locations_operations_list_task,
    assuredworkloads_organizations_locations_workloads_analyze_workload_move_builder, assuredworkloads_organizations_locations_workloads_analyze_workload_move_task,
    assuredworkloads_organizations_locations_workloads_create_builder, assuredworkloads_organizations_locations_workloads_create_task,
    assuredworkloads_organizations_locations_workloads_delete_builder, assuredworkloads_organizations_locations_workloads_delete_task,
    assuredworkloads_organizations_locations_workloads_enable_compliance_updates_builder, assuredworkloads_organizations_locations_workloads_enable_compliance_updates_task,
    assuredworkloads_organizations_locations_workloads_enable_resource_monitoring_builder, assuredworkloads_organizations_locations_workloads_enable_resource_monitoring_task,
    assuredworkloads_organizations_locations_workloads_get_builder, assuredworkloads_organizations_locations_workloads_get_task,
    assuredworkloads_organizations_locations_workloads_list_builder, assuredworkloads_organizations_locations_workloads_list_task,
    assuredworkloads_organizations_locations_workloads_mutate_partner_permissions_builder, assuredworkloads_organizations_locations_workloads_mutate_partner_permissions_task,
    assuredworkloads_organizations_locations_workloads_patch_builder, assuredworkloads_organizations_locations_workloads_patch_task,
    assuredworkloads_organizations_locations_workloads_restrict_allowed_resources_builder, assuredworkloads_organizations_locations_workloads_restrict_allowed_resources_task,
    assuredworkloads_organizations_locations_workloads_updates_apply_builder, assuredworkloads_organizations_locations_workloads_updates_apply_task,
    assuredworkloads_organizations_locations_workloads_updates_list_builder, assuredworkloads_organizations_locations_workloads_updates_list_task,
    assuredworkloads_organizations_locations_workloads_violations_acknowledge_builder, assuredworkloads_organizations_locations_workloads_violations_acknowledge_task,
    assuredworkloads_organizations_locations_workloads_violations_get_builder, assuredworkloads_organizations_locations_workloads_violations_get_task,
    assuredworkloads_organizations_locations_workloads_violations_list_builder, assuredworkloads_organizations_locations_workloads_violations_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::assuredworkloads::GoogleCloudAssuredworkloadsV1AcknowledgeViolationResponse;
use crate::providers::gcp::clients::assuredworkloads::GoogleCloudAssuredworkloadsV1AnalyzeWorkloadMoveResponse;
use crate::providers::gcp::clients::assuredworkloads::GoogleCloudAssuredworkloadsV1EnableComplianceUpdatesResponse;
use crate::providers::gcp::clients::assuredworkloads::GoogleCloudAssuredworkloadsV1EnableResourceMonitoringResponse;
use crate::providers::gcp::clients::assuredworkloads::GoogleCloudAssuredworkloadsV1ListViolationsResponse;
use crate::providers::gcp::clients::assuredworkloads::GoogleCloudAssuredworkloadsV1ListWorkloadUpdatesResponse;
use crate::providers::gcp::clients::assuredworkloads::GoogleCloudAssuredworkloadsV1ListWorkloadsResponse;
use crate::providers::gcp::clients::assuredworkloads::GoogleCloudAssuredworkloadsV1RestrictAllowedResourcesResponse;
use crate::providers::gcp::clients::assuredworkloads::GoogleCloudAssuredworkloadsV1Violation;
use crate::providers::gcp::clients::assuredworkloads::GoogleCloudAssuredworkloadsV1Workload;
use crate::providers::gcp::clients::assuredworkloads::GoogleLongrunningListOperationsResponse;
use crate::providers::gcp::clients::assuredworkloads::GoogleLongrunningOperation;
use crate::providers::gcp::clients::assuredworkloads::GoogleProtobufEmpty;
use crate::providers::gcp::clients::assuredworkloads::AssuredworkloadsOrganizationsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::assuredworkloads::AssuredworkloadsOrganizationsLocationsOperationsListArgs;
use crate::providers::gcp::clients::assuredworkloads::AssuredworkloadsOrganizationsLocationsWorkloadsAnalyzeWorkloadMoveArgs;
use crate::providers::gcp::clients::assuredworkloads::AssuredworkloadsOrganizationsLocationsWorkloadsCreateArgs;
use crate::providers::gcp::clients::assuredworkloads::AssuredworkloadsOrganizationsLocationsWorkloadsDeleteArgs;
use crate::providers::gcp::clients::assuredworkloads::AssuredworkloadsOrganizationsLocationsWorkloadsEnableComplianceUpdatesArgs;
use crate::providers::gcp::clients::assuredworkloads::AssuredworkloadsOrganizationsLocationsWorkloadsEnableResourceMonitoringArgs;
use crate::providers::gcp::clients::assuredworkloads::AssuredworkloadsOrganizationsLocationsWorkloadsGetArgs;
use crate::providers::gcp::clients::assuredworkloads::AssuredworkloadsOrganizationsLocationsWorkloadsListArgs;
use crate::providers::gcp::clients::assuredworkloads::AssuredworkloadsOrganizationsLocationsWorkloadsMutatePartnerPermissionsArgs;
use crate::providers::gcp::clients::assuredworkloads::AssuredworkloadsOrganizationsLocationsWorkloadsPatchArgs;
use crate::providers::gcp::clients::assuredworkloads::AssuredworkloadsOrganizationsLocationsWorkloadsRestrictAllowedResourcesArgs;
use crate::providers::gcp::clients::assuredworkloads::AssuredworkloadsOrganizationsLocationsWorkloadsUpdatesApplyArgs;
use crate::providers::gcp::clients::assuredworkloads::AssuredworkloadsOrganizationsLocationsWorkloadsUpdatesListArgs;
use crate::providers::gcp::clients::assuredworkloads::AssuredworkloadsOrganizationsLocationsWorkloadsViolationsAcknowledgeArgs;
use crate::providers::gcp::clients::assuredworkloads::AssuredworkloadsOrganizationsLocationsWorkloadsViolationsGetArgs;
use crate::providers::gcp::clients::assuredworkloads::AssuredworkloadsOrganizationsLocationsWorkloadsViolationsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// AssuredworkloadsProvider with automatic state tracking.
///
/// # Type Parameters
///
/// * `S` - StateStore implementation (FileStateStore, SqliteStateStore, etc.)
///
/// # Example
///
/// ```rust
/// let state_store = FileStateStore::new("/path", "my-project", "dev");
/// let client = ProviderClient::new("my-project", "dev", state_store);
/// let http_client = SimpleHttpClient::new(...);
/// let provider = AssuredworkloadsProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct AssuredworkloadsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> AssuredworkloadsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new AssuredworkloadsProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Assuredworkloads organizations locations operations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn assuredworkloads_organizations_locations_operations_get(
        &self,
        args: &AssuredworkloadsOrganizationsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = assuredworkloads_organizations_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = assuredworkloads_organizations_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Assuredworkloads organizations locations operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningListOperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn assuredworkloads_organizations_locations_operations_list(
        &self,
        args: &AssuredworkloadsOrganizationsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = assuredworkloads_organizations_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = assuredworkloads_organizations_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Assuredworkloads organizations locations workloads analyze workload move.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudAssuredworkloadsV1AnalyzeWorkloadMoveResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn assuredworkloads_organizations_locations_workloads_analyze_workload_move(
        &self,
        args: &AssuredworkloadsOrganizationsLocationsWorkloadsAnalyzeWorkloadMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudAssuredworkloadsV1AnalyzeWorkloadMoveResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = assuredworkloads_organizations_locations_workloads_analyze_workload_move_builder(
            &self.http_client,
            &args.target,
            &args.assetTypes,
            &args.pageSize,
            &args.pageToken,
            &args.project,
        )
        .map_err(ProviderError::Api)?;

        let task = assuredworkloads_organizations_locations_workloads_analyze_workload_move_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Assuredworkloads organizations locations workloads create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn assuredworkloads_organizations_locations_workloads_create(
        &self,
        args: &AssuredworkloadsOrganizationsLocationsWorkloadsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = assuredworkloads_organizations_locations_workloads_create_builder(
            &self.http_client,
            &args.parent,
            &args.externalId,
        )
        .map_err(ProviderError::Api)?;

        let task = assuredworkloads_organizations_locations_workloads_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Assuredworkloads organizations locations workloads delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn assuredworkloads_organizations_locations_workloads_delete(
        &self,
        args: &AssuredworkloadsOrganizationsLocationsWorkloadsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = assuredworkloads_organizations_locations_workloads_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = assuredworkloads_organizations_locations_workloads_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Assuredworkloads organizations locations workloads enable compliance updates.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudAssuredworkloadsV1EnableComplianceUpdatesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn assuredworkloads_organizations_locations_workloads_enable_compliance_updates(
        &self,
        args: &AssuredworkloadsOrganizationsLocationsWorkloadsEnableComplianceUpdatesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudAssuredworkloadsV1EnableComplianceUpdatesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = assuredworkloads_organizations_locations_workloads_enable_compliance_updates_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = assuredworkloads_organizations_locations_workloads_enable_compliance_updates_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Assuredworkloads organizations locations workloads enable resource monitoring.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudAssuredworkloadsV1EnableResourceMonitoringResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn assuredworkloads_organizations_locations_workloads_enable_resource_monitoring(
        &self,
        args: &AssuredworkloadsOrganizationsLocationsWorkloadsEnableResourceMonitoringArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudAssuredworkloadsV1EnableResourceMonitoringResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = assuredworkloads_organizations_locations_workloads_enable_resource_monitoring_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = assuredworkloads_organizations_locations_workloads_enable_resource_monitoring_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Assuredworkloads organizations locations workloads get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudAssuredworkloadsV1Workload result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn assuredworkloads_organizations_locations_workloads_get(
        &self,
        args: &AssuredworkloadsOrganizationsLocationsWorkloadsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudAssuredworkloadsV1Workload, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = assuredworkloads_organizations_locations_workloads_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = assuredworkloads_organizations_locations_workloads_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Assuredworkloads organizations locations workloads list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudAssuredworkloadsV1ListWorkloadsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn assuredworkloads_organizations_locations_workloads_list(
        &self,
        args: &AssuredworkloadsOrganizationsLocationsWorkloadsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudAssuredworkloadsV1ListWorkloadsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = assuredworkloads_organizations_locations_workloads_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = assuredworkloads_organizations_locations_workloads_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Assuredworkloads organizations locations workloads mutate partner permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudAssuredworkloadsV1Workload result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn assuredworkloads_organizations_locations_workloads_mutate_partner_permissions(
        &self,
        args: &AssuredworkloadsOrganizationsLocationsWorkloadsMutatePartnerPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudAssuredworkloadsV1Workload, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = assuredworkloads_organizations_locations_workloads_mutate_partner_permissions_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = assuredworkloads_organizations_locations_workloads_mutate_partner_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Assuredworkloads organizations locations workloads patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudAssuredworkloadsV1Workload result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn assuredworkloads_organizations_locations_workloads_patch(
        &self,
        args: &AssuredworkloadsOrganizationsLocationsWorkloadsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudAssuredworkloadsV1Workload, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = assuredworkloads_organizations_locations_workloads_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = assuredworkloads_organizations_locations_workloads_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Assuredworkloads organizations locations workloads restrict allowed resources.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudAssuredworkloadsV1RestrictAllowedResourcesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn assuredworkloads_organizations_locations_workloads_restrict_allowed_resources(
        &self,
        args: &AssuredworkloadsOrganizationsLocationsWorkloadsRestrictAllowedResourcesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudAssuredworkloadsV1RestrictAllowedResourcesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = assuredworkloads_organizations_locations_workloads_restrict_allowed_resources_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = assuredworkloads_organizations_locations_workloads_restrict_allowed_resources_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Assuredworkloads organizations locations workloads updates apply.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn assuredworkloads_organizations_locations_workloads_updates_apply(
        &self,
        args: &AssuredworkloadsOrganizationsLocationsWorkloadsUpdatesApplyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = assuredworkloads_organizations_locations_workloads_updates_apply_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = assuredworkloads_organizations_locations_workloads_updates_apply_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Assuredworkloads organizations locations workloads updates list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudAssuredworkloadsV1ListWorkloadUpdatesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn assuredworkloads_organizations_locations_workloads_updates_list(
        &self,
        args: &AssuredworkloadsOrganizationsLocationsWorkloadsUpdatesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudAssuredworkloadsV1ListWorkloadUpdatesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = assuredworkloads_organizations_locations_workloads_updates_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = assuredworkloads_organizations_locations_workloads_updates_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Assuredworkloads organizations locations workloads violations acknowledge.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudAssuredworkloadsV1AcknowledgeViolationResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn assuredworkloads_organizations_locations_workloads_violations_acknowledge(
        &self,
        args: &AssuredworkloadsOrganizationsLocationsWorkloadsViolationsAcknowledgeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudAssuredworkloadsV1AcknowledgeViolationResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = assuredworkloads_organizations_locations_workloads_violations_acknowledge_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = assuredworkloads_organizations_locations_workloads_violations_acknowledge_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Assuredworkloads organizations locations workloads violations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudAssuredworkloadsV1Violation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn assuredworkloads_organizations_locations_workloads_violations_get(
        &self,
        args: &AssuredworkloadsOrganizationsLocationsWorkloadsViolationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudAssuredworkloadsV1Violation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = assuredworkloads_organizations_locations_workloads_violations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = assuredworkloads_organizations_locations_workloads_violations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Assuredworkloads organizations locations workloads violations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudAssuredworkloadsV1ListViolationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn assuredworkloads_organizations_locations_workloads_violations_list(
        &self,
        args: &AssuredworkloadsOrganizationsLocationsWorkloadsViolationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudAssuredworkloadsV1ListViolationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = assuredworkloads_organizations_locations_workloads_violations_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.interval.endTime,
            &args.interval.startTime,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = assuredworkloads_organizations_locations_workloads_violations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
