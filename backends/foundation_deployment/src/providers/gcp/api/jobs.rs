//! JobsProvider - State-aware jobs API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       jobs API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::jobs::{
    jobs_projects_operations_get_builder, jobs_projects_operations_get_task,
    jobs_projects_tenants_complete_query_builder, jobs_projects_tenants_complete_query_task,
    jobs_projects_tenants_create_builder, jobs_projects_tenants_create_task,
    jobs_projects_tenants_delete_builder, jobs_projects_tenants_delete_task,
    jobs_projects_tenants_get_builder, jobs_projects_tenants_get_task,
    jobs_projects_tenants_list_builder, jobs_projects_tenants_list_task,
    jobs_projects_tenants_patch_builder, jobs_projects_tenants_patch_task,
    jobs_projects_tenants_client_events_create_builder, jobs_projects_tenants_client_events_create_task,
    jobs_projects_tenants_companies_create_builder, jobs_projects_tenants_companies_create_task,
    jobs_projects_tenants_companies_delete_builder, jobs_projects_tenants_companies_delete_task,
    jobs_projects_tenants_companies_get_builder, jobs_projects_tenants_companies_get_task,
    jobs_projects_tenants_companies_list_builder, jobs_projects_tenants_companies_list_task,
    jobs_projects_tenants_companies_patch_builder, jobs_projects_tenants_companies_patch_task,
    jobs_projects_tenants_jobs_batch_create_builder, jobs_projects_tenants_jobs_batch_create_task,
    jobs_projects_tenants_jobs_batch_delete_builder, jobs_projects_tenants_jobs_batch_delete_task,
    jobs_projects_tenants_jobs_batch_update_builder, jobs_projects_tenants_jobs_batch_update_task,
    jobs_projects_tenants_jobs_create_builder, jobs_projects_tenants_jobs_create_task,
    jobs_projects_tenants_jobs_delete_builder, jobs_projects_tenants_jobs_delete_task,
    jobs_projects_tenants_jobs_get_builder, jobs_projects_tenants_jobs_get_task,
    jobs_projects_tenants_jobs_list_builder, jobs_projects_tenants_jobs_list_task,
    jobs_projects_tenants_jobs_patch_builder, jobs_projects_tenants_jobs_patch_task,
    jobs_projects_tenants_jobs_search_builder, jobs_projects_tenants_jobs_search_task,
    jobs_projects_tenants_jobs_search_for_alert_builder, jobs_projects_tenants_jobs_search_for_alert_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::jobs::ClientEvent;
use crate::providers::gcp::clients::jobs::Company;
use crate::providers::gcp::clients::jobs::CompleteQueryResponse;
use crate::providers::gcp::clients::jobs::Empty;
use crate::providers::gcp::clients::jobs::Job;
use crate::providers::gcp::clients::jobs::ListCompaniesResponse;
use crate::providers::gcp::clients::jobs::ListJobsResponse;
use crate::providers::gcp::clients::jobs::ListTenantsResponse;
use crate::providers::gcp::clients::jobs::Operation;
use crate::providers::gcp::clients::jobs::SearchJobsResponse;
use crate::providers::gcp::clients::jobs::Tenant;
use crate::providers::gcp::clients::jobs::JobsProjectsOperationsGetArgs;
use crate::providers::gcp::clients::jobs::JobsProjectsTenantsClientEventsCreateArgs;
use crate::providers::gcp::clients::jobs::JobsProjectsTenantsCompaniesCreateArgs;
use crate::providers::gcp::clients::jobs::JobsProjectsTenantsCompaniesDeleteArgs;
use crate::providers::gcp::clients::jobs::JobsProjectsTenantsCompaniesGetArgs;
use crate::providers::gcp::clients::jobs::JobsProjectsTenantsCompaniesListArgs;
use crate::providers::gcp::clients::jobs::JobsProjectsTenantsCompaniesPatchArgs;
use crate::providers::gcp::clients::jobs::JobsProjectsTenantsCompleteQueryArgs;
use crate::providers::gcp::clients::jobs::JobsProjectsTenantsCreateArgs;
use crate::providers::gcp::clients::jobs::JobsProjectsTenantsDeleteArgs;
use crate::providers::gcp::clients::jobs::JobsProjectsTenantsGetArgs;
use crate::providers::gcp::clients::jobs::JobsProjectsTenantsJobsBatchCreateArgs;
use crate::providers::gcp::clients::jobs::JobsProjectsTenantsJobsBatchDeleteArgs;
use crate::providers::gcp::clients::jobs::JobsProjectsTenantsJobsBatchUpdateArgs;
use crate::providers::gcp::clients::jobs::JobsProjectsTenantsJobsCreateArgs;
use crate::providers::gcp::clients::jobs::JobsProjectsTenantsJobsDeleteArgs;
use crate::providers::gcp::clients::jobs::JobsProjectsTenantsJobsGetArgs;
use crate::providers::gcp::clients::jobs::JobsProjectsTenantsJobsListArgs;
use crate::providers::gcp::clients::jobs::JobsProjectsTenantsJobsPatchArgs;
use crate::providers::gcp::clients::jobs::JobsProjectsTenantsJobsSearchArgs;
use crate::providers::gcp::clients::jobs::JobsProjectsTenantsJobsSearchForAlertArgs;
use crate::providers::gcp::clients::jobs::JobsProjectsTenantsListArgs;
use crate::providers::gcp::clients::jobs::JobsProjectsTenantsPatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// JobsProvider with automatic state tracking.
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
/// let provider = JobsProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct JobsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> JobsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new JobsProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Jobs projects operations get.
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
    pub fn jobs_projects_operations_get(
        &self,
        args: &JobsProjectsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = jobs_projects_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = jobs_projects_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Jobs projects tenants complete query.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CompleteQueryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn jobs_projects_tenants_complete_query(
        &self,
        args: &JobsProjectsTenantsCompleteQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CompleteQueryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = jobs_projects_tenants_complete_query_builder(
            &self.http_client,
            &args.tenant,
            &args.company,
            &args.languageCodes,
            &args.pageSize,
            &args.query,
            &args.scope,
            &args.type_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = jobs_projects_tenants_complete_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Jobs projects tenants create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Tenant result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn jobs_projects_tenants_create(
        &self,
        args: &JobsProjectsTenantsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Tenant, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = jobs_projects_tenants_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = jobs_projects_tenants_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Jobs projects tenants delete.
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
    pub fn jobs_projects_tenants_delete(
        &self,
        args: &JobsProjectsTenantsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = jobs_projects_tenants_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = jobs_projects_tenants_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Jobs projects tenants get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Tenant result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn jobs_projects_tenants_get(
        &self,
        args: &JobsProjectsTenantsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Tenant, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = jobs_projects_tenants_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = jobs_projects_tenants_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Jobs projects tenants list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTenantsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn jobs_projects_tenants_list(
        &self,
        args: &JobsProjectsTenantsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTenantsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = jobs_projects_tenants_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = jobs_projects_tenants_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Jobs projects tenants patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Tenant result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn jobs_projects_tenants_patch(
        &self,
        args: &JobsProjectsTenantsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Tenant, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = jobs_projects_tenants_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = jobs_projects_tenants_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Jobs projects tenants client events create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ClientEvent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn jobs_projects_tenants_client_events_create(
        &self,
        args: &JobsProjectsTenantsClientEventsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ClientEvent, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = jobs_projects_tenants_client_events_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = jobs_projects_tenants_client_events_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Jobs projects tenants companies create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Company result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn jobs_projects_tenants_companies_create(
        &self,
        args: &JobsProjectsTenantsCompaniesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Company, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = jobs_projects_tenants_companies_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = jobs_projects_tenants_companies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Jobs projects tenants companies delete.
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
    pub fn jobs_projects_tenants_companies_delete(
        &self,
        args: &JobsProjectsTenantsCompaniesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = jobs_projects_tenants_companies_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = jobs_projects_tenants_companies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Jobs projects tenants companies get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Company result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn jobs_projects_tenants_companies_get(
        &self,
        args: &JobsProjectsTenantsCompaniesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Company, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = jobs_projects_tenants_companies_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = jobs_projects_tenants_companies_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Jobs projects tenants companies list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCompaniesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn jobs_projects_tenants_companies_list(
        &self,
        args: &JobsProjectsTenantsCompaniesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCompaniesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = jobs_projects_tenants_companies_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.requireOpenJobs,
        )
        .map_err(ProviderError::Api)?;

        let task = jobs_projects_tenants_companies_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Jobs projects tenants companies patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Company result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn jobs_projects_tenants_companies_patch(
        &self,
        args: &JobsProjectsTenantsCompaniesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Company, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = jobs_projects_tenants_companies_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = jobs_projects_tenants_companies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Jobs projects tenants jobs batch create.
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
    pub fn jobs_projects_tenants_jobs_batch_create(
        &self,
        args: &JobsProjectsTenantsJobsBatchCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = jobs_projects_tenants_jobs_batch_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = jobs_projects_tenants_jobs_batch_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Jobs projects tenants jobs batch delete.
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
    pub fn jobs_projects_tenants_jobs_batch_delete(
        &self,
        args: &JobsProjectsTenantsJobsBatchDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = jobs_projects_tenants_jobs_batch_delete_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = jobs_projects_tenants_jobs_batch_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Jobs projects tenants jobs batch update.
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
    pub fn jobs_projects_tenants_jobs_batch_update(
        &self,
        args: &JobsProjectsTenantsJobsBatchUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = jobs_projects_tenants_jobs_batch_update_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = jobs_projects_tenants_jobs_batch_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Jobs projects tenants jobs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Job result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn jobs_projects_tenants_jobs_create(
        &self,
        args: &JobsProjectsTenantsJobsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = jobs_projects_tenants_jobs_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = jobs_projects_tenants_jobs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Jobs projects tenants jobs delete.
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
    pub fn jobs_projects_tenants_jobs_delete(
        &self,
        args: &JobsProjectsTenantsJobsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = jobs_projects_tenants_jobs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = jobs_projects_tenants_jobs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Jobs projects tenants jobs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Job result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn jobs_projects_tenants_jobs_get(
        &self,
        args: &JobsProjectsTenantsJobsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = jobs_projects_tenants_jobs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = jobs_projects_tenants_jobs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Jobs projects tenants jobs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListJobsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn jobs_projects_tenants_jobs_list(
        &self,
        args: &JobsProjectsTenantsJobsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListJobsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = jobs_projects_tenants_jobs_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.jobView,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = jobs_projects_tenants_jobs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Jobs projects tenants jobs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Job result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn jobs_projects_tenants_jobs_patch(
        &self,
        args: &JobsProjectsTenantsJobsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = jobs_projects_tenants_jobs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = jobs_projects_tenants_jobs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Jobs projects tenants jobs search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchJobsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn jobs_projects_tenants_jobs_search(
        &self,
        args: &JobsProjectsTenantsJobsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchJobsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = jobs_projects_tenants_jobs_search_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = jobs_projects_tenants_jobs_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Jobs projects tenants jobs search for alert.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchJobsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn jobs_projects_tenants_jobs_search_for_alert(
        &self,
        args: &JobsProjectsTenantsJobsSearchForAlertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchJobsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = jobs_projects_tenants_jobs_search_for_alert_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = jobs_projects_tenants_jobs_search_for_alert_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
