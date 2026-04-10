//! DataplexProvider - State-aware dataplex API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       dataplex API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::dataplex::{
    dataplex_organizations_locations_encryption_configs_create_builder, dataplex_organizations_locations_encryption_configs_create_task,
    dataplex_organizations_locations_encryption_configs_delete_builder, dataplex_organizations_locations_encryption_configs_delete_task,
    dataplex_organizations_locations_encryption_configs_patch_builder, dataplex_organizations_locations_encryption_configs_patch_task,
    dataplex_organizations_locations_encryption_configs_set_iam_policy_builder, dataplex_organizations_locations_encryption_configs_set_iam_policy_task,
    dataplex_organizations_locations_encryption_configs_test_iam_permissions_builder, dataplex_organizations_locations_encryption_configs_test_iam_permissions_task,
    dataplex_organizations_locations_operations_cancel_builder, dataplex_organizations_locations_operations_cancel_task,
    dataplex_organizations_locations_operations_delete_builder, dataplex_organizations_locations_operations_delete_task,
    dataplex_projects_locations_lookup_context_builder, dataplex_projects_locations_lookup_context_task,
    dataplex_projects_locations_search_entries_builder, dataplex_projects_locations_search_entries_task,
    dataplex_projects_locations_aspect_types_create_builder, dataplex_projects_locations_aspect_types_create_task,
    dataplex_projects_locations_aspect_types_delete_builder, dataplex_projects_locations_aspect_types_delete_task,
    dataplex_projects_locations_aspect_types_patch_builder, dataplex_projects_locations_aspect_types_patch_task,
    dataplex_projects_locations_aspect_types_set_iam_policy_builder, dataplex_projects_locations_aspect_types_set_iam_policy_task,
    dataplex_projects_locations_aspect_types_test_iam_permissions_builder, dataplex_projects_locations_aspect_types_test_iam_permissions_task,
    dataplex_projects_locations_change_requests_set_iam_policy_builder, dataplex_projects_locations_change_requests_set_iam_policy_task,
    dataplex_projects_locations_change_requests_test_iam_permissions_builder, dataplex_projects_locations_change_requests_test_iam_permissions_task,
    dataplex_projects_locations_data_attribute_bindings_create_builder, dataplex_projects_locations_data_attribute_bindings_create_task,
    dataplex_projects_locations_data_attribute_bindings_delete_builder, dataplex_projects_locations_data_attribute_bindings_delete_task,
    dataplex_projects_locations_data_attribute_bindings_patch_builder, dataplex_projects_locations_data_attribute_bindings_patch_task,
    dataplex_projects_locations_data_attribute_bindings_set_iam_policy_builder, dataplex_projects_locations_data_attribute_bindings_set_iam_policy_task,
    dataplex_projects_locations_data_attribute_bindings_test_iam_permissions_builder, dataplex_projects_locations_data_attribute_bindings_test_iam_permissions_task,
    dataplex_projects_locations_data_domains_set_iam_policy_builder, dataplex_projects_locations_data_domains_set_iam_policy_task,
    dataplex_projects_locations_data_domains_test_iam_permissions_builder, dataplex_projects_locations_data_domains_test_iam_permissions_task,
    dataplex_projects_locations_data_products_create_builder, dataplex_projects_locations_data_products_create_task,
    dataplex_projects_locations_data_products_delete_builder, dataplex_projects_locations_data_products_delete_task,
    dataplex_projects_locations_data_products_patch_builder, dataplex_projects_locations_data_products_patch_task,
    dataplex_projects_locations_data_products_set_iam_policy_builder, dataplex_projects_locations_data_products_set_iam_policy_task,
    dataplex_projects_locations_data_products_test_iam_permissions_builder, dataplex_projects_locations_data_products_test_iam_permissions_task,
    dataplex_projects_locations_data_products_data_assets_create_builder, dataplex_projects_locations_data_products_data_assets_create_task,
    dataplex_projects_locations_data_products_data_assets_delete_builder, dataplex_projects_locations_data_products_data_assets_delete_task,
    dataplex_projects_locations_data_products_data_assets_patch_builder, dataplex_projects_locations_data_products_data_assets_patch_task,
    dataplex_projects_locations_data_scans_create_builder, dataplex_projects_locations_data_scans_create_task,
    dataplex_projects_locations_data_scans_delete_builder, dataplex_projects_locations_data_scans_delete_task,
    dataplex_projects_locations_data_scans_generate_data_quality_rules_builder, dataplex_projects_locations_data_scans_generate_data_quality_rules_task,
    dataplex_projects_locations_data_scans_patch_builder, dataplex_projects_locations_data_scans_patch_task,
    dataplex_projects_locations_data_scans_run_builder, dataplex_projects_locations_data_scans_run_task,
    dataplex_projects_locations_data_scans_set_iam_policy_builder, dataplex_projects_locations_data_scans_set_iam_policy_task,
    dataplex_projects_locations_data_scans_test_iam_permissions_builder, dataplex_projects_locations_data_scans_test_iam_permissions_task,
    dataplex_projects_locations_data_scans_jobs_generate_data_quality_rules_builder, dataplex_projects_locations_data_scans_jobs_generate_data_quality_rules_task,
    dataplex_projects_locations_data_taxonomies_create_builder, dataplex_projects_locations_data_taxonomies_create_task,
    dataplex_projects_locations_data_taxonomies_delete_builder, dataplex_projects_locations_data_taxonomies_delete_task,
    dataplex_projects_locations_data_taxonomies_patch_builder, dataplex_projects_locations_data_taxonomies_patch_task,
    dataplex_projects_locations_data_taxonomies_set_iam_policy_builder, dataplex_projects_locations_data_taxonomies_set_iam_policy_task,
    dataplex_projects_locations_data_taxonomies_test_iam_permissions_builder, dataplex_projects_locations_data_taxonomies_test_iam_permissions_task,
    dataplex_projects_locations_data_taxonomies_attributes_create_builder, dataplex_projects_locations_data_taxonomies_attributes_create_task,
    dataplex_projects_locations_data_taxonomies_attributes_delete_builder, dataplex_projects_locations_data_taxonomies_attributes_delete_task,
    dataplex_projects_locations_data_taxonomies_attributes_patch_builder, dataplex_projects_locations_data_taxonomies_attributes_patch_task,
    dataplex_projects_locations_data_taxonomies_attributes_set_iam_policy_builder, dataplex_projects_locations_data_taxonomies_attributes_set_iam_policy_task,
    dataplex_projects_locations_data_taxonomies_attributes_test_iam_permissions_builder, dataplex_projects_locations_data_taxonomies_attributes_test_iam_permissions_task,
    dataplex_projects_locations_entry_groups_create_builder, dataplex_projects_locations_entry_groups_create_task,
    dataplex_projects_locations_entry_groups_delete_builder, dataplex_projects_locations_entry_groups_delete_task,
    dataplex_projects_locations_entry_groups_patch_builder, dataplex_projects_locations_entry_groups_patch_task,
    dataplex_projects_locations_entry_groups_set_iam_policy_builder, dataplex_projects_locations_entry_groups_set_iam_policy_task,
    dataplex_projects_locations_entry_groups_test_iam_permissions_builder, dataplex_projects_locations_entry_groups_test_iam_permissions_task,
    dataplex_projects_locations_entry_groups_entries_create_builder, dataplex_projects_locations_entry_groups_entries_create_task,
    dataplex_projects_locations_entry_groups_entries_delete_builder, dataplex_projects_locations_entry_groups_entries_delete_task,
    dataplex_projects_locations_entry_groups_entries_patch_builder, dataplex_projects_locations_entry_groups_entries_patch_task,
    dataplex_projects_locations_entry_groups_entry_links_create_builder, dataplex_projects_locations_entry_groups_entry_links_create_task,
    dataplex_projects_locations_entry_groups_entry_links_delete_builder, dataplex_projects_locations_entry_groups_entry_links_delete_task,
    dataplex_projects_locations_entry_groups_entry_links_patch_builder, dataplex_projects_locations_entry_groups_entry_links_patch_task,
    dataplex_projects_locations_entry_link_types_set_iam_policy_builder, dataplex_projects_locations_entry_link_types_set_iam_policy_task,
    dataplex_projects_locations_entry_link_types_test_iam_permissions_builder, dataplex_projects_locations_entry_link_types_test_iam_permissions_task,
    dataplex_projects_locations_entry_types_create_builder, dataplex_projects_locations_entry_types_create_task,
    dataplex_projects_locations_entry_types_delete_builder, dataplex_projects_locations_entry_types_delete_task,
    dataplex_projects_locations_entry_types_patch_builder, dataplex_projects_locations_entry_types_patch_task,
    dataplex_projects_locations_entry_types_set_iam_policy_builder, dataplex_projects_locations_entry_types_set_iam_policy_task,
    dataplex_projects_locations_entry_types_test_iam_permissions_builder, dataplex_projects_locations_entry_types_test_iam_permissions_task,
    dataplex_projects_locations_glossaries_create_builder, dataplex_projects_locations_glossaries_create_task,
    dataplex_projects_locations_glossaries_delete_builder, dataplex_projects_locations_glossaries_delete_task,
    dataplex_projects_locations_glossaries_patch_builder, dataplex_projects_locations_glossaries_patch_task,
    dataplex_projects_locations_glossaries_set_iam_policy_builder, dataplex_projects_locations_glossaries_set_iam_policy_task,
    dataplex_projects_locations_glossaries_test_iam_permissions_builder, dataplex_projects_locations_glossaries_test_iam_permissions_task,
    dataplex_projects_locations_glossaries_categories_create_builder, dataplex_projects_locations_glossaries_categories_create_task,
    dataplex_projects_locations_glossaries_categories_delete_builder, dataplex_projects_locations_glossaries_categories_delete_task,
    dataplex_projects_locations_glossaries_categories_patch_builder, dataplex_projects_locations_glossaries_categories_patch_task,
    dataplex_projects_locations_glossaries_categories_set_iam_policy_builder, dataplex_projects_locations_glossaries_categories_set_iam_policy_task,
    dataplex_projects_locations_glossaries_categories_test_iam_permissions_builder, dataplex_projects_locations_glossaries_categories_test_iam_permissions_task,
    dataplex_projects_locations_glossaries_terms_create_builder, dataplex_projects_locations_glossaries_terms_create_task,
    dataplex_projects_locations_glossaries_terms_delete_builder, dataplex_projects_locations_glossaries_terms_delete_task,
    dataplex_projects_locations_glossaries_terms_patch_builder, dataplex_projects_locations_glossaries_terms_patch_task,
    dataplex_projects_locations_glossaries_terms_set_iam_policy_builder, dataplex_projects_locations_glossaries_terms_set_iam_policy_task,
    dataplex_projects_locations_glossaries_terms_test_iam_permissions_builder, dataplex_projects_locations_glossaries_terms_test_iam_permissions_task,
    dataplex_projects_locations_governance_rules_set_iam_policy_builder, dataplex_projects_locations_governance_rules_set_iam_policy_task,
    dataplex_projects_locations_governance_rules_test_iam_permissions_builder, dataplex_projects_locations_governance_rules_test_iam_permissions_task,
    dataplex_projects_locations_lakes_create_builder, dataplex_projects_locations_lakes_create_task,
    dataplex_projects_locations_lakes_delete_builder, dataplex_projects_locations_lakes_delete_task,
    dataplex_projects_locations_lakes_patch_builder, dataplex_projects_locations_lakes_patch_task,
    dataplex_projects_locations_lakes_set_iam_policy_builder, dataplex_projects_locations_lakes_set_iam_policy_task,
    dataplex_projects_locations_lakes_test_iam_permissions_builder, dataplex_projects_locations_lakes_test_iam_permissions_task,
    dataplex_projects_locations_lakes_environments_set_iam_policy_builder, dataplex_projects_locations_lakes_environments_set_iam_policy_task,
    dataplex_projects_locations_lakes_environments_test_iam_permissions_builder, dataplex_projects_locations_lakes_environments_test_iam_permissions_task,
    dataplex_projects_locations_lakes_tasks_create_builder, dataplex_projects_locations_lakes_tasks_create_task,
    dataplex_projects_locations_lakes_tasks_delete_builder, dataplex_projects_locations_lakes_tasks_delete_task,
    dataplex_projects_locations_lakes_tasks_patch_builder, dataplex_projects_locations_lakes_tasks_patch_task,
    dataplex_projects_locations_lakes_tasks_run_builder, dataplex_projects_locations_lakes_tasks_run_task,
    dataplex_projects_locations_lakes_tasks_set_iam_policy_builder, dataplex_projects_locations_lakes_tasks_set_iam_policy_task,
    dataplex_projects_locations_lakes_tasks_test_iam_permissions_builder, dataplex_projects_locations_lakes_tasks_test_iam_permissions_task,
    dataplex_projects_locations_lakes_tasks_jobs_cancel_builder, dataplex_projects_locations_lakes_tasks_jobs_cancel_task,
    dataplex_projects_locations_lakes_zones_create_builder, dataplex_projects_locations_lakes_zones_create_task,
    dataplex_projects_locations_lakes_zones_delete_builder, dataplex_projects_locations_lakes_zones_delete_task,
    dataplex_projects_locations_lakes_zones_patch_builder, dataplex_projects_locations_lakes_zones_patch_task,
    dataplex_projects_locations_lakes_zones_set_iam_policy_builder, dataplex_projects_locations_lakes_zones_set_iam_policy_task,
    dataplex_projects_locations_lakes_zones_test_iam_permissions_builder, dataplex_projects_locations_lakes_zones_test_iam_permissions_task,
    dataplex_projects_locations_lakes_zones_assets_create_builder, dataplex_projects_locations_lakes_zones_assets_create_task,
    dataplex_projects_locations_lakes_zones_assets_delete_builder, dataplex_projects_locations_lakes_zones_assets_delete_task,
    dataplex_projects_locations_lakes_zones_assets_patch_builder, dataplex_projects_locations_lakes_zones_assets_patch_task,
    dataplex_projects_locations_lakes_zones_assets_set_iam_policy_builder, dataplex_projects_locations_lakes_zones_assets_set_iam_policy_task,
    dataplex_projects_locations_lakes_zones_assets_test_iam_permissions_builder, dataplex_projects_locations_lakes_zones_assets_test_iam_permissions_task,
    dataplex_projects_locations_lakes_zones_entities_create_builder, dataplex_projects_locations_lakes_zones_entities_create_task,
    dataplex_projects_locations_lakes_zones_entities_delete_builder, dataplex_projects_locations_lakes_zones_entities_delete_task,
    dataplex_projects_locations_lakes_zones_entities_update_builder, dataplex_projects_locations_lakes_zones_entities_update_task,
    dataplex_projects_locations_lakes_zones_entities_partitions_create_builder, dataplex_projects_locations_lakes_zones_entities_partitions_create_task,
    dataplex_projects_locations_lakes_zones_entities_partitions_delete_builder, dataplex_projects_locations_lakes_zones_entities_partitions_delete_task,
    dataplex_projects_locations_metadata_feeds_create_builder, dataplex_projects_locations_metadata_feeds_create_task,
    dataplex_projects_locations_metadata_feeds_delete_builder, dataplex_projects_locations_metadata_feeds_delete_task,
    dataplex_projects_locations_metadata_feeds_patch_builder, dataplex_projects_locations_metadata_feeds_patch_task,
    dataplex_projects_locations_metadata_jobs_cancel_builder, dataplex_projects_locations_metadata_jobs_cancel_task,
    dataplex_projects_locations_metadata_jobs_create_builder, dataplex_projects_locations_metadata_jobs_create_task,
    dataplex_projects_locations_operations_cancel_builder, dataplex_projects_locations_operations_cancel_task,
    dataplex_projects_locations_operations_delete_builder, dataplex_projects_locations_operations_delete_task,
    dataplex_projects_locations_policy_intents_set_iam_policy_builder, dataplex_projects_locations_policy_intents_set_iam_policy_task,
    dataplex_projects_locations_policy_intents_test_iam_permissions_builder, dataplex_projects_locations_policy_intents_test_iam_permissions_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::dataplex::Empty;
use crate::providers::gcp::clients::dataplex::GoogleCloudDataplexV1Entity;
use crate::providers::gcp::clients::dataplex::GoogleCloudDataplexV1Entry;
use crate::providers::gcp::clients::dataplex::GoogleCloudDataplexV1EntryLink;
use crate::providers::gcp::clients::dataplex::GoogleCloudDataplexV1GenerateDataQualityRulesResponse;
use crate::providers::gcp::clients::dataplex::GoogleCloudDataplexV1GlossaryCategory;
use crate::providers::gcp::clients::dataplex::GoogleCloudDataplexV1GlossaryTerm;
use crate::providers::gcp::clients::dataplex::GoogleCloudDataplexV1LookupContextResponse;
use crate::providers::gcp::clients::dataplex::GoogleCloudDataplexV1Partition;
use crate::providers::gcp::clients::dataplex::GoogleCloudDataplexV1RunDataScanResponse;
use crate::providers::gcp::clients::dataplex::GoogleCloudDataplexV1RunTaskResponse;
use crate::providers::gcp::clients::dataplex::GoogleCloudDataplexV1SearchEntriesResponse;
use crate::providers::gcp::clients::dataplex::GoogleIamV1Policy;
use crate::providers::gcp::clients::dataplex::GoogleIamV1TestIamPermissionsResponse;
use crate::providers::gcp::clients::dataplex::GoogleLongrunningOperation;
use crate::providers::gcp::clients::dataplex::DataplexOrganizationsLocationsEncryptionConfigsCreateArgs;
use crate::providers::gcp::clients::dataplex::DataplexOrganizationsLocationsEncryptionConfigsDeleteArgs;
use crate::providers::gcp::clients::dataplex::DataplexOrganizationsLocationsEncryptionConfigsPatchArgs;
use crate::providers::gcp::clients::dataplex::DataplexOrganizationsLocationsEncryptionConfigsSetIamPolicyArgs;
use crate::providers::gcp::clients::dataplex::DataplexOrganizationsLocationsEncryptionConfigsTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataplex::DataplexOrganizationsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::dataplex::DataplexOrganizationsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsAspectTypesCreateArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsAspectTypesDeleteArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsAspectTypesPatchArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsAspectTypesSetIamPolicyArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsAspectTypesTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsChangeRequestsSetIamPolicyArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsChangeRequestsTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataAttributeBindingsCreateArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataAttributeBindingsDeleteArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataAttributeBindingsPatchArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataAttributeBindingsSetIamPolicyArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataAttributeBindingsTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataDomainsSetIamPolicyArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataDomainsTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataProductsCreateArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataProductsDataAssetsCreateArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataProductsDataAssetsDeleteArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataProductsDataAssetsPatchArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataProductsDeleteArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataProductsPatchArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataProductsSetIamPolicyArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataProductsTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataScansCreateArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataScansDeleteArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataScansGenerateDataQualityRulesArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataScansJobsGenerateDataQualityRulesArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataScansPatchArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataScansRunArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataScansSetIamPolicyArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataScansTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataTaxonomiesAttributesCreateArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataTaxonomiesAttributesDeleteArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataTaxonomiesAttributesPatchArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataTaxonomiesAttributesSetIamPolicyArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataTaxonomiesAttributesTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataTaxonomiesCreateArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataTaxonomiesDeleteArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataTaxonomiesPatchArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataTaxonomiesSetIamPolicyArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsDataTaxonomiesTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsEntryGroupsCreateArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsEntryGroupsDeleteArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsEntryGroupsEntriesCreateArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsEntryGroupsEntriesDeleteArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsEntryGroupsEntriesPatchArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsEntryGroupsEntryLinksCreateArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsEntryGroupsEntryLinksDeleteArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsEntryGroupsEntryLinksPatchArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsEntryGroupsPatchArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsEntryGroupsSetIamPolicyArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsEntryGroupsTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsEntryLinkTypesSetIamPolicyArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsEntryLinkTypesTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsEntryTypesCreateArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsEntryTypesDeleteArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsEntryTypesPatchArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsEntryTypesSetIamPolicyArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsEntryTypesTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsGlossariesCategoriesCreateArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsGlossariesCategoriesDeleteArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsGlossariesCategoriesPatchArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsGlossariesCategoriesSetIamPolicyArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsGlossariesCategoriesTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsGlossariesCreateArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsGlossariesDeleteArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsGlossariesPatchArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsGlossariesSetIamPolicyArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsGlossariesTermsCreateArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsGlossariesTermsDeleteArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsGlossariesTermsPatchArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsGlossariesTermsSetIamPolicyArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsGlossariesTermsTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsGlossariesTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsGovernanceRulesSetIamPolicyArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsGovernanceRulesTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLakesCreateArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLakesDeleteArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLakesEnvironmentsSetIamPolicyArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLakesEnvironmentsTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLakesPatchArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLakesSetIamPolicyArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLakesTasksCreateArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLakesTasksDeleteArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLakesTasksJobsCancelArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLakesTasksPatchArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLakesTasksRunArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLakesTasksSetIamPolicyArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLakesTasksTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLakesTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLakesZonesAssetsCreateArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLakesZonesAssetsDeleteArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLakesZonesAssetsPatchArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLakesZonesAssetsSetIamPolicyArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLakesZonesAssetsTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLakesZonesCreateArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLakesZonesDeleteArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLakesZonesEntitiesCreateArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLakesZonesEntitiesDeleteArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLakesZonesEntitiesPartitionsCreateArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLakesZonesEntitiesPartitionsDeleteArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLakesZonesEntitiesUpdateArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLakesZonesPatchArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLakesZonesSetIamPolicyArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLakesZonesTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsLookupContextArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsMetadataFeedsCreateArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsMetadataFeedsDeleteArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsMetadataFeedsPatchArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsMetadataJobsCancelArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsMetadataJobsCreateArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsPolicyIntentsSetIamPolicyArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsPolicyIntentsTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataplex::DataplexProjectsLocationsSearchEntriesArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DataplexProvider with automatic state tracking.
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
/// let provider = DataplexProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct DataplexProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> DataplexProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new DataplexProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Dataplex organizations locations encryption configs create.
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
    pub fn dataplex_organizations_locations_encryption_configs_create(
        &self,
        args: &DataplexOrganizationsLocationsEncryptionConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_organizations_locations_encryption_configs_create_builder(
            &self.http_client,
            &args.parent,
            &args.encryptionConfigId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_organizations_locations_encryption_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex organizations locations encryption configs delete.
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
    pub fn dataplex_organizations_locations_encryption_configs_delete(
        &self,
        args: &DataplexOrganizationsLocationsEncryptionConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_organizations_locations_encryption_configs_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_organizations_locations_encryption_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex organizations locations encryption configs patch.
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
    pub fn dataplex_organizations_locations_encryption_configs_patch(
        &self,
        args: &DataplexOrganizationsLocationsEncryptionConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_organizations_locations_encryption_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_organizations_locations_encryption_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex organizations locations encryption configs set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_organizations_locations_encryption_configs_set_iam_policy(
        &self,
        args: &DataplexOrganizationsLocationsEncryptionConfigsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_organizations_locations_encryption_configs_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_organizations_locations_encryption_configs_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex organizations locations encryption configs test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_organizations_locations_encryption_configs_test_iam_permissions(
        &self,
        args: &DataplexOrganizationsLocationsEncryptionConfigsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_organizations_locations_encryption_configs_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_organizations_locations_encryption_configs_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex organizations locations operations cancel.
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
    pub fn dataplex_organizations_locations_operations_cancel(
        &self,
        args: &DataplexOrganizationsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_organizations_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_organizations_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex organizations locations operations delete.
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
    pub fn dataplex_organizations_locations_operations_delete(
        &self,
        args: &DataplexOrganizationsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_organizations_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_organizations_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lookup context.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDataplexV1LookupContextResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_lookup_context(
        &self,
        args: &DataplexProjectsLocationsLookupContextArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDataplexV1LookupContextResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lookup_context_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lookup_context_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations search entries.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDataplexV1SearchEntriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_search_entries(
        &self,
        args: &DataplexProjectsLocationsSearchEntriesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDataplexV1SearchEntriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_search_entries_builder(
            &self.http_client,
            &args.name,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.query,
            &args.scope,
            &args.semanticSearch,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_search_entries_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations aspect types create.
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
    pub fn dataplex_projects_locations_aspect_types_create(
        &self,
        args: &DataplexProjectsLocationsAspectTypesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_aspect_types_create_builder(
            &self.http_client,
            &args.parent,
            &args.aspectTypeId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_aspect_types_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations aspect types delete.
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
    pub fn dataplex_projects_locations_aspect_types_delete(
        &self,
        args: &DataplexProjectsLocationsAspectTypesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_aspect_types_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_aspect_types_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations aspect types patch.
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
    pub fn dataplex_projects_locations_aspect_types_patch(
        &self,
        args: &DataplexProjectsLocationsAspectTypesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_aspect_types_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_aspect_types_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations aspect types set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_aspect_types_set_iam_policy(
        &self,
        args: &DataplexProjectsLocationsAspectTypesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_aspect_types_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_aspect_types_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations aspect types test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_aspect_types_test_iam_permissions(
        &self,
        args: &DataplexProjectsLocationsAspectTypesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_aspect_types_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_aspect_types_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations change requests set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_change_requests_set_iam_policy(
        &self,
        args: &DataplexProjectsLocationsChangeRequestsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_change_requests_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_change_requests_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations change requests test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_change_requests_test_iam_permissions(
        &self,
        args: &DataplexProjectsLocationsChangeRequestsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_change_requests_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_change_requests_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data attribute bindings create.
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
    pub fn dataplex_projects_locations_data_attribute_bindings_create(
        &self,
        args: &DataplexProjectsLocationsDataAttributeBindingsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_attribute_bindings_create_builder(
            &self.http_client,
            &args.parent,
            &args.dataAttributeBindingId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_attribute_bindings_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data attribute bindings delete.
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
    pub fn dataplex_projects_locations_data_attribute_bindings_delete(
        &self,
        args: &DataplexProjectsLocationsDataAttributeBindingsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_attribute_bindings_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_attribute_bindings_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data attribute bindings patch.
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
    pub fn dataplex_projects_locations_data_attribute_bindings_patch(
        &self,
        args: &DataplexProjectsLocationsDataAttributeBindingsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_attribute_bindings_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_attribute_bindings_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data attribute bindings set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_data_attribute_bindings_set_iam_policy(
        &self,
        args: &DataplexProjectsLocationsDataAttributeBindingsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_attribute_bindings_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_attribute_bindings_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data attribute bindings test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_data_attribute_bindings_test_iam_permissions(
        &self,
        args: &DataplexProjectsLocationsDataAttributeBindingsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_attribute_bindings_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_attribute_bindings_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data domains set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_data_domains_set_iam_policy(
        &self,
        args: &DataplexProjectsLocationsDataDomainsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_domains_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_domains_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data domains test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_data_domains_test_iam_permissions(
        &self,
        args: &DataplexProjectsLocationsDataDomainsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_domains_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_domains_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data products create.
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
    pub fn dataplex_projects_locations_data_products_create(
        &self,
        args: &DataplexProjectsLocationsDataProductsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_products_create_builder(
            &self.http_client,
            &args.parent,
            &args.dataProductId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_products_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data products delete.
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
    pub fn dataplex_projects_locations_data_products_delete(
        &self,
        args: &DataplexProjectsLocationsDataProductsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_products_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_products_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data products patch.
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
    pub fn dataplex_projects_locations_data_products_patch(
        &self,
        args: &DataplexProjectsLocationsDataProductsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_products_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_products_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data products set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_data_products_set_iam_policy(
        &self,
        args: &DataplexProjectsLocationsDataProductsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_products_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_products_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data products test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_data_products_test_iam_permissions(
        &self,
        args: &DataplexProjectsLocationsDataProductsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_products_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_products_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data products data assets create.
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
    pub fn dataplex_projects_locations_data_products_data_assets_create(
        &self,
        args: &DataplexProjectsLocationsDataProductsDataAssetsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_products_data_assets_create_builder(
            &self.http_client,
            &args.parent,
            &args.dataAssetId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_products_data_assets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data products data assets delete.
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
    pub fn dataplex_projects_locations_data_products_data_assets_delete(
        &self,
        args: &DataplexProjectsLocationsDataProductsDataAssetsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_products_data_assets_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_products_data_assets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data products data assets patch.
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
    pub fn dataplex_projects_locations_data_products_data_assets_patch(
        &self,
        args: &DataplexProjectsLocationsDataProductsDataAssetsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_products_data_assets_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_products_data_assets_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data scans create.
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
    pub fn dataplex_projects_locations_data_scans_create(
        &self,
        args: &DataplexProjectsLocationsDataScansCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_scans_create_builder(
            &self.http_client,
            &args.parent,
            &args.dataScanId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_scans_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data scans delete.
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
    pub fn dataplex_projects_locations_data_scans_delete(
        &self,
        args: &DataplexProjectsLocationsDataScansDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_scans_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_scans_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data scans generate data quality rules.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDataplexV1GenerateDataQualityRulesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_data_scans_generate_data_quality_rules(
        &self,
        args: &DataplexProjectsLocationsDataScansGenerateDataQualityRulesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDataplexV1GenerateDataQualityRulesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_scans_generate_data_quality_rules_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_scans_generate_data_quality_rules_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data scans patch.
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
    pub fn dataplex_projects_locations_data_scans_patch(
        &self,
        args: &DataplexProjectsLocationsDataScansPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_scans_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_scans_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data scans run.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDataplexV1RunDataScanResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_data_scans_run(
        &self,
        args: &DataplexProjectsLocationsDataScansRunArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDataplexV1RunDataScanResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_scans_run_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_scans_run_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data scans set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_data_scans_set_iam_policy(
        &self,
        args: &DataplexProjectsLocationsDataScansSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_scans_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_scans_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data scans test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_data_scans_test_iam_permissions(
        &self,
        args: &DataplexProjectsLocationsDataScansTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_scans_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_scans_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data scans jobs generate data quality rules.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDataplexV1GenerateDataQualityRulesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_data_scans_jobs_generate_data_quality_rules(
        &self,
        args: &DataplexProjectsLocationsDataScansJobsGenerateDataQualityRulesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDataplexV1GenerateDataQualityRulesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_scans_jobs_generate_data_quality_rules_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_scans_jobs_generate_data_quality_rules_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data taxonomies create.
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
    pub fn dataplex_projects_locations_data_taxonomies_create(
        &self,
        args: &DataplexProjectsLocationsDataTaxonomiesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_taxonomies_create_builder(
            &self.http_client,
            &args.parent,
            &args.dataTaxonomyId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_taxonomies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data taxonomies delete.
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
    pub fn dataplex_projects_locations_data_taxonomies_delete(
        &self,
        args: &DataplexProjectsLocationsDataTaxonomiesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_taxonomies_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_taxonomies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data taxonomies patch.
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
    pub fn dataplex_projects_locations_data_taxonomies_patch(
        &self,
        args: &DataplexProjectsLocationsDataTaxonomiesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_taxonomies_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_taxonomies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data taxonomies set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_data_taxonomies_set_iam_policy(
        &self,
        args: &DataplexProjectsLocationsDataTaxonomiesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_taxonomies_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_taxonomies_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data taxonomies test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_data_taxonomies_test_iam_permissions(
        &self,
        args: &DataplexProjectsLocationsDataTaxonomiesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_taxonomies_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_taxonomies_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data taxonomies attributes create.
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
    pub fn dataplex_projects_locations_data_taxonomies_attributes_create(
        &self,
        args: &DataplexProjectsLocationsDataTaxonomiesAttributesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_taxonomies_attributes_create_builder(
            &self.http_client,
            &args.parent,
            &args.dataAttributeId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_taxonomies_attributes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data taxonomies attributes delete.
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
    pub fn dataplex_projects_locations_data_taxonomies_attributes_delete(
        &self,
        args: &DataplexProjectsLocationsDataTaxonomiesAttributesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_taxonomies_attributes_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_taxonomies_attributes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data taxonomies attributes patch.
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
    pub fn dataplex_projects_locations_data_taxonomies_attributes_patch(
        &self,
        args: &DataplexProjectsLocationsDataTaxonomiesAttributesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_taxonomies_attributes_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_taxonomies_attributes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data taxonomies attributes set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_data_taxonomies_attributes_set_iam_policy(
        &self,
        args: &DataplexProjectsLocationsDataTaxonomiesAttributesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_taxonomies_attributes_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_taxonomies_attributes_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations data taxonomies attributes test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_data_taxonomies_attributes_test_iam_permissions(
        &self,
        args: &DataplexProjectsLocationsDataTaxonomiesAttributesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_data_taxonomies_attributes_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_data_taxonomies_attributes_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations entry groups create.
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
    pub fn dataplex_projects_locations_entry_groups_create(
        &self,
        args: &DataplexProjectsLocationsEntryGroupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_entry_groups_create_builder(
            &self.http_client,
            &args.parent,
            &args.entryGroupId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_entry_groups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations entry groups delete.
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
    pub fn dataplex_projects_locations_entry_groups_delete(
        &self,
        args: &DataplexProjectsLocationsEntryGroupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_entry_groups_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_entry_groups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations entry groups patch.
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
    pub fn dataplex_projects_locations_entry_groups_patch(
        &self,
        args: &DataplexProjectsLocationsEntryGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_entry_groups_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_entry_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations entry groups set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_entry_groups_set_iam_policy(
        &self,
        args: &DataplexProjectsLocationsEntryGroupsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_entry_groups_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_entry_groups_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations entry groups test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_entry_groups_test_iam_permissions(
        &self,
        args: &DataplexProjectsLocationsEntryGroupsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_entry_groups_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_entry_groups_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations entry groups entries create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDataplexV1Entry result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_entry_groups_entries_create(
        &self,
        args: &DataplexProjectsLocationsEntryGroupsEntriesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDataplexV1Entry, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_entry_groups_entries_create_builder(
            &self.http_client,
            &args.parent,
            &args.entryId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_entry_groups_entries_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations entry groups entries delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDataplexV1Entry result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_entry_groups_entries_delete(
        &self,
        args: &DataplexProjectsLocationsEntryGroupsEntriesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDataplexV1Entry, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_entry_groups_entries_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_entry_groups_entries_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations entry groups entries patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDataplexV1Entry result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_entry_groups_entries_patch(
        &self,
        args: &DataplexProjectsLocationsEntryGroupsEntriesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDataplexV1Entry, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_entry_groups_entries_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.aspectKeys,
            &args.deleteMissingAspects,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_entry_groups_entries_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations entry groups entry links create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDataplexV1EntryLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_entry_groups_entry_links_create(
        &self,
        args: &DataplexProjectsLocationsEntryGroupsEntryLinksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDataplexV1EntryLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_entry_groups_entry_links_create_builder(
            &self.http_client,
            &args.parent,
            &args.entryLinkId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_entry_groups_entry_links_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations entry groups entry links delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDataplexV1EntryLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_entry_groups_entry_links_delete(
        &self,
        args: &DataplexProjectsLocationsEntryGroupsEntryLinksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDataplexV1EntryLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_entry_groups_entry_links_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_entry_groups_entry_links_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations entry groups entry links patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDataplexV1EntryLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_entry_groups_entry_links_patch(
        &self,
        args: &DataplexProjectsLocationsEntryGroupsEntryLinksPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDataplexV1EntryLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_entry_groups_entry_links_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.aspectKeys,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_entry_groups_entry_links_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations entry link types set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_entry_link_types_set_iam_policy(
        &self,
        args: &DataplexProjectsLocationsEntryLinkTypesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_entry_link_types_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_entry_link_types_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations entry link types test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_entry_link_types_test_iam_permissions(
        &self,
        args: &DataplexProjectsLocationsEntryLinkTypesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_entry_link_types_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_entry_link_types_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations entry types create.
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
    pub fn dataplex_projects_locations_entry_types_create(
        &self,
        args: &DataplexProjectsLocationsEntryTypesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_entry_types_create_builder(
            &self.http_client,
            &args.parent,
            &args.entryTypeId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_entry_types_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations entry types delete.
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
    pub fn dataplex_projects_locations_entry_types_delete(
        &self,
        args: &DataplexProjectsLocationsEntryTypesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_entry_types_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_entry_types_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations entry types patch.
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
    pub fn dataplex_projects_locations_entry_types_patch(
        &self,
        args: &DataplexProjectsLocationsEntryTypesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_entry_types_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_entry_types_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations entry types set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_entry_types_set_iam_policy(
        &self,
        args: &DataplexProjectsLocationsEntryTypesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_entry_types_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_entry_types_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations entry types test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_entry_types_test_iam_permissions(
        &self,
        args: &DataplexProjectsLocationsEntryTypesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_entry_types_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_entry_types_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations glossaries create.
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
    pub fn dataplex_projects_locations_glossaries_create(
        &self,
        args: &DataplexProjectsLocationsGlossariesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_glossaries_create_builder(
            &self.http_client,
            &args.parent,
            &args.glossaryId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_glossaries_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations glossaries delete.
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
    pub fn dataplex_projects_locations_glossaries_delete(
        &self,
        args: &DataplexProjectsLocationsGlossariesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_glossaries_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_glossaries_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations glossaries patch.
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
    pub fn dataplex_projects_locations_glossaries_patch(
        &self,
        args: &DataplexProjectsLocationsGlossariesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_glossaries_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_glossaries_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations glossaries set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_glossaries_set_iam_policy(
        &self,
        args: &DataplexProjectsLocationsGlossariesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_glossaries_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_glossaries_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations glossaries test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_glossaries_test_iam_permissions(
        &self,
        args: &DataplexProjectsLocationsGlossariesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_glossaries_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_glossaries_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations glossaries categories create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDataplexV1GlossaryCategory result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_glossaries_categories_create(
        &self,
        args: &DataplexProjectsLocationsGlossariesCategoriesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDataplexV1GlossaryCategory, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_glossaries_categories_create_builder(
            &self.http_client,
            &args.parent,
            &args.categoryId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_glossaries_categories_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations glossaries categories delete.
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
    pub fn dataplex_projects_locations_glossaries_categories_delete(
        &self,
        args: &DataplexProjectsLocationsGlossariesCategoriesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_glossaries_categories_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_glossaries_categories_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations glossaries categories patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDataplexV1GlossaryCategory result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_glossaries_categories_patch(
        &self,
        args: &DataplexProjectsLocationsGlossariesCategoriesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDataplexV1GlossaryCategory, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_glossaries_categories_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_glossaries_categories_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations glossaries categories set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_glossaries_categories_set_iam_policy(
        &self,
        args: &DataplexProjectsLocationsGlossariesCategoriesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_glossaries_categories_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_glossaries_categories_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations glossaries categories test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_glossaries_categories_test_iam_permissions(
        &self,
        args: &DataplexProjectsLocationsGlossariesCategoriesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_glossaries_categories_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_glossaries_categories_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations glossaries terms create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDataplexV1GlossaryTerm result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_glossaries_terms_create(
        &self,
        args: &DataplexProjectsLocationsGlossariesTermsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDataplexV1GlossaryTerm, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_glossaries_terms_create_builder(
            &self.http_client,
            &args.parent,
            &args.termId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_glossaries_terms_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations glossaries terms delete.
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
    pub fn dataplex_projects_locations_glossaries_terms_delete(
        &self,
        args: &DataplexProjectsLocationsGlossariesTermsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_glossaries_terms_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_glossaries_terms_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations glossaries terms patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDataplexV1GlossaryTerm result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_glossaries_terms_patch(
        &self,
        args: &DataplexProjectsLocationsGlossariesTermsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDataplexV1GlossaryTerm, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_glossaries_terms_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_glossaries_terms_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations glossaries terms set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_glossaries_terms_set_iam_policy(
        &self,
        args: &DataplexProjectsLocationsGlossariesTermsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_glossaries_terms_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_glossaries_terms_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations glossaries terms test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_glossaries_terms_test_iam_permissions(
        &self,
        args: &DataplexProjectsLocationsGlossariesTermsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_glossaries_terms_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_glossaries_terms_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations governance rules set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_governance_rules_set_iam_policy(
        &self,
        args: &DataplexProjectsLocationsGovernanceRulesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_governance_rules_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_governance_rules_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations governance rules test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_governance_rules_test_iam_permissions(
        &self,
        args: &DataplexProjectsLocationsGovernanceRulesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_governance_rules_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_governance_rules_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lakes create.
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
    pub fn dataplex_projects_locations_lakes_create(
        &self,
        args: &DataplexProjectsLocationsLakesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lakes_create_builder(
            &self.http_client,
            &args.parent,
            &args.lakeId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lakes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lakes delete.
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
    pub fn dataplex_projects_locations_lakes_delete(
        &self,
        args: &DataplexProjectsLocationsLakesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lakes_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lakes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lakes patch.
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
    pub fn dataplex_projects_locations_lakes_patch(
        &self,
        args: &DataplexProjectsLocationsLakesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lakes_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lakes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lakes set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_lakes_set_iam_policy(
        &self,
        args: &DataplexProjectsLocationsLakesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lakes_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lakes_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lakes test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_lakes_test_iam_permissions(
        &self,
        args: &DataplexProjectsLocationsLakesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lakes_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lakes_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lakes environments set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_lakes_environments_set_iam_policy(
        &self,
        args: &DataplexProjectsLocationsLakesEnvironmentsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lakes_environments_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lakes_environments_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lakes environments test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_lakes_environments_test_iam_permissions(
        &self,
        args: &DataplexProjectsLocationsLakesEnvironmentsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lakes_environments_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lakes_environments_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lakes tasks create.
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
    pub fn dataplex_projects_locations_lakes_tasks_create(
        &self,
        args: &DataplexProjectsLocationsLakesTasksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lakes_tasks_create_builder(
            &self.http_client,
            &args.parent,
            &args.taskId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lakes_tasks_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lakes tasks delete.
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
    pub fn dataplex_projects_locations_lakes_tasks_delete(
        &self,
        args: &DataplexProjectsLocationsLakesTasksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lakes_tasks_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lakes_tasks_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lakes tasks patch.
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
    pub fn dataplex_projects_locations_lakes_tasks_patch(
        &self,
        args: &DataplexProjectsLocationsLakesTasksPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lakes_tasks_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lakes_tasks_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lakes tasks run.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDataplexV1RunTaskResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_lakes_tasks_run(
        &self,
        args: &DataplexProjectsLocationsLakesTasksRunArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDataplexV1RunTaskResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lakes_tasks_run_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lakes_tasks_run_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lakes tasks set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_lakes_tasks_set_iam_policy(
        &self,
        args: &DataplexProjectsLocationsLakesTasksSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lakes_tasks_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lakes_tasks_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lakes tasks test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_lakes_tasks_test_iam_permissions(
        &self,
        args: &DataplexProjectsLocationsLakesTasksTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lakes_tasks_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lakes_tasks_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lakes tasks jobs cancel.
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
    pub fn dataplex_projects_locations_lakes_tasks_jobs_cancel(
        &self,
        args: &DataplexProjectsLocationsLakesTasksJobsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lakes_tasks_jobs_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lakes_tasks_jobs_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lakes zones create.
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
    pub fn dataplex_projects_locations_lakes_zones_create(
        &self,
        args: &DataplexProjectsLocationsLakesZonesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lakes_zones_create_builder(
            &self.http_client,
            &args.parent,
            &args.validateOnly,
            &args.zoneId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lakes_zones_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lakes zones delete.
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
    pub fn dataplex_projects_locations_lakes_zones_delete(
        &self,
        args: &DataplexProjectsLocationsLakesZonesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lakes_zones_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lakes_zones_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lakes zones patch.
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
    pub fn dataplex_projects_locations_lakes_zones_patch(
        &self,
        args: &DataplexProjectsLocationsLakesZonesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lakes_zones_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lakes_zones_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lakes zones set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_lakes_zones_set_iam_policy(
        &self,
        args: &DataplexProjectsLocationsLakesZonesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lakes_zones_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lakes_zones_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lakes zones test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_lakes_zones_test_iam_permissions(
        &self,
        args: &DataplexProjectsLocationsLakesZonesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lakes_zones_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lakes_zones_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lakes zones assets create.
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
    pub fn dataplex_projects_locations_lakes_zones_assets_create(
        &self,
        args: &DataplexProjectsLocationsLakesZonesAssetsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lakes_zones_assets_create_builder(
            &self.http_client,
            &args.parent,
            &args.assetId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lakes_zones_assets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lakes zones assets delete.
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
    pub fn dataplex_projects_locations_lakes_zones_assets_delete(
        &self,
        args: &DataplexProjectsLocationsLakesZonesAssetsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lakes_zones_assets_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lakes_zones_assets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lakes zones assets patch.
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
    pub fn dataplex_projects_locations_lakes_zones_assets_patch(
        &self,
        args: &DataplexProjectsLocationsLakesZonesAssetsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lakes_zones_assets_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lakes_zones_assets_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lakes zones assets set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_lakes_zones_assets_set_iam_policy(
        &self,
        args: &DataplexProjectsLocationsLakesZonesAssetsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lakes_zones_assets_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lakes_zones_assets_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lakes zones assets test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_lakes_zones_assets_test_iam_permissions(
        &self,
        args: &DataplexProjectsLocationsLakesZonesAssetsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lakes_zones_assets_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lakes_zones_assets_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lakes zones entities create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDataplexV1Entity result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_lakes_zones_entities_create(
        &self,
        args: &DataplexProjectsLocationsLakesZonesEntitiesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDataplexV1Entity, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lakes_zones_entities_create_builder(
            &self.http_client,
            &args.parent,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lakes_zones_entities_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lakes zones entities delete.
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
    pub fn dataplex_projects_locations_lakes_zones_entities_delete(
        &self,
        args: &DataplexProjectsLocationsLakesZonesEntitiesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lakes_zones_entities_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lakes_zones_entities_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lakes zones entities update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDataplexV1Entity result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_lakes_zones_entities_update(
        &self,
        args: &DataplexProjectsLocationsLakesZonesEntitiesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDataplexV1Entity, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lakes_zones_entities_update_builder(
            &self.http_client,
            &args.name,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lakes_zones_entities_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lakes zones entities partitions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDataplexV1Partition result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_lakes_zones_entities_partitions_create(
        &self,
        args: &DataplexProjectsLocationsLakesZonesEntitiesPartitionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDataplexV1Partition, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lakes_zones_entities_partitions_create_builder(
            &self.http_client,
            &args.parent,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lakes_zones_entities_partitions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations lakes zones entities partitions delete.
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
    pub fn dataplex_projects_locations_lakes_zones_entities_partitions_delete(
        &self,
        args: &DataplexProjectsLocationsLakesZonesEntitiesPartitionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_lakes_zones_entities_partitions_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_lakes_zones_entities_partitions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations metadata feeds create.
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
    pub fn dataplex_projects_locations_metadata_feeds_create(
        &self,
        args: &DataplexProjectsLocationsMetadataFeedsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_metadata_feeds_create_builder(
            &self.http_client,
            &args.parent,
            &args.metadataFeedId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_metadata_feeds_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations metadata feeds delete.
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
    pub fn dataplex_projects_locations_metadata_feeds_delete(
        &self,
        args: &DataplexProjectsLocationsMetadataFeedsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_metadata_feeds_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_metadata_feeds_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations metadata feeds patch.
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
    pub fn dataplex_projects_locations_metadata_feeds_patch(
        &self,
        args: &DataplexProjectsLocationsMetadataFeedsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_metadata_feeds_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_metadata_feeds_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations metadata jobs cancel.
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
    pub fn dataplex_projects_locations_metadata_jobs_cancel(
        &self,
        args: &DataplexProjectsLocationsMetadataJobsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_metadata_jobs_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_metadata_jobs_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations metadata jobs create.
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
    pub fn dataplex_projects_locations_metadata_jobs_create(
        &self,
        args: &DataplexProjectsLocationsMetadataJobsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_metadata_jobs_create_builder(
            &self.http_client,
            &args.parent,
            &args.metadataJobId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_metadata_jobs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations operations cancel.
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
    pub fn dataplex_projects_locations_operations_cancel(
        &self,
        args: &DataplexProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations operations delete.
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
    pub fn dataplex_projects_locations_operations_delete(
        &self,
        args: &DataplexProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations policy intents set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_policy_intents_set_iam_policy(
        &self,
        args: &DataplexProjectsLocationsPolicyIntentsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_policy_intents_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_policy_intents_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataplex projects locations policy intents test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataplex_projects_locations_policy_intents_test_iam_permissions(
        &self,
        args: &DataplexProjectsLocationsPolicyIntentsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataplex_projects_locations_policy_intents_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataplex_projects_locations_policy_intents_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
