//! CloudcontrolspartnerProvider - State-aware cloudcontrolspartner API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       cloudcontrolspartner API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::cloudcontrolspartner::{
    cloudcontrolspartner_organizations_locations_get_partner_builder, cloudcontrolspartner_organizations_locations_get_partner_task,
    cloudcontrolspartner_organizations_locations_customers_create_builder, cloudcontrolspartner_organizations_locations_customers_create_task,
    cloudcontrolspartner_organizations_locations_customers_delete_builder, cloudcontrolspartner_organizations_locations_customers_delete_task,
    cloudcontrolspartner_organizations_locations_customers_get_builder, cloudcontrolspartner_organizations_locations_customers_get_task,
    cloudcontrolspartner_organizations_locations_customers_list_builder, cloudcontrolspartner_organizations_locations_customers_list_task,
    cloudcontrolspartner_organizations_locations_customers_patch_builder, cloudcontrolspartner_organizations_locations_customers_patch_task,
    cloudcontrolspartner_organizations_locations_customers_workloads_get_builder, cloudcontrolspartner_organizations_locations_customers_workloads_get_task,
    cloudcontrolspartner_organizations_locations_customers_workloads_get_ekm_connections_builder, cloudcontrolspartner_organizations_locations_customers_workloads_get_ekm_connections_task,
    cloudcontrolspartner_organizations_locations_customers_workloads_get_partner_permissions_builder, cloudcontrolspartner_organizations_locations_customers_workloads_get_partner_permissions_task,
    cloudcontrolspartner_organizations_locations_customers_workloads_list_builder, cloudcontrolspartner_organizations_locations_customers_workloads_list_task,
    cloudcontrolspartner_organizations_locations_customers_workloads_access_approval_requests_list_builder, cloudcontrolspartner_organizations_locations_customers_workloads_access_approval_requests_list_task,
    cloudcontrolspartner_organizations_locations_customers_workloads_violations_get_builder, cloudcontrolspartner_organizations_locations_customers_workloads_violations_get_task,
    cloudcontrolspartner_organizations_locations_customers_workloads_violations_list_builder, cloudcontrolspartner_organizations_locations_customers_workloads_violations_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::cloudcontrolspartner::Customer;
use crate::providers::gcp::clients::cloudcontrolspartner::EkmConnections;
use crate::providers::gcp::clients::cloudcontrolspartner::Empty;
use crate::providers::gcp::clients::cloudcontrolspartner::ListAccessApprovalRequestsResponse;
use crate::providers::gcp::clients::cloudcontrolspartner::ListCustomersResponse;
use crate::providers::gcp::clients::cloudcontrolspartner::ListViolationsResponse;
use crate::providers::gcp::clients::cloudcontrolspartner::ListWorkloadsResponse;
use crate::providers::gcp::clients::cloudcontrolspartner::Partner;
use crate::providers::gcp::clients::cloudcontrolspartner::PartnerPermissions;
use crate::providers::gcp::clients::cloudcontrolspartner::Violation;
use crate::providers::gcp::clients::cloudcontrolspartner::Workload;
use crate::providers::gcp::clients::cloudcontrolspartner::CloudcontrolspartnerOrganizationsLocationsCustomersCreateArgs;
use crate::providers::gcp::clients::cloudcontrolspartner::CloudcontrolspartnerOrganizationsLocationsCustomersDeleteArgs;
use crate::providers::gcp::clients::cloudcontrolspartner::CloudcontrolspartnerOrganizationsLocationsCustomersGetArgs;
use crate::providers::gcp::clients::cloudcontrolspartner::CloudcontrolspartnerOrganizationsLocationsCustomersListArgs;
use crate::providers::gcp::clients::cloudcontrolspartner::CloudcontrolspartnerOrganizationsLocationsCustomersPatchArgs;
use crate::providers::gcp::clients::cloudcontrolspartner::CloudcontrolspartnerOrganizationsLocationsCustomersWorkloadsAccessApprovalRequestsListArgs;
use crate::providers::gcp::clients::cloudcontrolspartner::CloudcontrolspartnerOrganizationsLocationsCustomersWorkloadsGetArgs;
use crate::providers::gcp::clients::cloudcontrolspartner::CloudcontrolspartnerOrganizationsLocationsCustomersWorkloadsGetEkmConnectionsArgs;
use crate::providers::gcp::clients::cloudcontrolspartner::CloudcontrolspartnerOrganizationsLocationsCustomersWorkloadsGetPartnerPermissionsArgs;
use crate::providers::gcp::clients::cloudcontrolspartner::CloudcontrolspartnerOrganizationsLocationsCustomersWorkloadsListArgs;
use crate::providers::gcp::clients::cloudcontrolspartner::CloudcontrolspartnerOrganizationsLocationsCustomersWorkloadsViolationsGetArgs;
use crate::providers::gcp::clients::cloudcontrolspartner::CloudcontrolspartnerOrganizationsLocationsCustomersWorkloadsViolationsListArgs;
use crate::providers::gcp::clients::cloudcontrolspartner::CloudcontrolspartnerOrganizationsLocationsGetPartnerArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// CloudcontrolspartnerProvider with automatic state tracking.
///
/// # Type Parameters
///
/// * `S` - StateStore implementation (FileStateStore, SqliteStateStore, etc.)
/// * `R` - DNS resolver type for HTTP client
///
/// # Example
///
/// ```rust
/// let state_store = FileStateStore::new("/path", "my-project", "dev");
/// let http_client = SimpleHttpClient::with_resolver(StaticSocketAddr::new(addr));
/// let client = ProviderClient::new("my-project", "dev", state_store, http_client);
/// let provider = CloudcontrolspartnerProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct CloudcontrolspartnerProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> CloudcontrolspartnerProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new CloudcontrolspartnerProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new CloudcontrolspartnerProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Cloudcontrolspartner organizations locations get partner.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Partner result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudcontrolspartner_organizations_locations_get_partner(
        &self,
        args: &CloudcontrolspartnerOrganizationsLocationsGetPartnerArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Partner, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudcontrolspartner_organizations_locations_get_partner_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudcontrolspartner_organizations_locations_get_partner_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudcontrolspartner organizations locations customers create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Customer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudcontrolspartner_organizations_locations_customers_create(
        &self,
        args: &CloudcontrolspartnerOrganizationsLocationsCustomersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Customer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudcontrolspartner_organizations_locations_customers_create_builder(
            &self.http_client,
            &args.parent,
            &args.customerId,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudcontrolspartner_organizations_locations_customers_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudcontrolspartner organizations locations customers delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudcontrolspartner_organizations_locations_customers_delete(
        &self,
        args: &CloudcontrolspartnerOrganizationsLocationsCustomersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudcontrolspartner_organizations_locations_customers_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudcontrolspartner_organizations_locations_customers_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudcontrolspartner organizations locations customers get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Customer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudcontrolspartner_organizations_locations_customers_get(
        &self,
        args: &CloudcontrolspartnerOrganizationsLocationsCustomersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Customer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudcontrolspartner_organizations_locations_customers_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudcontrolspartner_organizations_locations_customers_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudcontrolspartner organizations locations customers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCustomersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudcontrolspartner_organizations_locations_customers_list(
        &self,
        args: &CloudcontrolspartnerOrganizationsLocationsCustomersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCustomersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudcontrolspartner_organizations_locations_customers_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudcontrolspartner_organizations_locations_customers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudcontrolspartner organizations locations customers patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Customer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudcontrolspartner_organizations_locations_customers_patch(
        &self,
        args: &CloudcontrolspartnerOrganizationsLocationsCustomersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Customer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudcontrolspartner_organizations_locations_customers_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudcontrolspartner_organizations_locations_customers_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudcontrolspartner organizations locations customers workloads get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Workload result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudcontrolspartner_organizations_locations_customers_workloads_get(
        &self,
        args: &CloudcontrolspartnerOrganizationsLocationsCustomersWorkloadsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Workload, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudcontrolspartner_organizations_locations_customers_workloads_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudcontrolspartner_organizations_locations_customers_workloads_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudcontrolspartner organizations locations customers workloads get ekm connections.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EkmConnections result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudcontrolspartner_organizations_locations_customers_workloads_get_ekm_connections(
        &self,
        args: &CloudcontrolspartnerOrganizationsLocationsCustomersWorkloadsGetEkmConnectionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EkmConnections, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudcontrolspartner_organizations_locations_customers_workloads_get_ekm_connections_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudcontrolspartner_organizations_locations_customers_workloads_get_ekm_connections_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudcontrolspartner organizations locations customers workloads get partner permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PartnerPermissions result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudcontrolspartner_organizations_locations_customers_workloads_get_partner_permissions(
        &self,
        args: &CloudcontrolspartnerOrganizationsLocationsCustomersWorkloadsGetPartnerPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PartnerPermissions, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudcontrolspartner_organizations_locations_customers_workloads_get_partner_permissions_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudcontrolspartner_organizations_locations_customers_workloads_get_partner_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudcontrolspartner organizations locations customers workloads list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListWorkloadsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudcontrolspartner_organizations_locations_customers_workloads_list(
        &self,
        args: &CloudcontrolspartnerOrganizationsLocationsCustomersWorkloadsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListWorkloadsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudcontrolspartner_organizations_locations_customers_workloads_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudcontrolspartner_organizations_locations_customers_workloads_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudcontrolspartner organizations locations customers workloads access approval requests list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAccessApprovalRequestsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudcontrolspartner_organizations_locations_customers_workloads_access_approval_requests_list(
        &self,
        args: &CloudcontrolspartnerOrganizationsLocationsCustomersWorkloadsAccessApprovalRequestsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAccessApprovalRequestsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudcontrolspartner_organizations_locations_customers_workloads_access_approval_requests_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudcontrolspartner_organizations_locations_customers_workloads_access_approval_requests_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudcontrolspartner organizations locations customers workloads violations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Violation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudcontrolspartner_organizations_locations_customers_workloads_violations_get(
        &self,
        args: &CloudcontrolspartnerOrganizationsLocationsCustomersWorkloadsViolationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Violation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudcontrolspartner_organizations_locations_customers_workloads_violations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudcontrolspartner_organizations_locations_customers_workloads_violations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudcontrolspartner organizations locations customers workloads violations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListViolationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudcontrolspartner_organizations_locations_customers_workloads_violations_list(
        &self,
        args: &CloudcontrolspartnerOrganizationsLocationsCustomersWorkloadsViolationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListViolationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudcontrolspartner_organizations_locations_customers_workloads_violations_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.interval_endTime,
            &args.interval_startTime,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudcontrolspartner_organizations_locations_customers_workloads_violations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
