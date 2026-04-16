//! ContentwarehouseProvider - State-aware contentwarehouse API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       contentwarehouse API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::contentwarehouse::{
    contentwarehouse_projects_fetch_acl_builder, contentwarehouse_projects_fetch_acl_task,
    contentwarehouse_projects_set_acl_builder, contentwarehouse_projects_set_acl_task,
    contentwarehouse_projects_locations_get_status_builder, contentwarehouse_projects_locations_get_status_task,
    contentwarehouse_projects_locations_initialize_builder, contentwarehouse_projects_locations_initialize_task,
    contentwarehouse_projects_locations_run_pipeline_builder, contentwarehouse_projects_locations_run_pipeline_task,
    contentwarehouse_projects_locations_document_schemas_create_builder, contentwarehouse_projects_locations_document_schemas_create_task,
    contentwarehouse_projects_locations_document_schemas_delete_builder, contentwarehouse_projects_locations_document_schemas_delete_task,
    contentwarehouse_projects_locations_document_schemas_get_builder, contentwarehouse_projects_locations_document_schemas_get_task,
    contentwarehouse_projects_locations_document_schemas_list_builder, contentwarehouse_projects_locations_document_schemas_list_task,
    contentwarehouse_projects_locations_document_schemas_patch_builder, contentwarehouse_projects_locations_document_schemas_patch_task,
    contentwarehouse_projects_locations_documents_create_builder, contentwarehouse_projects_locations_documents_create_task,
    contentwarehouse_projects_locations_documents_delete_builder, contentwarehouse_projects_locations_documents_delete_task,
    contentwarehouse_projects_locations_documents_fetch_acl_builder, contentwarehouse_projects_locations_documents_fetch_acl_task,
    contentwarehouse_projects_locations_documents_get_builder, contentwarehouse_projects_locations_documents_get_task,
    contentwarehouse_projects_locations_documents_linked_sources_builder, contentwarehouse_projects_locations_documents_linked_sources_task,
    contentwarehouse_projects_locations_documents_linked_targets_builder, contentwarehouse_projects_locations_documents_linked_targets_task,
    contentwarehouse_projects_locations_documents_lock_builder, contentwarehouse_projects_locations_documents_lock_task,
    contentwarehouse_projects_locations_documents_patch_builder, contentwarehouse_projects_locations_documents_patch_task,
    contentwarehouse_projects_locations_documents_search_builder, contentwarehouse_projects_locations_documents_search_task,
    contentwarehouse_projects_locations_documents_set_acl_builder, contentwarehouse_projects_locations_documents_set_acl_task,
    contentwarehouse_projects_locations_documents_document_links_create_builder, contentwarehouse_projects_locations_documents_document_links_create_task,
    contentwarehouse_projects_locations_documents_document_links_delete_builder, contentwarehouse_projects_locations_documents_document_links_delete_task,
    contentwarehouse_projects_locations_documents_reference_id_delete_builder, contentwarehouse_projects_locations_documents_reference_id_delete_task,
    contentwarehouse_projects_locations_documents_reference_id_get_builder, contentwarehouse_projects_locations_documents_reference_id_get_task,
    contentwarehouse_projects_locations_documents_reference_id_patch_builder, contentwarehouse_projects_locations_documents_reference_id_patch_task,
    contentwarehouse_projects_locations_operations_get_builder, contentwarehouse_projects_locations_operations_get_task,
    contentwarehouse_projects_locations_rule_sets_create_builder, contentwarehouse_projects_locations_rule_sets_create_task,
    contentwarehouse_projects_locations_rule_sets_delete_builder, contentwarehouse_projects_locations_rule_sets_delete_task,
    contentwarehouse_projects_locations_rule_sets_get_builder, contentwarehouse_projects_locations_rule_sets_get_task,
    contentwarehouse_projects_locations_rule_sets_list_builder, contentwarehouse_projects_locations_rule_sets_list_task,
    contentwarehouse_projects_locations_rule_sets_patch_builder, contentwarehouse_projects_locations_rule_sets_patch_task,
    contentwarehouse_projects_locations_synonym_sets_create_builder, contentwarehouse_projects_locations_synonym_sets_create_task,
    contentwarehouse_projects_locations_synonym_sets_delete_builder, contentwarehouse_projects_locations_synonym_sets_delete_task,
    contentwarehouse_projects_locations_synonym_sets_get_builder, contentwarehouse_projects_locations_synonym_sets_get_task,
    contentwarehouse_projects_locations_synonym_sets_list_builder, contentwarehouse_projects_locations_synonym_sets_list_task,
    contentwarehouse_projects_locations_synonym_sets_patch_builder, contentwarehouse_projects_locations_synonym_sets_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::contentwarehouse::GoogleCloudContentwarehouseV1CreateDocumentResponse;
use crate::providers::gcp::clients::contentwarehouse::GoogleCloudContentwarehouseV1Document;
use crate::providers::gcp::clients::contentwarehouse::GoogleCloudContentwarehouseV1DocumentLink;
use crate::providers::gcp::clients::contentwarehouse::GoogleCloudContentwarehouseV1DocumentSchema;
use crate::providers::gcp::clients::contentwarehouse::GoogleCloudContentwarehouseV1FetchAclResponse;
use crate::providers::gcp::clients::contentwarehouse::GoogleCloudContentwarehouseV1ListDocumentSchemasResponse;
use crate::providers::gcp::clients::contentwarehouse::GoogleCloudContentwarehouseV1ListLinkedSourcesResponse;
use crate::providers::gcp::clients::contentwarehouse::GoogleCloudContentwarehouseV1ListLinkedTargetsResponse;
use crate::providers::gcp::clients::contentwarehouse::GoogleCloudContentwarehouseV1ListRuleSetsResponse;
use crate::providers::gcp::clients::contentwarehouse::GoogleCloudContentwarehouseV1ListSynonymSetsResponse;
use crate::providers::gcp::clients::contentwarehouse::GoogleCloudContentwarehouseV1ProjectStatus;
use crate::providers::gcp::clients::contentwarehouse::GoogleCloudContentwarehouseV1RuleSet;
use crate::providers::gcp::clients::contentwarehouse::GoogleCloudContentwarehouseV1SearchDocumentsResponse;
use crate::providers::gcp::clients::contentwarehouse::GoogleCloudContentwarehouseV1SetAclResponse;
use crate::providers::gcp::clients::contentwarehouse::GoogleCloudContentwarehouseV1SynonymSet;
use crate::providers::gcp::clients::contentwarehouse::GoogleCloudContentwarehouseV1UpdateDocumentResponse;
use crate::providers::gcp::clients::contentwarehouse::GoogleLongrunningOperation;
use crate::providers::gcp::clients::contentwarehouse::GoogleProtobufEmpty;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsFetchAclArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsDocumentSchemasCreateArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsDocumentSchemasDeleteArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsDocumentSchemasGetArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsDocumentSchemasListArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsDocumentSchemasPatchArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsDocumentsCreateArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsDocumentsDeleteArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsDocumentsDocumentLinksCreateArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsDocumentsDocumentLinksDeleteArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsDocumentsFetchAclArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsDocumentsGetArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsDocumentsLinkedSourcesArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsDocumentsLinkedTargetsArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsDocumentsLockArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsDocumentsPatchArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsDocumentsReferenceIdDeleteArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsDocumentsReferenceIdGetArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsDocumentsReferenceIdPatchArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsDocumentsSearchArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsDocumentsSetAclArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsGetStatusArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsInitializeArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsRuleSetsCreateArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsRuleSetsDeleteArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsRuleSetsGetArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsRuleSetsListArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsRuleSetsPatchArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsRunPipelineArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsSynonymSetsCreateArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsSynonymSetsDeleteArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsSynonymSetsGetArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsSynonymSetsListArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsLocationsSynonymSetsPatchArgs;
use crate::providers::gcp::clients::contentwarehouse::ContentwarehouseProjectsSetAclArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ContentwarehouseProvider with automatic state tracking.
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
/// let provider = ContentwarehouseProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct ContentwarehouseProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> ContentwarehouseProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new ContentwarehouseProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new ContentwarehouseProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Contentwarehouse projects fetch acl.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudContentwarehouseV1FetchAclResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn contentwarehouse_projects_fetch_acl(
        &self,
        args: &ContentwarehouseProjectsFetchAclArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudContentwarehouseV1FetchAclResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_fetch_acl_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_fetch_acl_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects set acl.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudContentwarehouseV1SetAclResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn contentwarehouse_projects_set_acl(
        &self,
        args: &ContentwarehouseProjectsSetAclArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudContentwarehouseV1SetAclResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_set_acl_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_set_acl_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations get status.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudContentwarehouseV1ProjectStatus result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn contentwarehouse_projects_locations_get_status(
        &self,
        args: &ContentwarehouseProjectsLocationsGetStatusArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudContentwarehouseV1ProjectStatus, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_get_status_builder(
            &self.http_client,
            &args.location,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_get_status_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations initialize.
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
    pub fn contentwarehouse_projects_locations_initialize(
        &self,
        args: &ContentwarehouseProjectsLocationsInitializeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_initialize_builder(
            &self.http_client,
            &args.location,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_initialize_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations run pipeline.
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
    pub fn contentwarehouse_projects_locations_run_pipeline(
        &self,
        args: &ContentwarehouseProjectsLocationsRunPipelineArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_run_pipeline_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_run_pipeline_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations document schemas create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudContentwarehouseV1DocumentSchema result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn contentwarehouse_projects_locations_document_schemas_create(
        &self,
        args: &ContentwarehouseProjectsLocationsDocumentSchemasCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudContentwarehouseV1DocumentSchema, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_document_schemas_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_document_schemas_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations document schemas delete.
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
    pub fn contentwarehouse_projects_locations_document_schemas_delete(
        &self,
        args: &ContentwarehouseProjectsLocationsDocumentSchemasDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_document_schemas_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_document_schemas_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations document schemas get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudContentwarehouseV1DocumentSchema result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn contentwarehouse_projects_locations_document_schemas_get(
        &self,
        args: &ContentwarehouseProjectsLocationsDocumentSchemasGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudContentwarehouseV1DocumentSchema, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_document_schemas_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_document_schemas_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations document schemas list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudContentwarehouseV1ListDocumentSchemasResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn contentwarehouse_projects_locations_document_schemas_list(
        &self,
        args: &ContentwarehouseProjectsLocationsDocumentSchemasListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudContentwarehouseV1ListDocumentSchemasResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_document_schemas_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_document_schemas_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations document schemas patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudContentwarehouseV1DocumentSchema result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn contentwarehouse_projects_locations_document_schemas_patch(
        &self,
        args: &ContentwarehouseProjectsLocationsDocumentSchemasPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudContentwarehouseV1DocumentSchema, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_document_schemas_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_document_schemas_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations documents create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudContentwarehouseV1CreateDocumentResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn contentwarehouse_projects_locations_documents_create(
        &self,
        args: &ContentwarehouseProjectsLocationsDocumentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudContentwarehouseV1CreateDocumentResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_documents_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_documents_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations documents delete.
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
    pub fn contentwarehouse_projects_locations_documents_delete(
        &self,
        args: &ContentwarehouseProjectsLocationsDocumentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_documents_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_documents_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations documents fetch acl.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudContentwarehouseV1FetchAclResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn contentwarehouse_projects_locations_documents_fetch_acl(
        &self,
        args: &ContentwarehouseProjectsLocationsDocumentsFetchAclArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudContentwarehouseV1FetchAclResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_documents_fetch_acl_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_documents_fetch_acl_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations documents get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudContentwarehouseV1Document result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn contentwarehouse_projects_locations_documents_get(
        &self,
        args: &ContentwarehouseProjectsLocationsDocumentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudContentwarehouseV1Document, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_documents_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_documents_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations documents linked sources.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudContentwarehouseV1ListLinkedSourcesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn contentwarehouse_projects_locations_documents_linked_sources(
        &self,
        args: &ContentwarehouseProjectsLocationsDocumentsLinkedSourcesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudContentwarehouseV1ListLinkedSourcesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_documents_linked_sources_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_documents_linked_sources_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations documents linked targets.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudContentwarehouseV1ListLinkedTargetsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn contentwarehouse_projects_locations_documents_linked_targets(
        &self,
        args: &ContentwarehouseProjectsLocationsDocumentsLinkedTargetsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudContentwarehouseV1ListLinkedTargetsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_documents_linked_targets_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_documents_linked_targets_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations documents lock.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudContentwarehouseV1Document result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn contentwarehouse_projects_locations_documents_lock(
        &self,
        args: &ContentwarehouseProjectsLocationsDocumentsLockArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudContentwarehouseV1Document, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_documents_lock_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_documents_lock_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations documents patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudContentwarehouseV1UpdateDocumentResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn contentwarehouse_projects_locations_documents_patch(
        &self,
        args: &ContentwarehouseProjectsLocationsDocumentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudContentwarehouseV1UpdateDocumentResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_documents_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_documents_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations documents search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudContentwarehouseV1SearchDocumentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn contentwarehouse_projects_locations_documents_search(
        &self,
        args: &ContentwarehouseProjectsLocationsDocumentsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudContentwarehouseV1SearchDocumentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_documents_search_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_documents_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations documents set acl.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudContentwarehouseV1SetAclResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn contentwarehouse_projects_locations_documents_set_acl(
        &self,
        args: &ContentwarehouseProjectsLocationsDocumentsSetAclArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudContentwarehouseV1SetAclResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_documents_set_acl_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_documents_set_acl_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations documents document links create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudContentwarehouseV1DocumentLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn contentwarehouse_projects_locations_documents_document_links_create(
        &self,
        args: &ContentwarehouseProjectsLocationsDocumentsDocumentLinksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudContentwarehouseV1DocumentLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_documents_document_links_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_documents_document_links_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations documents document links delete.
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
    pub fn contentwarehouse_projects_locations_documents_document_links_delete(
        &self,
        args: &ContentwarehouseProjectsLocationsDocumentsDocumentLinksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_documents_document_links_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_documents_document_links_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations documents reference id delete.
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
    pub fn contentwarehouse_projects_locations_documents_reference_id_delete(
        &self,
        args: &ContentwarehouseProjectsLocationsDocumentsReferenceIdDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_documents_reference_id_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_documents_reference_id_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations documents reference id get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudContentwarehouseV1Document result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn contentwarehouse_projects_locations_documents_reference_id_get(
        &self,
        args: &ContentwarehouseProjectsLocationsDocumentsReferenceIdGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudContentwarehouseV1Document, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_documents_reference_id_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_documents_reference_id_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations documents reference id patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudContentwarehouseV1UpdateDocumentResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn contentwarehouse_projects_locations_documents_reference_id_patch(
        &self,
        args: &ContentwarehouseProjectsLocationsDocumentsReferenceIdPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudContentwarehouseV1UpdateDocumentResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_documents_reference_id_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_documents_reference_id_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations operations get.
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
    pub fn contentwarehouse_projects_locations_operations_get(
        &self,
        args: &ContentwarehouseProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations rule sets create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudContentwarehouseV1RuleSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn contentwarehouse_projects_locations_rule_sets_create(
        &self,
        args: &ContentwarehouseProjectsLocationsRuleSetsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudContentwarehouseV1RuleSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_rule_sets_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_rule_sets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations rule sets delete.
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
    pub fn contentwarehouse_projects_locations_rule_sets_delete(
        &self,
        args: &ContentwarehouseProjectsLocationsRuleSetsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_rule_sets_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_rule_sets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations rule sets get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudContentwarehouseV1RuleSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn contentwarehouse_projects_locations_rule_sets_get(
        &self,
        args: &ContentwarehouseProjectsLocationsRuleSetsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudContentwarehouseV1RuleSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_rule_sets_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_rule_sets_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations rule sets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudContentwarehouseV1ListRuleSetsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn contentwarehouse_projects_locations_rule_sets_list(
        &self,
        args: &ContentwarehouseProjectsLocationsRuleSetsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudContentwarehouseV1ListRuleSetsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_rule_sets_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_rule_sets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations rule sets patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudContentwarehouseV1RuleSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn contentwarehouse_projects_locations_rule_sets_patch(
        &self,
        args: &ContentwarehouseProjectsLocationsRuleSetsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudContentwarehouseV1RuleSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_rule_sets_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_rule_sets_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations synonym sets create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudContentwarehouseV1SynonymSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn contentwarehouse_projects_locations_synonym_sets_create(
        &self,
        args: &ContentwarehouseProjectsLocationsSynonymSetsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudContentwarehouseV1SynonymSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_synonym_sets_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_synonym_sets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations synonym sets delete.
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
    pub fn contentwarehouse_projects_locations_synonym_sets_delete(
        &self,
        args: &ContentwarehouseProjectsLocationsSynonymSetsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_synonym_sets_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_synonym_sets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations synonym sets get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudContentwarehouseV1SynonymSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn contentwarehouse_projects_locations_synonym_sets_get(
        &self,
        args: &ContentwarehouseProjectsLocationsSynonymSetsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudContentwarehouseV1SynonymSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_synonym_sets_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_synonym_sets_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations synonym sets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudContentwarehouseV1ListSynonymSetsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn contentwarehouse_projects_locations_synonym_sets_list(
        &self,
        args: &ContentwarehouseProjectsLocationsSynonymSetsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudContentwarehouseV1ListSynonymSetsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_synonym_sets_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_synonym_sets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Contentwarehouse projects locations synonym sets patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudContentwarehouseV1SynonymSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn contentwarehouse_projects_locations_synonym_sets_patch(
        &self,
        args: &ContentwarehouseProjectsLocationsSynonymSetsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudContentwarehouseV1SynonymSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = contentwarehouse_projects_locations_synonym_sets_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = contentwarehouse_projects_locations_synonym_sets_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
