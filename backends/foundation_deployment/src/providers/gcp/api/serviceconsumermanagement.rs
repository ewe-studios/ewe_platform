//! ServiceconsumermanagementProvider - State-aware serviceconsumermanagement API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       serviceconsumermanagement API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::serviceconsumermanagement::{
    serviceconsumermanagement_operations_cancel_builder, serviceconsumermanagement_operations_cancel_task,
    serviceconsumermanagement_operations_delete_builder, serviceconsumermanagement_operations_delete_task,
    serviceconsumermanagement_operations_get_builder, serviceconsumermanagement_operations_get_task,
    serviceconsumermanagement_operations_list_builder, serviceconsumermanagement_operations_list_task,
    serviceconsumermanagement_services_search_builder, serviceconsumermanagement_services_search_task,
    serviceconsumermanagement_services_tenancy_units_add_project_builder, serviceconsumermanagement_services_tenancy_units_add_project_task,
    serviceconsumermanagement_services_tenancy_units_apply_project_config_builder, serviceconsumermanagement_services_tenancy_units_apply_project_config_task,
    serviceconsumermanagement_services_tenancy_units_attach_project_builder, serviceconsumermanagement_services_tenancy_units_attach_project_task,
    serviceconsumermanagement_services_tenancy_units_create_builder, serviceconsumermanagement_services_tenancy_units_create_task,
    serviceconsumermanagement_services_tenancy_units_delete_builder, serviceconsumermanagement_services_tenancy_units_delete_task,
    serviceconsumermanagement_services_tenancy_units_delete_project_builder, serviceconsumermanagement_services_tenancy_units_delete_project_task,
    serviceconsumermanagement_services_tenancy_units_list_builder, serviceconsumermanagement_services_tenancy_units_list_task,
    serviceconsumermanagement_services_tenancy_units_remove_project_builder, serviceconsumermanagement_services_tenancy_units_remove_project_task,
    serviceconsumermanagement_services_tenancy_units_undelete_project_builder, serviceconsumermanagement_services_tenancy_units_undelete_project_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::serviceconsumermanagement::Empty;
use crate::providers::gcp::clients::serviceconsumermanagement::ListOperationsResponse;
use crate::providers::gcp::clients::serviceconsumermanagement::ListTenancyUnitsResponse;
use crate::providers::gcp::clients::serviceconsumermanagement::Operation;
use crate::providers::gcp::clients::serviceconsumermanagement::SearchTenancyUnitsResponse;
use crate::providers::gcp::clients::serviceconsumermanagement::TenancyUnit;
use crate::providers::gcp::clients::serviceconsumermanagement::ServiceconsumermanagementOperationsCancelArgs;
use crate::providers::gcp::clients::serviceconsumermanagement::ServiceconsumermanagementOperationsDeleteArgs;
use crate::providers::gcp::clients::serviceconsumermanagement::ServiceconsumermanagementOperationsGetArgs;
use crate::providers::gcp::clients::serviceconsumermanagement::ServiceconsumermanagementOperationsListArgs;
use crate::providers::gcp::clients::serviceconsumermanagement::ServiceconsumermanagementServicesSearchArgs;
use crate::providers::gcp::clients::serviceconsumermanagement::ServiceconsumermanagementServicesTenancyUnitsAddProjectArgs;
use crate::providers::gcp::clients::serviceconsumermanagement::ServiceconsumermanagementServicesTenancyUnitsApplyProjectConfigArgs;
use crate::providers::gcp::clients::serviceconsumermanagement::ServiceconsumermanagementServicesTenancyUnitsAttachProjectArgs;
use crate::providers::gcp::clients::serviceconsumermanagement::ServiceconsumermanagementServicesTenancyUnitsCreateArgs;
use crate::providers::gcp::clients::serviceconsumermanagement::ServiceconsumermanagementServicesTenancyUnitsDeleteArgs;
use crate::providers::gcp::clients::serviceconsumermanagement::ServiceconsumermanagementServicesTenancyUnitsDeleteProjectArgs;
use crate::providers::gcp::clients::serviceconsumermanagement::ServiceconsumermanagementServicesTenancyUnitsListArgs;
use crate::providers::gcp::clients::serviceconsumermanagement::ServiceconsumermanagementServicesTenancyUnitsRemoveProjectArgs;
use crate::providers::gcp::clients::serviceconsumermanagement::ServiceconsumermanagementServicesTenancyUnitsUndeleteProjectArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ServiceconsumermanagementProvider with automatic state tracking.
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
/// let provider = ServiceconsumermanagementProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ServiceconsumermanagementProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ServiceconsumermanagementProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ServiceconsumermanagementProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Serviceconsumermanagement operations cancel.
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
    pub fn serviceconsumermanagement_operations_cancel(
        &self,
        args: &ServiceconsumermanagementOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = serviceconsumermanagement_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = serviceconsumermanagement_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Serviceconsumermanagement operations delete.
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
    pub fn serviceconsumermanagement_operations_delete(
        &self,
        args: &ServiceconsumermanagementOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = serviceconsumermanagement_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = serviceconsumermanagement_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Serviceconsumermanagement operations get.
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
    pub fn serviceconsumermanagement_operations_get(
        &self,
        args: &ServiceconsumermanagementOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = serviceconsumermanagement_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = serviceconsumermanagement_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Serviceconsumermanagement operations list.
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
    pub fn serviceconsumermanagement_operations_list(
        &self,
        args: &ServiceconsumermanagementOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = serviceconsumermanagement_operations_list_builder(
            &self.http_client,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = serviceconsumermanagement_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Serviceconsumermanagement services search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchTenancyUnitsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn serviceconsumermanagement_services_search(
        &self,
        args: &ServiceconsumermanagementServicesSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchTenancyUnitsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = serviceconsumermanagement_services_search_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.query,
        )
        .map_err(ProviderError::Api)?;

        let task = serviceconsumermanagement_services_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Serviceconsumermanagement services tenancy units add project.
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
    pub fn serviceconsumermanagement_services_tenancy_units_add_project(
        &self,
        args: &ServiceconsumermanagementServicesTenancyUnitsAddProjectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = serviceconsumermanagement_services_tenancy_units_add_project_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = serviceconsumermanagement_services_tenancy_units_add_project_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Serviceconsumermanagement services tenancy units apply project config.
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
    pub fn serviceconsumermanagement_services_tenancy_units_apply_project_config(
        &self,
        args: &ServiceconsumermanagementServicesTenancyUnitsApplyProjectConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = serviceconsumermanagement_services_tenancy_units_apply_project_config_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = serviceconsumermanagement_services_tenancy_units_apply_project_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Serviceconsumermanagement services tenancy units attach project.
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
    pub fn serviceconsumermanagement_services_tenancy_units_attach_project(
        &self,
        args: &ServiceconsumermanagementServicesTenancyUnitsAttachProjectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = serviceconsumermanagement_services_tenancy_units_attach_project_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = serviceconsumermanagement_services_tenancy_units_attach_project_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Serviceconsumermanagement services tenancy units create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TenancyUnit result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn serviceconsumermanagement_services_tenancy_units_create(
        &self,
        args: &ServiceconsumermanagementServicesTenancyUnitsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TenancyUnit, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = serviceconsumermanagement_services_tenancy_units_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = serviceconsumermanagement_services_tenancy_units_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Serviceconsumermanagement services tenancy units delete.
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
    pub fn serviceconsumermanagement_services_tenancy_units_delete(
        &self,
        args: &ServiceconsumermanagementServicesTenancyUnitsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = serviceconsumermanagement_services_tenancy_units_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = serviceconsumermanagement_services_tenancy_units_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Serviceconsumermanagement services tenancy units delete project.
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
    pub fn serviceconsumermanagement_services_tenancy_units_delete_project(
        &self,
        args: &ServiceconsumermanagementServicesTenancyUnitsDeleteProjectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = serviceconsumermanagement_services_tenancy_units_delete_project_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = serviceconsumermanagement_services_tenancy_units_delete_project_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Serviceconsumermanagement services tenancy units list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTenancyUnitsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn serviceconsumermanagement_services_tenancy_units_list(
        &self,
        args: &ServiceconsumermanagementServicesTenancyUnitsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTenancyUnitsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = serviceconsumermanagement_services_tenancy_units_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = serviceconsumermanagement_services_tenancy_units_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Serviceconsumermanagement services tenancy units remove project.
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
    pub fn serviceconsumermanagement_services_tenancy_units_remove_project(
        &self,
        args: &ServiceconsumermanagementServicesTenancyUnitsRemoveProjectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = serviceconsumermanagement_services_tenancy_units_remove_project_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = serviceconsumermanagement_services_tenancy_units_remove_project_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Serviceconsumermanagement services tenancy units undelete project.
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
    pub fn serviceconsumermanagement_services_tenancy_units_undelete_project(
        &self,
        args: &ServiceconsumermanagementServicesTenancyUnitsUndeleteProjectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = serviceconsumermanagement_services_tenancy_units_undelete_project_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = serviceconsumermanagement_services_tenancy_units_undelete_project_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
