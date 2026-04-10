//! VaultProvider - State-aware vault API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       vault API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::vault::{
    vault_matters_add_permissions_builder, vault_matters_add_permissions_task,
    vault_matters_close_builder, vault_matters_close_task,
    vault_matters_count_builder, vault_matters_count_task,
    vault_matters_create_builder, vault_matters_create_task,
    vault_matters_delete_builder, vault_matters_delete_task,
    vault_matters_get_builder, vault_matters_get_task,
    vault_matters_list_builder, vault_matters_list_task,
    vault_matters_remove_permissions_builder, vault_matters_remove_permissions_task,
    vault_matters_reopen_builder, vault_matters_reopen_task,
    vault_matters_undelete_builder, vault_matters_undelete_task,
    vault_matters_update_builder, vault_matters_update_task,
    vault_matters_exports_create_builder, vault_matters_exports_create_task,
    vault_matters_exports_delete_builder, vault_matters_exports_delete_task,
    vault_matters_exports_get_builder, vault_matters_exports_get_task,
    vault_matters_exports_list_builder, vault_matters_exports_list_task,
    vault_matters_holds_add_held_accounts_builder, vault_matters_holds_add_held_accounts_task,
    vault_matters_holds_create_builder, vault_matters_holds_create_task,
    vault_matters_holds_delete_builder, vault_matters_holds_delete_task,
    vault_matters_holds_get_builder, vault_matters_holds_get_task,
    vault_matters_holds_list_builder, vault_matters_holds_list_task,
    vault_matters_holds_remove_held_accounts_builder, vault_matters_holds_remove_held_accounts_task,
    vault_matters_holds_update_builder, vault_matters_holds_update_task,
    vault_matters_holds_accounts_create_builder, vault_matters_holds_accounts_create_task,
    vault_matters_holds_accounts_delete_builder, vault_matters_holds_accounts_delete_task,
    vault_matters_holds_accounts_list_builder, vault_matters_holds_accounts_list_task,
    vault_matters_saved_queries_create_builder, vault_matters_saved_queries_create_task,
    vault_matters_saved_queries_delete_builder, vault_matters_saved_queries_delete_task,
    vault_matters_saved_queries_get_builder, vault_matters_saved_queries_get_task,
    vault_matters_saved_queries_list_builder, vault_matters_saved_queries_list_task,
    vault_operations_cancel_builder, vault_operations_cancel_task,
    vault_operations_delete_builder, vault_operations_delete_task,
    vault_operations_get_builder, vault_operations_get_task,
    vault_operations_list_builder, vault_operations_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::vault::AddHeldAccountsResponse;
use crate::providers::gcp::clients::vault::CloseMatterResponse;
use crate::providers::gcp::clients::vault::Empty;
use crate::providers::gcp::clients::vault::Export;
use crate::providers::gcp::clients::vault::HeldAccount;
use crate::providers::gcp::clients::vault::Hold;
use crate::providers::gcp::clients::vault::ListExportsResponse;
use crate::providers::gcp::clients::vault::ListHeldAccountsResponse;
use crate::providers::gcp::clients::vault::ListHoldsResponse;
use crate::providers::gcp::clients::vault::ListMattersResponse;
use crate::providers::gcp::clients::vault::ListOperationsResponse;
use crate::providers::gcp::clients::vault::ListSavedQueriesResponse;
use crate::providers::gcp::clients::vault::Matter;
use crate::providers::gcp::clients::vault::MatterPermission;
use crate::providers::gcp::clients::vault::Operation;
use crate::providers::gcp::clients::vault::RemoveHeldAccountsResponse;
use crate::providers::gcp::clients::vault::ReopenMatterResponse;
use crate::providers::gcp::clients::vault::SavedQuery;
use crate::providers::gcp::clients::vault::VaultMattersAddPermissionsArgs;
use crate::providers::gcp::clients::vault::VaultMattersCloseArgs;
use crate::providers::gcp::clients::vault::VaultMattersCountArgs;
use crate::providers::gcp::clients::vault::VaultMattersCreateArgs;
use crate::providers::gcp::clients::vault::VaultMattersDeleteArgs;
use crate::providers::gcp::clients::vault::VaultMattersExportsCreateArgs;
use crate::providers::gcp::clients::vault::VaultMattersExportsDeleteArgs;
use crate::providers::gcp::clients::vault::VaultMattersExportsGetArgs;
use crate::providers::gcp::clients::vault::VaultMattersExportsListArgs;
use crate::providers::gcp::clients::vault::VaultMattersGetArgs;
use crate::providers::gcp::clients::vault::VaultMattersHoldsAccountsCreateArgs;
use crate::providers::gcp::clients::vault::VaultMattersHoldsAccountsDeleteArgs;
use crate::providers::gcp::clients::vault::VaultMattersHoldsAccountsListArgs;
use crate::providers::gcp::clients::vault::VaultMattersHoldsAddHeldAccountsArgs;
use crate::providers::gcp::clients::vault::VaultMattersHoldsCreateArgs;
use crate::providers::gcp::clients::vault::VaultMattersHoldsDeleteArgs;
use crate::providers::gcp::clients::vault::VaultMattersHoldsGetArgs;
use crate::providers::gcp::clients::vault::VaultMattersHoldsListArgs;
use crate::providers::gcp::clients::vault::VaultMattersHoldsRemoveHeldAccountsArgs;
use crate::providers::gcp::clients::vault::VaultMattersHoldsUpdateArgs;
use crate::providers::gcp::clients::vault::VaultMattersListArgs;
use crate::providers::gcp::clients::vault::VaultMattersRemovePermissionsArgs;
use crate::providers::gcp::clients::vault::VaultMattersReopenArgs;
use crate::providers::gcp::clients::vault::VaultMattersSavedQueriesCreateArgs;
use crate::providers::gcp::clients::vault::VaultMattersSavedQueriesDeleteArgs;
use crate::providers::gcp::clients::vault::VaultMattersSavedQueriesGetArgs;
use crate::providers::gcp::clients::vault::VaultMattersSavedQueriesListArgs;
use crate::providers::gcp::clients::vault::VaultMattersUndeleteArgs;
use crate::providers::gcp::clients::vault::VaultMattersUpdateArgs;
use crate::providers::gcp::clients::vault::VaultOperationsCancelArgs;
use crate::providers::gcp::clients::vault::VaultOperationsDeleteArgs;
use crate::providers::gcp::clients::vault::VaultOperationsGetArgs;
use crate::providers::gcp::clients::vault::VaultOperationsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// VaultProvider with automatic state tracking.
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
/// let provider = VaultProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct VaultProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> VaultProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new VaultProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Vault matters add permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MatterPermission result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vault_matters_add_permissions(
        &self,
        args: &VaultMattersAddPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MatterPermission, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_matters_add_permissions_builder(
            &self.http_client,
            &args.matterId,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_matters_add_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault matters close.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CloseMatterResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vault_matters_close(
        &self,
        args: &VaultMattersCloseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CloseMatterResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_matters_close_builder(
            &self.http_client,
            &args.matterId,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_matters_close_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault matters count.
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
    pub fn vault_matters_count(
        &self,
        args: &VaultMattersCountArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_matters_count_builder(
            &self.http_client,
            &args.matterId,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_matters_count_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault matters create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Matter result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vault_matters_create(
        &self,
        args: &VaultMattersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Matter, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_matters_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_matters_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault matters delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Matter result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vault_matters_delete(
        &self,
        args: &VaultMattersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Matter, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_matters_delete_builder(
            &self.http_client,
            &args.matterId,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_matters_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault matters get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Matter result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vault_matters_get(
        &self,
        args: &VaultMattersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Matter, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_matters_get_builder(
            &self.http_client,
            &args.matterId,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_matters_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault matters list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListMattersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vault_matters_list(
        &self,
        args: &VaultMattersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListMattersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_matters_list_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
            &args.state,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_matters_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault matters remove permissions.
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
    pub fn vault_matters_remove_permissions(
        &self,
        args: &VaultMattersRemovePermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_matters_remove_permissions_builder(
            &self.http_client,
            &args.matterId,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_matters_remove_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault matters reopen.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReopenMatterResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vault_matters_reopen(
        &self,
        args: &VaultMattersReopenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReopenMatterResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_matters_reopen_builder(
            &self.http_client,
            &args.matterId,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_matters_reopen_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault matters undelete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Matter result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vault_matters_undelete(
        &self,
        args: &VaultMattersUndeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Matter, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_matters_undelete_builder(
            &self.http_client,
            &args.matterId,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_matters_undelete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault matters update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Matter result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vault_matters_update(
        &self,
        args: &VaultMattersUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Matter, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_matters_update_builder(
            &self.http_client,
            &args.matterId,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_matters_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault matters exports create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Export result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vault_matters_exports_create(
        &self,
        args: &VaultMattersExportsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Export, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_matters_exports_create_builder(
            &self.http_client,
            &args.matterId,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_matters_exports_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault matters exports delete.
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
    pub fn vault_matters_exports_delete(
        &self,
        args: &VaultMattersExportsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_matters_exports_delete_builder(
            &self.http_client,
            &args.matterId,
            &args.exportId,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_matters_exports_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault matters exports get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Export result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vault_matters_exports_get(
        &self,
        args: &VaultMattersExportsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Export, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_matters_exports_get_builder(
            &self.http_client,
            &args.matterId,
            &args.exportId,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_matters_exports_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault matters exports list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListExportsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vault_matters_exports_list(
        &self,
        args: &VaultMattersExportsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListExportsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_matters_exports_list_builder(
            &self.http_client,
            &args.matterId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_matters_exports_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault matters holds add held accounts.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AddHeldAccountsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vault_matters_holds_add_held_accounts(
        &self,
        args: &VaultMattersHoldsAddHeldAccountsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AddHeldAccountsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_matters_holds_add_held_accounts_builder(
            &self.http_client,
            &args.matterId,
            &args.holdId,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_matters_holds_add_held_accounts_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault matters holds create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Hold result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vault_matters_holds_create(
        &self,
        args: &VaultMattersHoldsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Hold, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_matters_holds_create_builder(
            &self.http_client,
            &args.matterId,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_matters_holds_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault matters holds delete.
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
    pub fn vault_matters_holds_delete(
        &self,
        args: &VaultMattersHoldsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_matters_holds_delete_builder(
            &self.http_client,
            &args.matterId,
            &args.holdId,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_matters_holds_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault matters holds get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Hold result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vault_matters_holds_get(
        &self,
        args: &VaultMattersHoldsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Hold, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_matters_holds_get_builder(
            &self.http_client,
            &args.matterId,
            &args.holdId,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_matters_holds_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault matters holds list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListHoldsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vault_matters_holds_list(
        &self,
        args: &VaultMattersHoldsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListHoldsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_matters_holds_list_builder(
            &self.http_client,
            &args.matterId,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_matters_holds_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault matters holds remove held accounts.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RemoveHeldAccountsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vault_matters_holds_remove_held_accounts(
        &self,
        args: &VaultMattersHoldsRemoveHeldAccountsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RemoveHeldAccountsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_matters_holds_remove_held_accounts_builder(
            &self.http_client,
            &args.matterId,
            &args.holdId,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_matters_holds_remove_held_accounts_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault matters holds update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Hold result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vault_matters_holds_update(
        &self,
        args: &VaultMattersHoldsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Hold, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_matters_holds_update_builder(
            &self.http_client,
            &args.matterId,
            &args.holdId,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_matters_holds_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault matters holds accounts create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HeldAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vault_matters_holds_accounts_create(
        &self,
        args: &VaultMattersHoldsAccountsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HeldAccount, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_matters_holds_accounts_create_builder(
            &self.http_client,
            &args.matterId,
            &args.holdId,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_matters_holds_accounts_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault matters holds accounts delete.
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
    pub fn vault_matters_holds_accounts_delete(
        &self,
        args: &VaultMattersHoldsAccountsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_matters_holds_accounts_delete_builder(
            &self.http_client,
            &args.matterId,
            &args.holdId,
            &args.accountId,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_matters_holds_accounts_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault matters holds accounts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListHeldAccountsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vault_matters_holds_accounts_list(
        &self,
        args: &VaultMattersHoldsAccountsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListHeldAccountsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_matters_holds_accounts_list_builder(
            &self.http_client,
            &args.matterId,
            &args.holdId,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_matters_holds_accounts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault matters saved queries create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SavedQuery result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vault_matters_saved_queries_create(
        &self,
        args: &VaultMattersSavedQueriesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SavedQuery, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_matters_saved_queries_create_builder(
            &self.http_client,
            &args.matterId,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_matters_saved_queries_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault matters saved queries delete.
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
    pub fn vault_matters_saved_queries_delete(
        &self,
        args: &VaultMattersSavedQueriesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_matters_saved_queries_delete_builder(
            &self.http_client,
            &args.matterId,
            &args.savedQueryId,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_matters_saved_queries_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault matters saved queries get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SavedQuery result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vault_matters_saved_queries_get(
        &self,
        args: &VaultMattersSavedQueriesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SavedQuery, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_matters_saved_queries_get_builder(
            &self.http_client,
            &args.matterId,
            &args.savedQueryId,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_matters_saved_queries_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault matters saved queries list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSavedQueriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vault_matters_saved_queries_list(
        &self,
        args: &VaultMattersSavedQueriesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSavedQueriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_matters_saved_queries_list_builder(
            &self.http_client,
            &args.matterId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_matters_saved_queries_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault operations cancel.
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
    pub fn vault_operations_cancel(
        &self,
        args: &VaultOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault operations delete.
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
    pub fn vault_operations_delete(
        &self,
        args: &VaultOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault operations get.
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
    pub fn vault_operations_get(
        &self,
        args: &VaultOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vault operations list.
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
    pub fn vault_operations_list(
        &self,
        args: &VaultOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vault_operations_list_builder(
            &self.http_client,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = vault_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
