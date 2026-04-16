//! PrivatecaProvider - State-aware privateca API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       privateca API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::privateca::{
    privateca_projects_locations_get_builder, privateca_projects_locations_get_task,
    privateca_projects_locations_list_builder, privateca_projects_locations_list_task,
    privateca_projects_locations_ca_pools_create_builder, privateca_projects_locations_ca_pools_create_task,
    privateca_projects_locations_ca_pools_delete_builder, privateca_projects_locations_ca_pools_delete_task,
    privateca_projects_locations_ca_pools_fetch_ca_certs_builder, privateca_projects_locations_ca_pools_fetch_ca_certs_task,
    privateca_projects_locations_ca_pools_get_builder, privateca_projects_locations_ca_pools_get_task,
    privateca_projects_locations_ca_pools_get_iam_policy_builder, privateca_projects_locations_ca_pools_get_iam_policy_task,
    privateca_projects_locations_ca_pools_list_builder, privateca_projects_locations_ca_pools_list_task,
    privateca_projects_locations_ca_pools_patch_builder, privateca_projects_locations_ca_pools_patch_task,
    privateca_projects_locations_ca_pools_set_iam_policy_builder, privateca_projects_locations_ca_pools_set_iam_policy_task,
    privateca_projects_locations_ca_pools_test_iam_permissions_builder, privateca_projects_locations_ca_pools_test_iam_permissions_task,
    privateca_projects_locations_ca_pools_certificate_authorities_activate_builder, privateca_projects_locations_ca_pools_certificate_authorities_activate_task,
    privateca_projects_locations_ca_pools_certificate_authorities_create_builder, privateca_projects_locations_ca_pools_certificate_authorities_create_task,
    privateca_projects_locations_ca_pools_certificate_authorities_delete_builder, privateca_projects_locations_ca_pools_certificate_authorities_delete_task,
    privateca_projects_locations_ca_pools_certificate_authorities_disable_builder, privateca_projects_locations_ca_pools_certificate_authorities_disable_task,
    privateca_projects_locations_ca_pools_certificate_authorities_enable_builder, privateca_projects_locations_ca_pools_certificate_authorities_enable_task,
    privateca_projects_locations_ca_pools_certificate_authorities_fetch_builder, privateca_projects_locations_ca_pools_certificate_authorities_fetch_task,
    privateca_projects_locations_ca_pools_certificate_authorities_get_builder, privateca_projects_locations_ca_pools_certificate_authorities_get_task,
    privateca_projects_locations_ca_pools_certificate_authorities_list_builder, privateca_projects_locations_ca_pools_certificate_authorities_list_task,
    privateca_projects_locations_ca_pools_certificate_authorities_patch_builder, privateca_projects_locations_ca_pools_certificate_authorities_patch_task,
    privateca_projects_locations_ca_pools_certificate_authorities_undelete_builder, privateca_projects_locations_ca_pools_certificate_authorities_undelete_task,
    privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_get_builder, privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_get_task,
    privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_get_iam_policy_builder, privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_get_iam_policy_task,
    privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_list_builder, privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_list_task,
    privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_patch_builder, privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_patch_task,
    privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_set_iam_policy_builder, privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_set_iam_policy_task,
    privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_test_iam_permissions_builder, privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_test_iam_permissions_task,
    privateca_projects_locations_ca_pools_certificates_create_builder, privateca_projects_locations_ca_pools_certificates_create_task,
    privateca_projects_locations_ca_pools_certificates_get_builder, privateca_projects_locations_ca_pools_certificates_get_task,
    privateca_projects_locations_ca_pools_certificates_list_builder, privateca_projects_locations_ca_pools_certificates_list_task,
    privateca_projects_locations_ca_pools_certificates_patch_builder, privateca_projects_locations_ca_pools_certificates_patch_task,
    privateca_projects_locations_ca_pools_certificates_revoke_builder, privateca_projects_locations_ca_pools_certificates_revoke_task,
    privateca_projects_locations_certificate_templates_create_builder, privateca_projects_locations_certificate_templates_create_task,
    privateca_projects_locations_certificate_templates_delete_builder, privateca_projects_locations_certificate_templates_delete_task,
    privateca_projects_locations_certificate_templates_get_builder, privateca_projects_locations_certificate_templates_get_task,
    privateca_projects_locations_certificate_templates_get_iam_policy_builder, privateca_projects_locations_certificate_templates_get_iam_policy_task,
    privateca_projects_locations_certificate_templates_list_builder, privateca_projects_locations_certificate_templates_list_task,
    privateca_projects_locations_certificate_templates_patch_builder, privateca_projects_locations_certificate_templates_patch_task,
    privateca_projects_locations_certificate_templates_set_iam_policy_builder, privateca_projects_locations_certificate_templates_set_iam_policy_task,
    privateca_projects_locations_certificate_templates_test_iam_permissions_builder, privateca_projects_locations_certificate_templates_test_iam_permissions_task,
    privateca_projects_locations_operations_cancel_builder, privateca_projects_locations_operations_cancel_task,
    privateca_projects_locations_operations_delete_builder, privateca_projects_locations_operations_delete_task,
    privateca_projects_locations_operations_get_builder, privateca_projects_locations_operations_get_task,
    privateca_projects_locations_operations_list_builder, privateca_projects_locations_operations_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::privateca::CaPool;
use crate::providers::gcp::clients::privateca::Certificate;
use crate::providers::gcp::clients::privateca::CertificateAuthority;
use crate::providers::gcp::clients::privateca::CertificateRevocationList;
use crate::providers::gcp::clients::privateca::CertificateTemplate;
use crate::providers::gcp::clients::privateca::Empty;
use crate::providers::gcp::clients::privateca::FetchCaCertsResponse;
use crate::providers::gcp::clients::privateca::FetchCertificateAuthorityCsrResponse;
use crate::providers::gcp::clients::privateca::ListCaPoolsResponse;
use crate::providers::gcp::clients::privateca::ListCertificateAuthoritiesResponse;
use crate::providers::gcp::clients::privateca::ListCertificateRevocationListsResponse;
use crate::providers::gcp::clients::privateca::ListCertificateTemplatesResponse;
use crate::providers::gcp::clients::privateca::ListCertificatesResponse;
use crate::providers::gcp::clients::privateca::ListLocationsResponse;
use crate::providers::gcp::clients::privateca::ListOperationsResponse;
use crate::providers::gcp::clients::privateca::Location;
use crate::providers::gcp::clients::privateca::Operation;
use crate::providers::gcp::clients::privateca::Policy;
use crate::providers::gcp::clients::privateca::TestIamPermissionsResponse;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesActivateArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesCertificateRevocationListsGetArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesCertificateRevocationListsGetIamPolicyArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesCertificateRevocationListsListArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesCertificateRevocationListsPatchArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesCertificateRevocationListsSetIamPolicyArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesCertificateRevocationListsTestIamPermissionsArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesCreateArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesDeleteArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesDisableArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesEnableArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesFetchArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesGetArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesListArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesPatchArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesUndeleteArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsCertificatesCreateArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsCertificatesGetArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsCertificatesListArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsCertificatesPatchArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsCertificatesRevokeArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsCreateArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsDeleteArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsFetchCaCertsArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsGetArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsGetIamPolicyArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsListArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsPatchArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsSetIamPolicyArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCaPoolsTestIamPermissionsArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCertificateTemplatesCreateArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCertificateTemplatesDeleteArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCertificateTemplatesGetArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCertificateTemplatesGetIamPolicyArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCertificateTemplatesListArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCertificateTemplatesPatchArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCertificateTemplatesSetIamPolicyArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsCertificateTemplatesTestIamPermissionsArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsGetArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsListArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::privateca::PrivatecaProjectsLocationsOperationsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// PrivatecaProvider with automatic state tracking.
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
/// let provider = PrivatecaProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct PrivatecaProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> PrivatecaProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new PrivatecaProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new PrivatecaProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Privateca projects locations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Location result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn privateca_projects_locations_get(
        &self,
        args: &PrivatecaProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLocationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn privateca_projects_locations_list(
        &self,
        args: &PrivatecaProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn privateca_projects_locations_ca_pools_create(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_create_builder(
            &self.http_client,
            &args.parent,
            &args.caPoolId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn privateca_projects_locations_ca_pools_delete(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_delete_builder(
            &self.http_client,
            &args.name,
            &args.ignoreDependentResources,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools fetch ca certs.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchCaCertsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn privateca_projects_locations_ca_pools_fetch_ca_certs(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsFetchCaCertsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchCaCertsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_fetch_ca_certs_builder(
            &self.http_client,
            &args.caPool,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_fetch_ca_certs_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CaPool result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn privateca_projects_locations_ca_pools_get(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CaPool, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn privateca_projects_locations_ca_pools_get_iam_policy(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCaPoolsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn privateca_projects_locations_ca_pools_list(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCaPoolsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn privateca_projects_locations_ca_pools_patch(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn privateca_projects_locations_ca_pools_set_iam_policy(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn privateca_projects_locations_ca_pools_test_iam_permissions(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools certificate authorities activate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn privateca_projects_locations_ca_pools_certificate_authorities_activate(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesActivateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_certificate_authorities_activate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_certificate_authorities_activate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools certificate authorities create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn privateca_projects_locations_ca_pools_certificate_authorities_create(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_certificate_authorities_create_builder(
            &self.http_client,
            &args.parent,
            &args.certificateAuthorityId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_certificate_authorities_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools certificate authorities delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn privateca_projects_locations_ca_pools_certificate_authorities_delete(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_certificate_authorities_delete_builder(
            &self.http_client,
            &args.name,
            &args.ignoreActiveCertificates,
            &args.ignoreDependentResources,
            &args.requestId,
            &args.skipGracePeriod,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_certificate_authorities_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools certificate authorities disable.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn privateca_projects_locations_ca_pools_certificate_authorities_disable(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesDisableArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_certificate_authorities_disable_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_certificate_authorities_disable_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools certificate authorities enable.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn privateca_projects_locations_ca_pools_certificate_authorities_enable(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesEnableArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_certificate_authorities_enable_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_certificate_authorities_enable_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools certificate authorities fetch.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchCertificateAuthorityCsrResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn privateca_projects_locations_ca_pools_certificate_authorities_fetch(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesFetchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchCertificateAuthorityCsrResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_certificate_authorities_fetch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_certificate_authorities_fetch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools certificate authorities get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CertificateAuthority result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn privateca_projects_locations_ca_pools_certificate_authorities_get(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CertificateAuthority, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_certificate_authorities_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_certificate_authorities_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools certificate authorities list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCertificateAuthoritiesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn privateca_projects_locations_ca_pools_certificate_authorities_list(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCertificateAuthoritiesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_certificate_authorities_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_certificate_authorities_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools certificate authorities patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn privateca_projects_locations_ca_pools_certificate_authorities_patch(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_certificate_authorities_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_certificate_authorities_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools certificate authorities undelete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn privateca_projects_locations_ca_pools_certificate_authorities_undelete(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesUndeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_certificate_authorities_undelete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_certificate_authorities_undelete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools certificate authorities certificate revocation lists get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CertificateRevocationList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_get(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesCertificateRevocationListsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CertificateRevocationList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools certificate authorities certificate revocation lists get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_get_iam_policy(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesCertificateRevocationListsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools certificate authorities certificate revocation lists list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCertificateRevocationListsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_list(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesCertificateRevocationListsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCertificateRevocationListsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools certificate authorities certificate revocation lists patch.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_patch(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesCertificateRevocationListsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_patch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools certificate authorities certificate revocation lists set iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_set_iam_policy(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesCertificateRevocationListsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools certificate authorities certificate revocation lists test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_test_iam_permissions(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsCertificateAuthoritiesCertificateRevocationListsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_certificate_authorities_certificate_revocation_lists_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools certificates create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Certificate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn privateca_projects_locations_ca_pools_certificates_create(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsCertificatesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Certificate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_certificates_create_builder(
            &self.http_client,
            &args.parent,
            &args.certificateId,
            &args.issuingCertificateAuthorityId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_certificates_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools certificates get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Certificate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn privateca_projects_locations_ca_pools_certificates_get(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsCertificatesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Certificate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_certificates_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_certificates_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools certificates list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCertificatesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn privateca_projects_locations_ca_pools_certificates_list(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsCertificatesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCertificatesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_certificates_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_certificates_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools certificates patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Certificate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn privateca_projects_locations_ca_pools_certificates_patch(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsCertificatesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Certificate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_certificates_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_certificates_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations ca pools certificates revoke.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Certificate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn privateca_projects_locations_ca_pools_certificates_revoke(
        &self,
        args: &PrivatecaProjectsLocationsCaPoolsCertificatesRevokeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Certificate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_ca_pools_certificates_revoke_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_ca_pools_certificates_revoke_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations certificate templates create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn privateca_projects_locations_certificate_templates_create(
        &self,
        args: &PrivatecaProjectsLocationsCertificateTemplatesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_certificate_templates_create_builder(
            &self.http_client,
            &args.parent,
            &args.certificateTemplateId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_certificate_templates_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations certificate templates delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn privateca_projects_locations_certificate_templates_delete(
        &self,
        args: &PrivatecaProjectsLocationsCertificateTemplatesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_certificate_templates_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_certificate_templates_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations certificate templates get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CertificateTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn privateca_projects_locations_certificate_templates_get(
        &self,
        args: &PrivatecaProjectsLocationsCertificateTemplatesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CertificateTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_certificate_templates_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_certificate_templates_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations certificate templates get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn privateca_projects_locations_certificate_templates_get_iam_policy(
        &self,
        args: &PrivatecaProjectsLocationsCertificateTemplatesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_certificate_templates_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_certificate_templates_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations certificate templates list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCertificateTemplatesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn privateca_projects_locations_certificate_templates_list(
        &self,
        args: &PrivatecaProjectsLocationsCertificateTemplatesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCertificateTemplatesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_certificate_templates_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_certificate_templates_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations certificate templates patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn privateca_projects_locations_certificate_templates_patch(
        &self,
        args: &PrivatecaProjectsLocationsCertificateTemplatesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_certificate_templates_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_certificate_templates_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations certificate templates set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn privateca_projects_locations_certificate_templates_set_iam_policy(
        &self,
        args: &PrivatecaProjectsLocationsCertificateTemplatesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_certificate_templates_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_certificate_templates_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations certificate templates test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn privateca_projects_locations_certificate_templates_test_iam_permissions(
        &self,
        args: &PrivatecaProjectsLocationsCertificateTemplatesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_certificate_templates_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_certificate_templates_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations operations cancel.
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
    pub fn privateca_projects_locations_operations_cancel(
        &self,
        args: &PrivatecaProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations operations delete.
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
    pub fn privateca_projects_locations_operations_delete(
        &self,
        args: &PrivatecaProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations operations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn privateca_projects_locations_operations_get(
        &self,
        args: &PrivatecaProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Privateca projects locations operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListOperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn privateca_projects_locations_operations_list(
        &self,
        args: &PrivatecaProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = privateca_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = privateca_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
