//! OsloginProvider - State-aware oslogin API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       oslogin API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::oslogin::{
    oslogin_projects_locations_sign_ssh_public_key_builder, oslogin_projects_locations_sign_ssh_public_key_task,
    oslogin_users_import_ssh_public_key_builder, oslogin_users_import_ssh_public_key_task,
    oslogin_users_projects_delete_builder, oslogin_users_projects_delete_task,
    oslogin_users_projects_provision_posix_account_builder, oslogin_users_projects_provision_posix_account_task,
    oslogin_users_ssh_public_keys_create_builder, oslogin_users_ssh_public_keys_create_task,
    oslogin_users_ssh_public_keys_delete_builder, oslogin_users_ssh_public_keys_delete_task,
    oslogin_users_ssh_public_keys_patch_builder, oslogin_users_ssh_public_keys_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::oslogin::Empty;
use crate::providers::gcp::clients::oslogin::ImportSshPublicKeyResponse;
use crate::providers::gcp::clients::oslogin::PosixAccount;
use crate::providers::gcp::clients::oslogin::SignSshPublicKeyResponse;
use crate::providers::gcp::clients::oslogin::SshPublicKey;
use crate::providers::gcp::clients::oslogin::OsloginProjectsLocationsSignSshPublicKeyArgs;
use crate::providers::gcp::clients::oslogin::OsloginUsersImportSshPublicKeyArgs;
use crate::providers::gcp::clients::oslogin::OsloginUsersProjectsDeleteArgs;
use crate::providers::gcp::clients::oslogin::OsloginUsersProjectsProvisionPosixAccountArgs;
use crate::providers::gcp::clients::oslogin::OsloginUsersSshPublicKeysCreateArgs;
use crate::providers::gcp::clients::oslogin::OsloginUsersSshPublicKeysDeleteArgs;
use crate::providers::gcp::clients::oslogin::OsloginUsersSshPublicKeysPatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// OsloginProvider with automatic state tracking.
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
/// let provider = OsloginProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct OsloginProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> OsloginProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new OsloginProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Oslogin projects locations sign ssh public key.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SignSshPublicKeyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oslogin_projects_locations_sign_ssh_public_key(
        &self,
        args: &OsloginProjectsLocationsSignSshPublicKeyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SignSshPublicKeyResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oslogin_projects_locations_sign_ssh_public_key_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = oslogin_projects_locations_sign_ssh_public_key_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oslogin users import ssh public key.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ImportSshPublicKeyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oslogin_users_import_ssh_public_key(
        &self,
        args: &OsloginUsersImportSshPublicKeyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ImportSshPublicKeyResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oslogin_users_import_ssh_public_key_builder(
            &self.http_client,
            &args.parent,
            &args.projectId,
            &args.regions,
        )
        .map_err(ProviderError::Api)?;

        let task = oslogin_users_import_ssh_public_key_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oslogin users projects delete.
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
    pub fn oslogin_users_projects_delete(
        &self,
        args: &OsloginUsersProjectsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oslogin_users_projects_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oslogin_users_projects_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oslogin users projects provision posix account.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PosixAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oslogin_users_projects_provision_posix_account(
        &self,
        args: &OsloginUsersProjectsProvisionPosixAccountArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PosixAccount, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oslogin_users_projects_provision_posix_account_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oslogin_users_projects_provision_posix_account_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oslogin users ssh public keys create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SshPublicKey result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oslogin_users_ssh_public_keys_create(
        &self,
        args: &OsloginUsersSshPublicKeysCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SshPublicKey, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oslogin_users_ssh_public_keys_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = oslogin_users_ssh_public_keys_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oslogin users ssh public keys delete.
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
    pub fn oslogin_users_ssh_public_keys_delete(
        &self,
        args: &OsloginUsersSshPublicKeysDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oslogin_users_ssh_public_keys_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oslogin_users_ssh_public_keys_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oslogin users ssh public keys patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SshPublicKey result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oslogin_users_ssh_public_keys_patch(
        &self,
        args: &OsloginUsersSshPublicKeysPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SshPublicKey, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oslogin_users_ssh_public_keys_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = oslogin_users_ssh_public_keys_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
