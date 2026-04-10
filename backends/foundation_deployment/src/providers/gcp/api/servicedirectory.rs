//! ServicedirectoryProvider - State-aware servicedirectory API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       servicedirectory API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::servicedirectory::{
    servicedirectory_projects_locations_namespaces_create_builder, servicedirectory_projects_locations_namespaces_create_task,
    servicedirectory_projects_locations_namespaces_delete_builder, servicedirectory_projects_locations_namespaces_delete_task,
    servicedirectory_projects_locations_namespaces_get_iam_policy_builder, servicedirectory_projects_locations_namespaces_get_iam_policy_task,
    servicedirectory_projects_locations_namespaces_patch_builder, servicedirectory_projects_locations_namespaces_patch_task,
    servicedirectory_projects_locations_namespaces_set_iam_policy_builder, servicedirectory_projects_locations_namespaces_set_iam_policy_task,
    servicedirectory_projects_locations_namespaces_test_iam_permissions_builder, servicedirectory_projects_locations_namespaces_test_iam_permissions_task,
    servicedirectory_projects_locations_namespaces_services_create_builder, servicedirectory_projects_locations_namespaces_services_create_task,
    servicedirectory_projects_locations_namespaces_services_delete_builder, servicedirectory_projects_locations_namespaces_services_delete_task,
    servicedirectory_projects_locations_namespaces_services_get_iam_policy_builder, servicedirectory_projects_locations_namespaces_services_get_iam_policy_task,
    servicedirectory_projects_locations_namespaces_services_patch_builder, servicedirectory_projects_locations_namespaces_services_patch_task,
    servicedirectory_projects_locations_namespaces_services_resolve_builder, servicedirectory_projects_locations_namespaces_services_resolve_task,
    servicedirectory_projects_locations_namespaces_services_set_iam_policy_builder, servicedirectory_projects_locations_namespaces_services_set_iam_policy_task,
    servicedirectory_projects_locations_namespaces_services_test_iam_permissions_builder, servicedirectory_projects_locations_namespaces_services_test_iam_permissions_task,
    servicedirectory_projects_locations_namespaces_services_endpoints_create_builder, servicedirectory_projects_locations_namespaces_services_endpoints_create_task,
    servicedirectory_projects_locations_namespaces_services_endpoints_delete_builder, servicedirectory_projects_locations_namespaces_services_endpoints_delete_task,
    servicedirectory_projects_locations_namespaces_services_endpoints_patch_builder, servicedirectory_projects_locations_namespaces_services_endpoints_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::servicedirectory::Empty;
use crate::providers::gcp::clients::servicedirectory::Endpoint;
use crate::providers::gcp::clients::servicedirectory::Namespace;
use crate::providers::gcp::clients::servicedirectory::Policy;
use crate::providers::gcp::clients::servicedirectory::ResolveServiceResponse;
use crate::providers::gcp::clients::servicedirectory::Service;
use crate::providers::gcp::clients::servicedirectory::TestIamPermissionsResponse;
use crate::providers::gcp::clients::servicedirectory::ServicedirectoryProjectsLocationsNamespacesCreateArgs;
use crate::providers::gcp::clients::servicedirectory::ServicedirectoryProjectsLocationsNamespacesDeleteArgs;
use crate::providers::gcp::clients::servicedirectory::ServicedirectoryProjectsLocationsNamespacesGetIamPolicyArgs;
use crate::providers::gcp::clients::servicedirectory::ServicedirectoryProjectsLocationsNamespacesPatchArgs;
use crate::providers::gcp::clients::servicedirectory::ServicedirectoryProjectsLocationsNamespacesServicesCreateArgs;
use crate::providers::gcp::clients::servicedirectory::ServicedirectoryProjectsLocationsNamespacesServicesDeleteArgs;
use crate::providers::gcp::clients::servicedirectory::ServicedirectoryProjectsLocationsNamespacesServicesEndpointsCreateArgs;
use crate::providers::gcp::clients::servicedirectory::ServicedirectoryProjectsLocationsNamespacesServicesEndpointsDeleteArgs;
use crate::providers::gcp::clients::servicedirectory::ServicedirectoryProjectsLocationsNamespacesServicesEndpointsPatchArgs;
use crate::providers::gcp::clients::servicedirectory::ServicedirectoryProjectsLocationsNamespacesServicesGetIamPolicyArgs;
use crate::providers::gcp::clients::servicedirectory::ServicedirectoryProjectsLocationsNamespacesServicesPatchArgs;
use crate::providers::gcp::clients::servicedirectory::ServicedirectoryProjectsLocationsNamespacesServicesResolveArgs;
use crate::providers::gcp::clients::servicedirectory::ServicedirectoryProjectsLocationsNamespacesServicesSetIamPolicyArgs;
use crate::providers::gcp::clients::servicedirectory::ServicedirectoryProjectsLocationsNamespacesServicesTestIamPermissionsArgs;
use crate::providers::gcp::clients::servicedirectory::ServicedirectoryProjectsLocationsNamespacesSetIamPolicyArgs;
use crate::providers::gcp::clients::servicedirectory::ServicedirectoryProjectsLocationsNamespacesTestIamPermissionsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ServicedirectoryProvider with automatic state tracking.
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
/// let provider = ServicedirectoryProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ServicedirectoryProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ServicedirectoryProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ServicedirectoryProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Servicedirectory projects locations namespaces create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Namespace result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn servicedirectory_projects_locations_namespaces_create(
        &self,
        args: &ServicedirectoryProjectsLocationsNamespacesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Namespace, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicedirectory_projects_locations_namespaces_create_builder(
            &self.http_client,
            &args.parent,
            &args.namespaceId,
        )
        .map_err(ProviderError::Api)?;

        let task = servicedirectory_projects_locations_namespaces_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicedirectory projects locations namespaces delete.
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
    pub fn servicedirectory_projects_locations_namespaces_delete(
        &self,
        args: &ServicedirectoryProjectsLocationsNamespacesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicedirectory_projects_locations_namespaces_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = servicedirectory_projects_locations_namespaces_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicedirectory projects locations namespaces get iam policy.
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
    pub fn servicedirectory_projects_locations_namespaces_get_iam_policy(
        &self,
        args: &ServicedirectoryProjectsLocationsNamespacesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicedirectory_projects_locations_namespaces_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = servicedirectory_projects_locations_namespaces_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicedirectory projects locations namespaces patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Namespace result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn servicedirectory_projects_locations_namespaces_patch(
        &self,
        args: &ServicedirectoryProjectsLocationsNamespacesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Namespace, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicedirectory_projects_locations_namespaces_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = servicedirectory_projects_locations_namespaces_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicedirectory projects locations namespaces set iam policy.
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
    pub fn servicedirectory_projects_locations_namespaces_set_iam_policy(
        &self,
        args: &ServicedirectoryProjectsLocationsNamespacesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicedirectory_projects_locations_namespaces_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = servicedirectory_projects_locations_namespaces_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicedirectory projects locations namespaces test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn servicedirectory_projects_locations_namespaces_test_iam_permissions(
        &self,
        args: &ServicedirectoryProjectsLocationsNamespacesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicedirectory_projects_locations_namespaces_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = servicedirectory_projects_locations_namespaces_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicedirectory projects locations namespaces services create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Service result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn servicedirectory_projects_locations_namespaces_services_create(
        &self,
        args: &ServicedirectoryProjectsLocationsNamespacesServicesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Service, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicedirectory_projects_locations_namespaces_services_create_builder(
            &self.http_client,
            &args.parent,
            &args.serviceId,
        )
        .map_err(ProviderError::Api)?;

        let task = servicedirectory_projects_locations_namespaces_services_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicedirectory projects locations namespaces services delete.
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
    pub fn servicedirectory_projects_locations_namespaces_services_delete(
        &self,
        args: &ServicedirectoryProjectsLocationsNamespacesServicesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicedirectory_projects_locations_namespaces_services_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = servicedirectory_projects_locations_namespaces_services_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicedirectory projects locations namespaces services get iam policy.
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
    pub fn servicedirectory_projects_locations_namespaces_services_get_iam_policy(
        &self,
        args: &ServicedirectoryProjectsLocationsNamespacesServicesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicedirectory_projects_locations_namespaces_services_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = servicedirectory_projects_locations_namespaces_services_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicedirectory projects locations namespaces services patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Service result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn servicedirectory_projects_locations_namespaces_services_patch(
        &self,
        args: &ServicedirectoryProjectsLocationsNamespacesServicesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Service, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicedirectory_projects_locations_namespaces_services_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = servicedirectory_projects_locations_namespaces_services_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicedirectory projects locations namespaces services resolve.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ResolveServiceResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn servicedirectory_projects_locations_namespaces_services_resolve(
        &self,
        args: &ServicedirectoryProjectsLocationsNamespacesServicesResolveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ResolveServiceResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicedirectory_projects_locations_namespaces_services_resolve_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = servicedirectory_projects_locations_namespaces_services_resolve_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicedirectory projects locations namespaces services set iam policy.
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
    pub fn servicedirectory_projects_locations_namespaces_services_set_iam_policy(
        &self,
        args: &ServicedirectoryProjectsLocationsNamespacesServicesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicedirectory_projects_locations_namespaces_services_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = servicedirectory_projects_locations_namespaces_services_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicedirectory projects locations namespaces services test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn servicedirectory_projects_locations_namespaces_services_test_iam_permissions(
        &self,
        args: &ServicedirectoryProjectsLocationsNamespacesServicesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicedirectory_projects_locations_namespaces_services_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = servicedirectory_projects_locations_namespaces_services_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicedirectory projects locations namespaces services endpoints create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Endpoint result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn servicedirectory_projects_locations_namespaces_services_endpoints_create(
        &self,
        args: &ServicedirectoryProjectsLocationsNamespacesServicesEndpointsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Endpoint, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicedirectory_projects_locations_namespaces_services_endpoints_create_builder(
            &self.http_client,
            &args.parent,
            &args.endpointId,
        )
        .map_err(ProviderError::Api)?;

        let task = servicedirectory_projects_locations_namespaces_services_endpoints_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicedirectory projects locations namespaces services endpoints delete.
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
    pub fn servicedirectory_projects_locations_namespaces_services_endpoints_delete(
        &self,
        args: &ServicedirectoryProjectsLocationsNamespacesServicesEndpointsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicedirectory_projects_locations_namespaces_services_endpoints_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = servicedirectory_projects_locations_namespaces_services_endpoints_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicedirectory projects locations namespaces services endpoints patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Endpoint result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn servicedirectory_projects_locations_namespaces_services_endpoints_patch(
        &self,
        args: &ServicedirectoryProjectsLocationsNamespacesServicesEndpointsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Endpoint, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicedirectory_projects_locations_namespaces_services_endpoints_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = servicedirectory_projects_locations_namespaces_services_endpoints_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
