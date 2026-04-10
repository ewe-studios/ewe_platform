//! DatacatalogProvider - State-aware datacatalog API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       datacatalog API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::datacatalog::{
    datacatalog_catalog_search_builder, datacatalog_catalog_search_task,
    datacatalog_organizations_locations_set_config_builder, datacatalog_organizations_locations_set_config_task,
    datacatalog_projects_locations_set_config_builder, datacatalog_projects_locations_set_config_task,
    datacatalog_projects_locations_entry_groups_create_builder, datacatalog_projects_locations_entry_groups_create_task,
    datacatalog_projects_locations_entry_groups_delete_builder, datacatalog_projects_locations_entry_groups_delete_task,
    datacatalog_projects_locations_entry_groups_get_iam_policy_builder, datacatalog_projects_locations_entry_groups_get_iam_policy_task,
    datacatalog_projects_locations_entry_groups_patch_builder, datacatalog_projects_locations_entry_groups_patch_task,
    datacatalog_projects_locations_entry_groups_set_iam_policy_builder, datacatalog_projects_locations_entry_groups_set_iam_policy_task,
    datacatalog_projects_locations_entry_groups_test_iam_permissions_builder, datacatalog_projects_locations_entry_groups_test_iam_permissions_task,
    datacatalog_projects_locations_entry_groups_entries_create_builder, datacatalog_projects_locations_entry_groups_entries_create_task,
    datacatalog_projects_locations_entry_groups_entries_delete_builder, datacatalog_projects_locations_entry_groups_entries_delete_task,
    datacatalog_projects_locations_entry_groups_entries_get_iam_policy_builder, datacatalog_projects_locations_entry_groups_entries_get_iam_policy_task,
    datacatalog_projects_locations_entry_groups_entries_import_builder, datacatalog_projects_locations_entry_groups_entries_import_task,
    datacatalog_projects_locations_entry_groups_entries_modify_entry_contacts_builder, datacatalog_projects_locations_entry_groups_entries_modify_entry_contacts_task,
    datacatalog_projects_locations_entry_groups_entries_modify_entry_overview_builder, datacatalog_projects_locations_entry_groups_entries_modify_entry_overview_task,
    datacatalog_projects_locations_entry_groups_entries_patch_builder, datacatalog_projects_locations_entry_groups_entries_patch_task,
    datacatalog_projects_locations_entry_groups_entries_star_builder, datacatalog_projects_locations_entry_groups_entries_star_task,
    datacatalog_projects_locations_entry_groups_entries_test_iam_permissions_builder, datacatalog_projects_locations_entry_groups_entries_test_iam_permissions_task,
    datacatalog_projects_locations_entry_groups_entries_unstar_builder, datacatalog_projects_locations_entry_groups_entries_unstar_task,
    datacatalog_projects_locations_entry_groups_entries_tags_create_builder, datacatalog_projects_locations_entry_groups_entries_tags_create_task,
    datacatalog_projects_locations_entry_groups_entries_tags_delete_builder, datacatalog_projects_locations_entry_groups_entries_tags_delete_task,
    datacatalog_projects_locations_entry_groups_entries_tags_patch_builder, datacatalog_projects_locations_entry_groups_entries_tags_patch_task,
    datacatalog_projects_locations_entry_groups_entries_tags_reconcile_builder, datacatalog_projects_locations_entry_groups_entries_tags_reconcile_task,
    datacatalog_projects_locations_entry_groups_tags_create_builder, datacatalog_projects_locations_entry_groups_tags_create_task,
    datacatalog_projects_locations_entry_groups_tags_delete_builder, datacatalog_projects_locations_entry_groups_tags_delete_task,
    datacatalog_projects_locations_entry_groups_tags_patch_builder, datacatalog_projects_locations_entry_groups_tags_patch_task,
    datacatalog_projects_locations_operations_cancel_builder, datacatalog_projects_locations_operations_cancel_task,
    datacatalog_projects_locations_operations_delete_builder, datacatalog_projects_locations_operations_delete_task,
    datacatalog_projects_locations_tag_templates_create_builder, datacatalog_projects_locations_tag_templates_create_task,
    datacatalog_projects_locations_tag_templates_delete_builder, datacatalog_projects_locations_tag_templates_delete_task,
    datacatalog_projects_locations_tag_templates_get_iam_policy_builder, datacatalog_projects_locations_tag_templates_get_iam_policy_task,
    datacatalog_projects_locations_tag_templates_patch_builder, datacatalog_projects_locations_tag_templates_patch_task,
    datacatalog_projects_locations_tag_templates_set_iam_policy_builder, datacatalog_projects_locations_tag_templates_set_iam_policy_task,
    datacatalog_projects_locations_tag_templates_test_iam_permissions_builder, datacatalog_projects_locations_tag_templates_test_iam_permissions_task,
    datacatalog_projects_locations_tag_templates_fields_create_builder, datacatalog_projects_locations_tag_templates_fields_create_task,
    datacatalog_projects_locations_tag_templates_fields_delete_builder, datacatalog_projects_locations_tag_templates_fields_delete_task,
    datacatalog_projects_locations_tag_templates_fields_patch_builder, datacatalog_projects_locations_tag_templates_fields_patch_task,
    datacatalog_projects_locations_tag_templates_fields_rename_builder, datacatalog_projects_locations_tag_templates_fields_rename_task,
    datacatalog_projects_locations_tag_templates_fields_enum_values_rename_builder, datacatalog_projects_locations_tag_templates_fields_enum_values_rename_task,
    datacatalog_projects_locations_taxonomies_create_builder, datacatalog_projects_locations_taxonomies_create_task,
    datacatalog_projects_locations_taxonomies_delete_builder, datacatalog_projects_locations_taxonomies_delete_task,
    datacatalog_projects_locations_taxonomies_get_iam_policy_builder, datacatalog_projects_locations_taxonomies_get_iam_policy_task,
    datacatalog_projects_locations_taxonomies_import_builder, datacatalog_projects_locations_taxonomies_import_task,
    datacatalog_projects_locations_taxonomies_patch_builder, datacatalog_projects_locations_taxonomies_patch_task,
    datacatalog_projects_locations_taxonomies_replace_builder, datacatalog_projects_locations_taxonomies_replace_task,
    datacatalog_projects_locations_taxonomies_set_iam_policy_builder, datacatalog_projects_locations_taxonomies_set_iam_policy_task,
    datacatalog_projects_locations_taxonomies_test_iam_permissions_builder, datacatalog_projects_locations_taxonomies_test_iam_permissions_task,
    datacatalog_projects_locations_taxonomies_policy_tags_create_builder, datacatalog_projects_locations_taxonomies_policy_tags_create_task,
    datacatalog_projects_locations_taxonomies_policy_tags_delete_builder, datacatalog_projects_locations_taxonomies_policy_tags_delete_task,
    datacatalog_projects_locations_taxonomies_policy_tags_get_iam_policy_builder, datacatalog_projects_locations_taxonomies_policy_tags_get_iam_policy_task,
    datacatalog_projects_locations_taxonomies_policy_tags_patch_builder, datacatalog_projects_locations_taxonomies_policy_tags_patch_task,
    datacatalog_projects_locations_taxonomies_policy_tags_set_iam_policy_builder, datacatalog_projects_locations_taxonomies_policy_tags_set_iam_policy_task,
    datacatalog_projects_locations_taxonomies_policy_tags_test_iam_permissions_builder, datacatalog_projects_locations_taxonomies_policy_tags_test_iam_permissions_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::datacatalog::Empty;
use crate::providers::gcp::clients::datacatalog::GoogleCloudDatacatalogV1Contacts;
use crate::providers::gcp::clients::datacatalog::GoogleCloudDatacatalogV1Entry;
use crate::providers::gcp::clients::datacatalog::GoogleCloudDatacatalogV1EntryGroup;
use crate::providers::gcp::clients::datacatalog::GoogleCloudDatacatalogV1EntryOverview;
use crate::providers::gcp::clients::datacatalog::GoogleCloudDatacatalogV1ImportTaxonomiesResponse;
use crate::providers::gcp::clients::datacatalog::GoogleCloudDatacatalogV1MigrationConfig;
use crate::providers::gcp::clients::datacatalog::GoogleCloudDatacatalogV1PolicyTag;
use crate::providers::gcp::clients::datacatalog::GoogleCloudDatacatalogV1SearchCatalogResponse;
use crate::providers::gcp::clients::datacatalog::GoogleCloudDatacatalogV1StarEntryResponse;
use crate::providers::gcp::clients::datacatalog::GoogleCloudDatacatalogV1Tag;
use crate::providers::gcp::clients::datacatalog::GoogleCloudDatacatalogV1TagTemplate;
use crate::providers::gcp::clients::datacatalog::GoogleCloudDatacatalogV1TagTemplateField;
use crate::providers::gcp::clients::datacatalog::GoogleCloudDatacatalogV1Taxonomy;
use crate::providers::gcp::clients::datacatalog::GoogleCloudDatacatalogV1UnstarEntryResponse;
use crate::providers::gcp::clients::datacatalog::Operation;
use crate::providers::gcp::clients::datacatalog::Policy;
use crate::providers::gcp::clients::datacatalog::TestIamPermissionsResponse;
use crate::providers::gcp::clients::datacatalog::DatacatalogCatalogSearchArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogOrganizationsLocationsSetConfigArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsEntryGroupsCreateArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsEntryGroupsDeleteArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsEntryGroupsEntriesCreateArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsEntryGroupsEntriesDeleteArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsEntryGroupsEntriesGetIamPolicyArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsEntryGroupsEntriesImportArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsEntryGroupsEntriesModifyEntryContactsArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsEntryGroupsEntriesModifyEntryOverviewArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsEntryGroupsEntriesPatchArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsEntryGroupsEntriesStarArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsEntryGroupsEntriesTagsCreateArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsEntryGroupsEntriesTagsDeleteArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsEntryGroupsEntriesTagsPatchArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsEntryGroupsEntriesTagsReconcileArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsEntryGroupsEntriesTestIamPermissionsArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsEntryGroupsEntriesUnstarArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsEntryGroupsGetIamPolicyArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsEntryGroupsPatchArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsEntryGroupsSetIamPolicyArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsEntryGroupsTagsCreateArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsEntryGroupsTagsDeleteArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsEntryGroupsTagsPatchArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsEntryGroupsTestIamPermissionsArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsSetConfigArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsTagTemplatesCreateArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsTagTemplatesDeleteArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsTagTemplatesFieldsCreateArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsTagTemplatesFieldsDeleteArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsTagTemplatesFieldsEnumValuesRenameArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsTagTemplatesFieldsPatchArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsTagTemplatesFieldsRenameArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsTagTemplatesGetIamPolicyArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsTagTemplatesPatchArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsTagTemplatesSetIamPolicyArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsTagTemplatesTestIamPermissionsArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsTaxonomiesCreateArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsTaxonomiesDeleteArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsTaxonomiesGetIamPolicyArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsTaxonomiesImportArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsTaxonomiesPatchArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsTaxonomiesPolicyTagsCreateArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsTaxonomiesPolicyTagsDeleteArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsTaxonomiesPolicyTagsGetIamPolicyArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsTaxonomiesPolicyTagsPatchArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsTaxonomiesPolicyTagsSetIamPolicyArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsTaxonomiesPolicyTagsTestIamPermissionsArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsTaxonomiesReplaceArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsTaxonomiesSetIamPolicyArgs;
use crate::providers::gcp::clients::datacatalog::DatacatalogProjectsLocationsTaxonomiesTestIamPermissionsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DatacatalogProvider with automatic state tracking.
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
/// let provider = DatacatalogProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct DatacatalogProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> DatacatalogProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new DatacatalogProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Datacatalog catalog search.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogV1SearchCatalogResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datacatalog_catalog_search(
        &self,
        args: &DatacatalogCatalogSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogV1SearchCatalogResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_catalog_search_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_catalog_search_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog organizations locations set config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogV1MigrationConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datacatalog_organizations_locations_set_config(
        &self,
        args: &DatacatalogOrganizationsLocationsSetConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogV1MigrationConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_organizations_locations_set_config_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_organizations_locations_set_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations set config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogV1MigrationConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datacatalog_projects_locations_set_config(
        &self,
        args: &DatacatalogProjectsLocationsSetConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogV1MigrationConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_set_config_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_set_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations entry groups create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogV1EntryGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datacatalog_projects_locations_entry_groups_create(
        &self,
        args: &DatacatalogProjectsLocationsEntryGroupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogV1EntryGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_entry_groups_create_builder(
            &self.http_client,
            &args.parent,
            &args.entryGroupId,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_entry_groups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations entry groups delete.
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
    pub fn datacatalog_projects_locations_entry_groups_delete(
        &self,
        args: &DatacatalogProjectsLocationsEntryGroupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_entry_groups_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_entry_groups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations entry groups get iam policy.
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
    pub fn datacatalog_projects_locations_entry_groups_get_iam_policy(
        &self,
        args: &DatacatalogProjectsLocationsEntryGroupsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_entry_groups_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_entry_groups_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations entry groups patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogV1EntryGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datacatalog_projects_locations_entry_groups_patch(
        &self,
        args: &DatacatalogProjectsLocationsEntryGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogV1EntryGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_entry_groups_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_entry_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations entry groups set iam policy.
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
    pub fn datacatalog_projects_locations_entry_groups_set_iam_policy(
        &self,
        args: &DatacatalogProjectsLocationsEntryGroupsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_entry_groups_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_entry_groups_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations entry groups test iam permissions.
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
    pub fn datacatalog_projects_locations_entry_groups_test_iam_permissions(
        &self,
        args: &DatacatalogProjectsLocationsEntryGroupsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_entry_groups_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_entry_groups_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations entry groups entries create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogV1Entry result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datacatalog_projects_locations_entry_groups_entries_create(
        &self,
        args: &DatacatalogProjectsLocationsEntryGroupsEntriesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogV1Entry, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_entry_groups_entries_create_builder(
            &self.http_client,
            &args.parent,
            &args.entryId,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_entry_groups_entries_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations entry groups entries delete.
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
    pub fn datacatalog_projects_locations_entry_groups_entries_delete(
        &self,
        args: &DatacatalogProjectsLocationsEntryGroupsEntriesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_entry_groups_entries_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_entry_groups_entries_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations entry groups entries get iam policy.
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
    pub fn datacatalog_projects_locations_entry_groups_entries_get_iam_policy(
        &self,
        args: &DatacatalogProjectsLocationsEntryGroupsEntriesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_entry_groups_entries_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_entry_groups_entries_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations entry groups entries import.
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
    pub fn datacatalog_projects_locations_entry_groups_entries_import(
        &self,
        args: &DatacatalogProjectsLocationsEntryGroupsEntriesImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_entry_groups_entries_import_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_entry_groups_entries_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations entry groups entries modify entry contacts.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogV1Contacts result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datacatalog_projects_locations_entry_groups_entries_modify_entry_contacts(
        &self,
        args: &DatacatalogProjectsLocationsEntryGroupsEntriesModifyEntryContactsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogV1Contacts, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_entry_groups_entries_modify_entry_contacts_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_entry_groups_entries_modify_entry_contacts_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations entry groups entries modify entry overview.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogV1EntryOverview result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datacatalog_projects_locations_entry_groups_entries_modify_entry_overview(
        &self,
        args: &DatacatalogProjectsLocationsEntryGroupsEntriesModifyEntryOverviewArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogV1EntryOverview, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_entry_groups_entries_modify_entry_overview_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_entry_groups_entries_modify_entry_overview_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations entry groups entries patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogV1Entry result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datacatalog_projects_locations_entry_groups_entries_patch(
        &self,
        args: &DatacatalogProjectsLocationsEntryGroupsEntriesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogV1Entry, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_entry_groups_entries_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_entry_groups_entries_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations entry groups entries star.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogV1StarEntryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datacatalog_projects_locations_entry_groups_entries_star(
        &self,
        args: &DatacatalogProjectsLocationsEntryGroupsEntriesStarArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogV1StarEntryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_entry_groups_entries_star_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_entry_groups_entries_star_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations entry groups entries test iam permissions.
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
    pub fn datacatalog_projects_locations_entry_groups_entries_test_iam_permissions(
        &self,
        args: &DatacatalogProjectsLocationsEntryGroupsEntriesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_entry_groups_entries_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_entry_groups_entries_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations entry groups entries unstar.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogV1UnstarEntryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datacatalog_projects_locations_entry_groups_entries_unstar(
        &self,
        args: &DatacatalogProjectsLocationsEntryGroupsEntriesUnstarArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogV1UnstarEntryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_entry_groups_entries_unstar_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_entry_groups_entries_unstar_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations entry groups entries tags create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogV1Tag result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datacatalog_projects_locations_entry_groups_entries_tags_create(
        &self,
        args: &DatacatalogProjectsLocationsEntryGroupsEntriesTagsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogV1Tag, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_entry_groups_entries_tags_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_entry_groups_entries_tags_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations entry groups entries tags delete.
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
    pub fn datacatalog_projects_locations_entry_groups_entries_tags_delete(
        &self,
        args: &DatacatalogProjectsLocationsEntryGroupsEntriesTagsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_entry_groups_entries_tags_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_entry_groups_entries_tags_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations entry groups entries tags patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogV1Tag result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datacatalog_projects_locations_entry_groups_entries_tags_patch(
        &self,
        args: &DatacatalogProjectsLocationsEntryGroupsEntriesTagsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogV1Tag, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_entry_groups_entries_tags_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_entry_groups_entries_tags_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations entry groups entries tags reconcile.
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
    pub fn datacatalog_projects_locations_entry_groups_entries_tags_reconcile(
        &self,
        args: &DatacatalogProjectsLocationsEntryGroupsEntriesTagsReconcileArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_entry_groups_entries_tags_reconcile_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_entry_groups_entries_tags_reconcile_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations entry groups tags create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogV1Tag result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datacatalog_projects_locations_entry_groups_tags_create(
        &self,
        args: &DatacatalogProjectsLocationsEntryGroupsTagsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogV1Tag, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_entry_groups_tags_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_entry_groups_tags_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations entry groups tags delete.
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
    pub fn datacatalog_projects_locations_entry_groups_tags_delete(
        &self,
        args: &DatacatalogProjectsLocationsEntryGroupsTagsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_entry_groups_tags_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_entry_groups_tags_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations entry groups tags patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogV1Tag result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datacatalog_projects_locations_entry_groups_tags_patch(
        &self,
        args: &DatacatalogProjectsLocationsEntryGroupsTagsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogV1Tag, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_entry_groups_tags_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_entry_groups_tags_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations operations cancel.
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
    pub fn datacatalog_projects_locations_operations_cancel(
        &self,
        args: &DatacatalogProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations operations delete.
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
    pub fn datacatalog_projects_locations_operations_delete(
        &self,
        args: &DatacatalogProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations tag templates create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogV1TagTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datacatalog_projects_locations_tag_templates_create(
        &self,
        args: &DatacatalogProjectsLocationsTagTemplatesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogV1TagTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_tag_templates_create_builder(
            &self.http_client,
            &args.parent,
            &args.tagTemplateId,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_tag_templates_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations tag templates delete.
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
    pub fn datacatalog_projects_locations_tag_templates_delete(
        &self,
        args: &DatacatalogProjectsLocationsTagTemplatesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_tag_templates_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_tag_templates_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations tag templates get iam policy.
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
    pub fn datacatalog_projects_locations_tag_templates_get_iam_policy(
        &self,
        args: &DatacatalogProjectsLocationsTagTemplatesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_tag_templates_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_tag_templates_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations tag templates patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogV1TagTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datacatalog_projects_locations_tag_templates_patch(
        &self,
        args: &DatacatalogProjectsLocationsTagTemplatesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogV1TagTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_tag_templates_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_tag_templates_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations tag templates set iam policy.
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
    pub fn datacatalog_projects_locations_tag_templates_set_iam_policy(
        &self,
        args: &DatacatalogProjectsLocationsTagTemplatesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_tag_templates_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_tag_templates_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations tag templates test iam permissions.
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
    pub fn datacatalog_projects_locations_tag_templates_test_iam_permissions(
        &self,
        args: &DatacatalogProjectsLocationsTagTemplatesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_tag_templates_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_tag_templates_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations tag templates fields create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogV1TagTemplateField result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datacatalog_projects_locations_tag_templates_fields_create(
        &self,
        args: &DatacatalogProjectsLocationsTagTemplatesFieldsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogV1TagTemplateField, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_tag_templates_fields_create_builder(
            &self.http_client,
            &args.parent,
            &args.tagTemplateFieldId,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_tag_templates_fields_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations tag templates fields delete.
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
    pub fn datacatalog_projects_locations_tag_templates_fields_delete(
        &self,
        args: &DatacatalogProjectsLocationsTagTemplatesFieldsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_tag_templates_fields_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_tag_templates_fields_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations tag templates fields patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogV1TagTemplateField result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datacatalog_projects_locations_tag_templates_fields_patch(
        &self,
        args: &DatacatalogProjectsLocationsTagTemplatesFieldsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogV1TagTemplateField, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_tag_templates_fields_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_tag_templates_fields_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations tag templates fields rename.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogV1TagTemplateField result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datacatalog_projects_locations_tag_templates_fields_rename(
        &self,
        args: &DatacatalogProjectsLocationsTagTemplatesFieldsRenameArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogV1TagTemplateField, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_tag_templates_fields_rename_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_tag_templates_fields_rename_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations tag templates fields enum values rename.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogV1TagTemplateField result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datacatalog_projects_locations_tag_templates_fields_enum_values_rename(
        &self,
        args: &DatacatalogProjectsLocationsTagTemplatesFieldsEnumValuesRenameArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogV1TagTemplateField, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_tag_templates_fields_enum_values_rename_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_tag_templates_fields_enum_values_rename_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations taxonomies create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogV1Taxonomy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datacatalog_projects_locations_taxonomies_create(
        &self,
        args: &DatacatalogProjectsLocationsTaxonomiesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogV1Taxonomy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_taxonomies_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_taxonomies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations taxonomies delete.
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
    pub fn datacatalog_projects_locations_taxonomies_delete(
        &self,
        args: &DatacatalogProjectsLocationsTaxonomiesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_taxonomies_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_taxonomies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations taxonomies get iam policy.
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
    pub fn datacatalog_projects_locations_taxonomies_get_iam_policy(
        &self,
        args: &DatacatalogProjectsLocationsTaxonomiesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_taxonomies_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_taxonomies_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations taxonomies import.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogV1ImportTaxonomiesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datacatalog_projects_locations_taxonomies_import(
        &self,
        args: &DatacatalogProjectsLocationsTaxonomiesImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogV1ImportTaxonomiesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_taxonomies_import_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_taxonomies_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations taxonomies patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogV1Taxonomy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datacatalog_projects_locations_taxonomies_patch(
        &self,
        args: &DatacatalogProjectsLocationsTaxonomiesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogV1Taxonomy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_taxonomies_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_taxonomies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations taxonomies replace.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogV1Taxonomy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datacatalog_projects_locations_taxonomies_replace(
        &self,
        args: &DatacatalogProjectsLocationsTaxonomiesReplaceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogV1Taxonomy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_taxonomies_replace_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_taxonomies_replace_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations taxonomies set iam policy.
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
    pub fn datacatalog_projects_locations_taxonomies_set_iam_policy(
        &self,
        args: &DatacatalogProjectsLocationsTaxonomiesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_taxonomies_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_taxonomies_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations taxonomies test iam permissions.
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
    pub fn datacatalog_projects_locations_taxonomies_test_iam_permissions(
        &self,
        args: &DatacatalogProjectsLocationsTaxonomiesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_taxonomies_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_taxonomies_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations taxonomies policy tags create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogV1PolicyTag result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datacatalog_projects_locations_taxonomies_policy_tags_create(
        &self,
        args: &DatacatalogProjectsLocationsTaxonomiesPolicyTagsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogV1PolicyTag, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_taxonomies_policy_tags_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_taxonomies_policy_tags_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations taxonomies policy tags delete.
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
    pub fn datacatalog_projects_locations_taxonomies_policy_tags_delete(
        &self,
        args: &DatacatalogProjectsLocationsTaxonomiesPolicyTagsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_taxonomies_policy_tags_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_taxonomies_policy_tags_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations taxonomies policy tags get iam policy.
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
    pub fn datacatalog_projects_locations_taxonomies_policy_tags_get_iam_policy(
        &self,
        args: &DatacatalogProjectsLocationsTaxonomiesPolicyTagsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_taxonomies_policy_tags_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_taxonomies_policy_tags_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations taxonomies policy tags patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogV1PolicyTag result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datacatalog_projects_locations_taxonomies_policy_tags_patch(
        &self,
        args: &DatacatalogProjectsLocationsTaxonomiesPolicyTagsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogV1PolicyTag, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_taxonomies_policy_tags_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_taxonomies_policy_tags_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations taxonomies policy tags set iam policy.
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
    pub fn datacatalog_projects_locations_taxonomies_policy_tags_set_iam_policy(
        &self,
        args: &DatacatalogProjectsLocationsTaxonomiesPolicyTagsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_taxonomies_policy_tags_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_taxonomies_policy_tags_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datacatalog projects locations taxonomies policy tags test iam permissions.
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
    pub fn datacatalog_projects_locations_taxonomies_policy_tags_test_iam_permissions(
        &self,
        args: &DatacatalogProjectsLocationsTaxonomiesPolicyTagsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datacatalog_projects_locations_taxonomies_policy_tags_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = datacatalog_projects_locations_taxonomies_policy_tags_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
