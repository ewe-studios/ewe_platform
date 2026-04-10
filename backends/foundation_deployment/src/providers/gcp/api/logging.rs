//! LoggingProvider - State-aware logging API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       logging API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::logging::{
    logging_billing_accounts_get_cmek_settings_builder, logging_billing_accounts_get_cmek_settings_task,
    logging_billing_accounts_get_settings_builder, logging_billing_accounts_get_settings_task,
    logging_billing_accounts_exclusions_create_builder, logging_billing_accounts_exclusions_create_task,
    logging_billing_accounts_exclusions_delete_builder, logging_billing_accounts_exclusions_delete_task,
    logging_billing_accounts_exclusions_get_builder, logging_billing_accounts_exclusions_get_task,
    logging_billing_accounts_exclusions_list_builder, logging_billing_accounts_exclusions_list_task,
    logging_billing_accounts_exclusions_patch_builder, logging_billing_accounts_exclusions_patch_task,
    logging_billing_accounts_locations_get_builder, logging_billing_accounts_locations_get_task,
    logging_billing_accounts_locations_list_builder, logging_billing_accounts_locations_list_task,
    logging_billing_accounts_locations_buckets_create_builder, logging_billing_accounts_locations_buckets_create_task,
    logging_billing_accounts_locations_buckets_create_async_builder, logging_billing_accounts_locations_buckets_create_async_task,
    logging_billing_accounts_locations_buckets_delete_builder, logging_billing_accounts_locations_buckets_delete_task,
    logging_billing_accounts_locations_buckets_get_builder, logging_billing_accounts_locations_buckets_get_task,
    logging_billing_accounts_locations_buckets_list_builder, logging_billing_accounts_locations_buckets_list_task,
    logging_billing_accounts_locations_buckets_patch_builder, logging_billing_accounts_locations_buckets_patch_task,
    logging_billing_accounts_locations_buckets_undelete_builder, logging_billing_accounts_locations_buckets_undelete_task,
    logging_billing_accounts_locations_buckets_update_async_builder, logging_billing_accounts_locations_buckets_update_async_task,
    logging_billing_accounts_locations_buckets_links_create_builder, logging_billing_accounts_locations_buckets_links_create_task,
    logging_billing_accounts_locations_buckets_links_delete_builder, logging_billing_accounts_locations_buckets_links_delete_task,
    logging_billing_accounts_locations_buckets_links_get_builder, logging_billing_accounts_locations_buckets_links_get_task,
    logging_billing_accounts_locations_buckets_links_list_builder, logging_billing_accounts_locations_buckets_links_list_task,
    logging_billing_accounts_locations_buckets_views_create_builder, logging_billing_accounts_locations_buckets_views_create_task,
    logging_billing_accounts_locations_buckets_views_delete_builder, logging_billing_accounts_locations_buckets_views_delete_task,
    logging_billing_accounts_locations_buckets_views_get_builder, logging_billing_accounts_locations_buckets_views_get_task,
    logging_billing_accounts_locations_buckets_views_list_builder, logging_billing_accounts_locations_buckets_views_list_task,
    logging_billing_accounts_locations_buckets_views_patch_builder, logging_billing_accounts_locations_buckets_views_patch_task,
    logging_billing_accounts_locations_buckets_views_logs_list_builder, logging_billing_accounts_locations_buckets_views_logs_list_task,
    logging_billing_accounts_locations_operations_cancel_builder, logging_billing_accounts_locations_operations_cancel_task,
    logging_billing_accounts_locations_operations_get_builder, logging_billing_accounts_locations_operations_get_task,
    logging_billing_accounts_locations_operations_list_builder, logging_billing_accounts_locations_operations_list_task,
    logging_billing_accounts_locations_recent_queries_list_builder, logging_billing_accounts_locations_recent_queries_list_task,
    logging_billing_accounts_locations_saved_queries_create_builder, logging_billing_accounts_locations_saved_queries_create_task,
    logging_billing_accounts_locations_saved_queries_delete_builder, logging_billing_accounts_locations_saved_queries_delete_task,
    logging_billing_accounts_locations_saved_queries_get_builder, logging_billing_accounts_locations_saved_queries_get_task,
    logging_billing_accounts_locations_saved_queries_list_builder, logging_billing_accounts_locations_saved_queries_list_task,
    logging_billing_accounts_locations_saved_queries_patch_builder, logging_billing_accounts_locations_saved_queries_patch_task,
    logging_billing_accounts_logs_delete_builder, logging_billing_accounts_logs_delete_task,
    logging_billing_accounts_logs_list_builder, logging_billing_accounts_logs_list_task,
    logging_billing_accounts_sinks_create_builder, logging_billing_accounts_sinks_create_task,
    logging_billing_accounts_sinks_delete_builder, logging_billing_accounts_sinks_delete_task,
    logging_billing_accounts_sinks_get_builder, logging_billing_accounts_sinks_get_task,
    logging_billing_accounts_sinks_list_builder, logging_billing_accounts_sinks_list_task,
    logging_billing_accounts_sinks_patch_builder, logging_billing_accounts_sinks_patch_task,
    logging_billing_accounts_sinks_update_builder, logging_billing_accounts_sinks_update_task,
    logging_entries_copy_builder, logging_entries_copy_task,
    logging_entries_list_builder, logging_entries_list_task,
    logging_entries_tail_builder, logging_entries_tail_task,
    logging_entries_write_builder, logging_entries_write_task,
    logging_exclusions_create_builder, logging_exclusions_create_task,
    logging_exclusions_delete_builder, logging_exclusions_delete_task,
    logging_exclusions_get_builder, logging_exclusions_get_task,
    logging_exclusions_list_builder, logging_exclusions_list_task,
    logging_exclusions_patch_builder, logging_exclusions_patch_task,
    logging_folders_get_cmek_settings_builder, logging_folders_get_cmek_settings_task,
    logging_folders_get_settings_builder, logging_folders_get_settings_task,
    logging_folders_update_settings_builder, logging_folders_update_settings_task,
    logging_folders_exclusions_create_builder, logging_folders_exclusions_create_task,
    logging_folders_exclusions_delete_builder, logging_folders_exclusions_delete_task,
    logging_folders_exclusions_get_builder, logging_folders_exclusions_get_task,
    logging_folders_exclusions_list_builder, logging_folders_exclusions_list_task,
    logging_folders_exclusions_patch_builder, logging_folders_exclusions_patch_task,
    logging_folders_locations_get_builder, logging_folders_locations_get_task,
    logging_folders_locations_list_builder, logging_folders_locations_list_task,
    logging_folders_locations_buckets_create_builder, logging_folders_locations_buckets_create_task,
    logging_folders_locations_buckets_create_async_builder, logging_folders_locations_buckets_create_async_task,
    logging_folders_locations_buckets_delete_builder, logging_folders_locations_buckets_delete_task,
    logging_folders_locations_buckets_get_builder, logging_folders_locations_buckets_get_task,
    logging_folders_locations_buckets_list_builder, logging_folders_locations_buckets_list_task,
    logging_folders_locations_buckets_patch_builder, logging_folders_locations_buckets_patch_task,
    logging_folders_locations_buckets_undelete_builder, logging_folders_locations_buckets_undelete_task,
    logging_folders_locations_buckets_update_async_builder, logging_folders_locations_buckets_update_async_task,
    logging_folders_locations_buckets_links_create_builder, logging_folders_locations_buckets_links_create_task,
    logging_folders_locations_buckets_links_delete_builder, logging_folders_locations_buckets_links_delete_task,
    logging_folders_locations_buckets_links_get_builder, logging_folders_locations_buckets_links_get_task,
    logging_folders_locations_buckets_links_list_builder, logging_folders_locations_buckets_links_list_task,
    logging_folders_locations_buckets_views_create_builder, logging_folders_locations_buckets_views_create_task,
    logging_folders_locations_buckets_views_delete_builder, logging_folders_locations_buckets_views_delete_task,
    logging_folders_locations_buckets_views_get_builder, logging_folders_locations_buckets_views_get_task,
    logging_folders_locations_buckets_views_get_iam_policy_builder, logging_folders_locations_buckets_views_get_iam_policy_task,
    logging_folders_locations_buckets_views_list_builder, logging_folders_locations_buckets_views_list_task,
    logging_folders_locations_buckets_views_patch_builder, logging_folders_locations_buckets_views_patch_task,
    logging_folders_locations_buckets_views_set_iam_policy_builder, logging_folders_locations_buckets_views_set_iam_policy_task,
    logging_folders_locations_buckets_views_test_iam_permissions_builder, logging_folders_locations_buckets_views_test_iam_permissions_task,
    logging_folders_locations_buckets_views_logs_list_builder, logging_folders_locations_buckets_views_logs_list_task,
    logging_folders_locations_log_scopes_create_builder, logging_folders_locations_log_scopes_create_task,
    logging_folders_locations_log_scopes_delete_builder, logging_folders_locations_log_scopes_delete_task,
    logging_folders_locations_log_scopes_get_builder, logging_folders_locations_log_scopes_get_task,
    logging_folders_locations_log_scopes_list_builder, logging_folders_locations_log_scopes_list_task,
    logging_folders_locations_log_scopes_patch_builder, logging_folders_locations_log_scopes_patch_task,
    logging_folders_locations_operations_cancel_builder, logging_folders_locations_operations_cancel_task,
    logging_folders_locations_operations_get_builder, logging_folders_locations_operations_get_task,
    logging_folders_locations_operations_list_builder, logging_folders_locations_operations_list_task,
    logging_folders_locations_recent_queries_list_builder, logging_folders_locations_recent_queries_list_task,
    logging_folders_locations_saved_queries_create_builder, logging_folders_locations_saved_queries_create_task,
    logging_folders_locations_saved_queries_delete_builder, logging_folders_locations_saved_queries_delete_task,
    logging_folders_locations_saved_queries_get_builder, logging_folders_locations_saved_queries_get_task,
    logging_folders_locations_saved_queries_list_builder, logging_folders_locations_saved_queries_list_task,
    logging_folders_locations_saved_queries_patch_builder, logging_folders_locations_saved_queries_patch_task,
    logging_folders_logs_delete_builder, logging_folders_logs_delete_task,
    logging_folders_logs_list_builder, logging_folders_logs_list_task,
    logging_folders_sinks_create_builder, logging_folders_sinks_create_task,
    logging_folders_sinks_delete_builder, logging_folders_sinks_delete_task,
    logging_folders_sinks_get_builder, logging_folders_sinks_get_task,
    logging_folders_sinks_list_builder, logging_folders_sinks_list_task,
    logging_folders_sinks_patch_builder, logging_folders_sinks_patch_task,
    logging_folders_sinks_update_builder, logging_folders_sinks_update_task,
    logging_locations_get_builder, logging_locations_get_task,
    logging_locations_list_builder, logging_locations_list_task,
    logging_locations_buckets_create_builder, logging_locations_buckets_create_task,
    logging_locations_buckets_create_async_builder, logging_locations_buckets_create_async_task,
    logging_locations_buckets_delete_builder, logging_locations_buckets_delete_task,
    logging_locations_buckets_get_builder, logging_locations_buckets_get_task,
    logging_locations_buckets_list_builder, logging_locations_buckets_list_task,
    logging_locations_buckets_patch_builder, logging_locations_buckets_patch_task,
    logging_locations_buckets_undelete_builder, logging_locations_buckets_undelete_task,
    logging_locations_buckets_update_async_builder, logging_locations_buckets_update_async_task,
    logging_locations_buckets_links_create_builder, logging_locations_buckets_links_create_task,
    logging_locations_buckets_links_delete_builder, logging_locations_buckets_links_delete_task,
    logging_locations_buckets_links_get_builder, logging_locations_buckets_links_get_task,
    logging_locations_buckets_links_list_builder, logging_locations_buckets_links_list_task,
    logging_locations_buckets_views_create_builder, logging_locations_buckets_views_create_task,
    logging_locations_buckets_views_delete_builder, logging_locations_buckets_views_delete_task,
    logging_locations_buckets_views_get_builder, logging_locations_buckets_views_get_task,
    logging_locations_buckets_views_get_iam_policy_builder, logging_locations_buckets_views_get_iam_policy_task,
    logging_locations_buckets_views_list_builder, logging_locations_buckets_views_list_task,
    logging_locations_buckets_views_patch_builder, logging_locations_buckets_views_patch_task,
    logging_locations_buckets_views_set_iam_policy_builder, logging_locations_buckets_views_set_iam_policy_task,
    logging_locations_buckets_views_test_iam_permissions_builder, logging_locations_buckets_views_test_iam_permissions_task,
    logging_locations_operations_cancel_builder, logging_locations_operations_cancel_task,
    logging_locations_operations_get_builder, logging_locations_operations_get_task,
    logging_locations_operations_list_builder, logging_locations_operations_list_task,
    logging_logs_delete_builder, logging_logs_delete_task,
    logging_logs_list_builder, logging_logs_list_task,
    logging_monitored_resource_descriptors_list_builder, logging_monitored_resource_descriptors_list_task,
    logging_organizations_get_cmek_settings_builder, logging_organizations_get_cmek_settings_task,
    logging_organizations_get_settings_builder, logging_organizations_get_settings_task,
    logging_organizations_update_cmek_settings_builder, logging_organizations_update_cmek_settings_task,
    logging_organizations_update_settings_builder, logging_organizations_update_settings_task,
    logging_organizations_exclusions_create_builder, logging_organizations_exclusions_create_task,
    logging_organizations_exclusions_delete_builder, logging_organizations_exclusions_delete_task,
    logging_organizations_exclusions_get_builder, logging_organizations_exclusions_get_task,
    logging_organizations_exclusions_list_builder, logging_organizations_exclusions_list_task,
    logging_organizations_exclusions_patch_builder, logging_organizations_exclusions_patch_task,
    logging_organizations_locations_get_builder, logging_organizations_locations_get_task,
    logging_organizations_locations_list_builder, logging_organizations_locations_list_task,
    logging_organizations_locations_buckets_create_builder, logging_organizations_locations_buckets_create_task,
    logging_organizations_locations_buckets_create_async_builder, logging_organizations_locations_buckets_create_async_task,
    logging_organizations_locations_buckets_delete_builder, logging_organizations_locations_buckets_delete_task,
    logging_organizations_locations_buckets_get_builder, logging_organizations_locations_buckets_get_task,
    logging_organizations_locations_buckets_list_builder, logging_organizations_locations_buckets_list_task,
    logging_organizations_locations_buckets_patch_builder, logging_organizations_locations_buckets_patch_task,
    logging_organizations_locations_buckets_undelete_builder, logging_organizations_locations_buckets_undelete_task,
    logging_organizations_locations_buckets_update_async_builder, logging_organizations_locations_buckets_update_async_task,
    logging_organizations_locations_buckets_links_create_builder, logging_organizations_locations_buckets_links_create_task,
    logging_organizations_locations_buckets_links_delete_builder, logging_organizations_locations_buckets_links_delete_task,
    logging_organizations_locations_buckets_links_get_builder, logging_organizations_locations_buckets_links_get_task,
    logging_organizations_locations_buckets_links_list_builder, logging_organizations_locations_buckets_links_list_task,
    logging_organizations_locations_buckets_views_create_builder, logging_organizations_locations_buckets_views_create_task,
    logging_organizations_locations_buckets_views_delete_builder, logging_organizations_locations_buckets_views_delete_task,
    logging_organizations_locations_buckets_views_get_builder, logging_organizations_locations_buckets_views_get_task,
    logging_organizations_locations_buckets_views_get_iam_policy_builder, logging_organizations_locations_buckets_views_get_iam_policy_task,
    logging_organizations_locations_buckets_views_list_builder, logging_organizations_locations_buckets_views_list_task,
    logging_organizations_locations_buckets_views_patch_builder, logging_organizations_locations_buckets_views_patch_task,
    logging_organizations_locations_buckets_views_set_iam_policy_builder, logging_organizations_locations_buckets_views_set_iam_policy_task,
    logging_organizations_locations_buckets_views_test_iam_permissions_builder, logging_organizations_locations_buckets_views_test_iam_permissions_task,
    logging_organizations_locations_buckets_views_logs_list_builder, logging_organizations_locations_buckets_views_logs_list_task,
    logging_organizations_locations_log_scopes_create_builder, logging_organizations_locations_log_scopes_create_task,
    logging_organizations_locations_log_scopes_delete_builder, logging_organizations_locations_log_scopes_delete_task,
    logging_organizations_locations_log_scopes_get_builder, logging_organizations_locations_log_scopes_get_task,
    logging_organizations_locations_log_scopes_list_builder, logging_organizations_locations_log_scopes_list_task,
    logging_organizations_locations_log_scopes_patch_builder, logging_organizations_locations_log_scopes_patch_task,
    logging_organizations_locations_operations_cancel_builder, logging_organizations_locations_operations_cancel_task,
    logging_organizations_locations_operations_get_builder, logging_organizations_locations_operations_get_task,
    logging_organizations_locations_operations_list_builder, logging_organizations_locations_operations_list_task,
    logging_organizations_locations_recent_queries_list_builder, logging_organizations_locations_recent_queries_list_task,
    logging_organizations_locations_saved_queries_create_builder, logging_organizations_locations_saved_queries_create_task,
    logging_organizations_locations_saved_queries_delete_builder, logging_organizations_locations_saved_queries_delete_task,
    logging_organizations_locations_saved_queries_get_builder, logging_organizations_locations_saved_queries_get_task,
    logging_organizations_locations_saved_queries_list_builder, logging_organizations_locations_saved_queries_list_task,
    logging_organizations_locations_saved_queries_patch_builder, logging_organizations_locations_saved_queries_patch_task,
    logging_organizations_logs_delete_builder, logging_organizations_logs_delete_task,
    logging_organizations_logs_list_builder, logging_organizations_logs_list_task,
    logging_organizations_sinks_create_builder, logging_organizations_sinks_create_task,
    logging_organizations_sinks_delete_builder, logging_organizations_sinks_delete_task,
    logging_organizations_sinks_get_builder, logging_organizations_sinks_get_task,
    logging_organizations_sinks_list_builder, logging_organizations_sinks_list_task,
    logging_organizations_sinks_patch_builder, logging_organizations_sinks_patch_task,
    logging_organizations_sinks_update_builder, logging_organizations_sinks_update_task,
    logging_projects_get_cmek_settings_builder, logging_projects_get_cmek_settings_task,
    logging_projects_get_settings_builder, logging_projects_get_settings_task,
    logging_projects_exclusions_create_builder, logging_projects_exclusions_create_task,
    logging_projects_exclusions_delete_builder, logging_projects_exclusions_delete_task,
    logging_projects_exclusions_get_builder, logging_projects_exclusions_get_task,
    logging_projects_exclusions_list_builder, logging_projects_exclusions_list_task,
    logging_projects_exclusions_patch_builder, logging_projects_exclusions_patch_task,
    logging_projects_locations_get_builder, logging_projects_locations_get_task,
    logging_projects_locations_list_builder, logging_projects_locations_list_task,
    logging_projects_locations_buckets_create_builder, logging_projects_locations_buckets_create_task,
    logging_projects_locations_buckets_create_async_builder, logging_projects_locations_buckets_create_async_task,
    logging_projects_locations_buckets_delete_builder, logging_projects_locations_buckets_delete_task,
    logging_projects_locations_buckets_get_builder, logging_projects_locations_buckets_get_task,
    logging_projects_locations_buckets_list_builder, logging_projects_locations_buckets_list_task,
    logging_projects_locations_buckets_patch_builder, logging_projects_locations_buckets_patch_task,
    logging_projects_locations_buckets_undelete_builder, logging_projects_locations_buckets_undelete_task,
    logging_projects_locations_buckets_update_async_builder, logging_projects_locations_buckets_update_async_task,
    logging_projects_locations_buckets_links_create_builder, logging_projects_locations_buckets_links_create_task,
    logging_projects_locations_buckets_links_delete_builder, logging_projects_locations_buckets_links_delete_task,
    logging_projects_locations_buckets_links_get_builder, logging_projects_locations_buckets_links_get_task,
    logging_projects_locations_buckets_links_list_builder, logging_projects_locations_buckets_links_list_task,
    logging_projects_locations_buckets_views_create_builder, logging_projects_locations_buckets_views_create_task,
    logging_projects_locations_buckets_views_delete_builder, logging_projects_locations_buckets_views_delete_task,
    logging_projects_locations_buckets_views_get_builder, logging_projects_locations_buckets_views_get_task,
    logging_projects_locations_buckets_views_get_iam_policy_builder, logging_projects_locations_buckets_views_get_iam_policy_task,
    logging_projects_locations_buckets_views_list_builder, logging_projects_locations_buckets_views_list_task,
    logging_projects_locations_buckets_views_patch_builder, logging_projects_locations_buckets_views_patch_task,
    logging_projects_locations_buckets_views_set_iam_policy_builder, logging_projects_locations_buckets_views_set_iam_policy_task,
    logging_projects_locations_buckets_views_test_iam_permissions_builder, logging_projects_locations_buckets_views_test_iam_permissions_task,
    logging_projects_locations_buckets_views_logs_list_builder, logging_projects_locations_buckets_views_logs_list_task,
    logging_projects_locations_log_scopes_create_builder, logging_projects_locations_log_scopes_create_task,
    logging_projects_locations_log_scopes_delete_builder, logging_projects_locations_log_scopes_delete_task,
    logging_projects_locations_log_scopes_get_builder, logging_projects_locations_log_scopes_get_task,
    logging_projects_locations_log_scopes_list_builder, logging_projects_locations_log_scopes_list_task,
    logging_projects_locations_log_scopes_patch_builder, logging_projects_locations_log_scopes_patch_task,
    logging_projects_locations_operations_cancel_builder, logging_projects_locations_operations_cancel_task,
    logging_projects_locations_operations_get_builder, logging_projects_locations_operations_get_task,
    logging_projects_locations_operations_list_builder, logging_projects_locations_operations_list_task,
    logging_projects_locations_recent_queries_list_builder, logging_projects_locations_recent_queries_list_task,
    logging_projects_locations_saved_queries_create_builder, logging_projects_locations_saved_queries_create_task,
    logging_projects_locations_saved_queries_delete_builder, logging_projects_locations_saved_queries_delete_task,
    logging_projects_locations_saved_queries_get_builder, logging_projects_locations_saved_queries_get_task,
    logging_projects_locations_saved_queries_list_builder, logging_projects_locations_saved_queries_list_task,
    logging_projects_locations_saved_queries_patch_builder, logging_projects_locations_saved_queries_patch_task,
    logging_projects_logs_delete_builder, logging_projects_logs_delete_task,
    logging_projects_logs_list_builder, logging_projects_logs_list_task,
    logging_projects_metrics_create_builder, logging_projects_metrics_create_task,
    logging_projects_metrics_delete_builder, logging_projects_metrics_delete_task,
    logging_projects_metrics_get_builder, logging_projects_metrics_get_task,
    logging_projects_metrics_list_builder, logging_projects_metrics_list_task,
    logging_projects_metrics_update_builder, logging_projects_metrics_update_task,
    logging_projects_sinks_create_builder, logging_projects_sinks_create_task,
    logging_projects_sinks_delete_builder, logging_projects_sinks_delete_task,
    logging_projects_sinks_get_builder, logging_projects_sinks_get_task,
    logging_projects_sinks_list_builder, logging_projects_sinks_list_task,
    logging_projects_sinks_patch_builder, logging_projects_sinks_patch_task,
    logging_projects_sinks_update_builder, logging_projects_sinks_update_task,
    logging_sinks_create_builder, logging_sinks_create_task,
    logging_sinks_delete_builder, logging_sinks_delete_task,
    logging_sinks_get_builder, logging_sinks_get_task,
    logging_sinks_list_builder, logging_sinks_list_task,
    logging_sinks_update_builder, logging_sinks_update_task,
    logging_get_cmek_settings_builder, logging_get_cmek_settings_task,
    logging_get_settings_builder, logging_get_settings_task,
    logging_update_cmek_settings_builder, logging_update_cmek_settings_task,
    logging_update_settings_builder, logging_update_settings_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::logging::CmekSettings;
use crate::providers::gcp::clients::logging::Empty;
use crate::providers::gcp::clients::logging::Link;
use crate::providers::gcp::clients::logging::ListBucketsResponse;
use crate::providers::gcp::clients::logging::ListExclusionsResponse;
use crate::providers::gcp::clients::logging::ListLinksResponse;
use crate::providers::gcp::clients::logging::ListLocationsResponse;
use crate::providers::gcp::clients::logging::ListLogEntriesResponse;
use crate::providers::gcp::clients::logging::ListLogMetricsResponse;
use crate::providers::gcp::clients::logging::ListLogScopesResponse;
use crate::providers::gcp::clients::logging::ListLogsResponse;
use crate::providers::gcp::clients::logging::ListMonitoredResourceDescriptorsResponse;
use crate::providers::gcp::clients::logging::ListOperationsResponse;
use crate::providers::gcp::clients::logging::ListRecentQueriesResponse;
use crate::providers::gcp::clients::logging::ListSavedQueriesResponse;
use crate::providers::gcp::clients::logging::ListSinksResponse;
use crate::providers::gcp::clients::logging::ListViewsResponse;
use crate::providers::gcp::clients::logging::Location;
use crate::providers::gcp::clients::logging::LogBucket;
use crate::providers::gcp::clients::logging::LogExclusion;
use crate::providers::gcp::clients::logging::LogMetric;
use crate::providers::gcp::clients::logging::LogScope;
use crate::providers::gcp::clients::logging::LogSink;
use crate::providers::gcp::clients::logging::LogView;
use crate::providers::gcp::clients::logging::Operation;
use crate::providers::gcp::clients::logging::Policy;
use crate::providers::gcp::clients::logging::SavedQuery;
use crate::providers::gcp::clients::logging::Settings;
use crate::providers::gcp::clients::logging::TailLogEntriesResponse;
use crate::providers::gcp::clients::logging::TestIamPermissionsResponse;
use crate::providers::gcp::clients::logging::WriteLogEntriesResponse;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsExclusionsCreateArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsExclusionsDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsExclusionsGetArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsExclusionsListArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsExclusionsPatchArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsGetCmekSettingsArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsGetSettingsArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLocationsBucketsCreateArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLocationsBucketsCreateAsyncArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLocationsBucketsDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLocationsBucketsGetArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLocationsBucketsLinksCreateArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLocationsBucketsLinksDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLocationsBucketsLinksGetArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLocationsBucketsLinksListArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLocationsBucketsListArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLocationsBucketsPatchArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLocationsBucketsUndeleteArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLocationsBucketsUpdateAsyncArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLocationsBucketsViewsCreateArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLocationsBucketsViewsDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLocationsBucketsViewsGetArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLocationsBucketsViewsListArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLocationsBucketsViewsLogsListArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLocationsBucketsViewsPatchArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLocationsGetArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLocationsListArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLocationsOperationsListArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLocationsRecentQueriesListArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLocationsSavedQueriesCreateArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLocationsSavedQueriesDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLocationsSavedQueriesGetArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLocationsSavedQueriesListArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLocationsSavedQueriesPatchArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLogsDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsLogsListArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsSinksCreateArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsSinksDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsSinksGetArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsSinksListArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsSinksPatchArgs;
use crate::providers::gcp::clients::logging::LoggingBillingAccountsSinksUpdateArgs;
use crate::providers::gcp::clients::logging::LoggingEntriesCopyArgs;
use crate::providers::gcp::clients::logging::LoggingEntriesListArgs;
use crate::providers::gcp::clients::logging::LoggingEntriesTailArgs;
use crate::providers::gcp::clients::logging::LoggingEntriesWriteArgs;
use crate::providers::gcp::clients::logging::LoggingExclusionsCreateArgs;
use crate::providers::gcp::clients::logging::LoggingExclusionsDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingExclusionsGetArgs;
use crate::providers::gcp::clients::logging::LoggingExclusionsListArgs;
use crate::providers::gcp::clients::logging::LoggingExclusionsPatchArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersExclusionsCreateArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersExclusionsDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersExclusionsGetArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersExclusionsListArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersExclusionsPatchArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersGetCmekSettingsArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersGetSettingsArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsBucketsCreateArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsBucketsCreateAsyncArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsBucketsDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsBucketsGetArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsBucketsLinksCreateArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsBucketsLinksDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsBucketsLinksGetArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsBucketsLinksListArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsBucketsListArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsBucketsPatchArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsBucketsUndeleteArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsBucketsUpdateAsyncArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsBucketsViewsCreateArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsBucketsViewsDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsBucketsViewsGetArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsBucketsViewsGetIamPolicyArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsBucketsViewsListArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsBucketsViewsLogsListArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsBucketsViewsPatchArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsBucketsViewsSetIamPolicyArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsBucketsViewsTestIamPermissionsArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsGetArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsListArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsLogScopesCreateArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsLogScopesDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsLogScopesGetArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsLogScopesListArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsLogScopesPatchArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsOperationsGetArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsOperationsListArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsRecentQueriesListArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsSavedQueriesCreateArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsSavedQueriesDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsSavedQueriesGetArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsSavedQueriesListArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLocationsSavedQueriesPatchArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLogsDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersLogsListArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersSinksCreateArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersSinksDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersSinksGetArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersSinksListArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersSinksPatchArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersSinksUpdateArgs;
use crate::providers::gcp::clients::logging::LoggingFoldersUpdateSettingsArgs;
use crate::providers::gcp::clients::logging::LoggingGetCmekSettingsArgs;
use crate::providers::gcp::clients::logging::LoggingGetSettingsArgs;
use crate::providers::gcp::clients::logging::LoggingLocationsBucketsCreateArgs;
use crate::providers::gcp::clients::logging::LoggingLocationsBucketsCreateAsyncArgs;
use crate::providers::gcp::clients::logging::LoggingLocationsBucketsDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingLocationsBucketsGetArgs;
use crate::providers::gcp::clients::logging::LoggingLocationsBucketsLinksCreateArgs;
use crate::providers::gcp::clients::logging::LoggingLocationsBucketsLinksDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingLocationsBucketsLinksGetArgs;
use crate::providers::gcp::clients::logging::LoggingLocationsBucketsLinksListArgs;
use crate::providers::gcp::clients::logging::LoggingLocationsBucketsListArgs;
use crate::providers::gcp::clients::logging::LoggingLocationsBucketsPatchArgs;
use crate::providers::gcp::clients::logging::LoggingLocationsBucketsUndeleteArgs;
use crate::providers::gcp::clients::logging::LoggingLocationsBucketsUpdateAsyncArgs;
use crate::providers::gcp::clients::logging::LoggingLocationsBucketsViewsCreateArgs;
use crate::providers::gcp::clients::logging::LoggingLocationsBucketsViewsDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingLocationsBucketsViewsGetArgs;
use crate::providers::gcp::clients::logging::LoggingLocationsBucketsViewsGetIamPolicyArgs;
use crate::providers::gcp::clients::logging::LoggingLocationsBucketsViewsListArgs;
use crate::providers::gcp::clients::logging::LoggingLocationsBucketsViewsPatchArgs;
use crate::providers::gcp::clients::logging::LoggingLocationsBucketsViewsSetIamPolicyArgs;
use crate::providers::gcp::clients::logging::LoggingLocationsBucketsViewsTestIamPermissionsArgs;
use crate::providers::gcp::clients::logging::LoggingLocationsGetArgs;
use crate::providers::gcp::clients::logging::LoggingLocationsListArgs;
use crate::providers::gcp::clients::logging::LoggingLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::logging::LoggingLocationsOperationsGetArgs;
use crate::providers::gcp::clients::logging::LoggingLocationsOperationsListArgs;
use crate::providers::gcp::clients::logging::LoggingLogsDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingLogsListArgs;
use crate::providers::gcp::clients::logging::LoggingMonitoredResourceDescriptorsListArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsExclusionsCreateArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsExclusionsDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsExclusionsGetArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsExclusionsListArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsExclusionsPatchArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsGetCmekSettingsArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsGetSettingsArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsBucketsCreateArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsBucketsCreateAsyncArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsBucketsDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsBucketsGetArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsBucketsLinksCreateArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsBucketsLinksDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsBucketsLinksGetArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsBucketsLinksListArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsBucketsListArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsBucketsPatchArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsBucketsUndeleteArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsBucketsUpdateAsyncArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsBucketsViewsCreateArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsBucketsViewsDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsBucketsViewsGetArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsBucketsViewsGetIamPolicyArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsBucketsViewsListArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsBucketsViewsLogsListArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsBucketsViewsPatchArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsBucketsViewsSetIamPolicyArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsBucketsViewsTestIamPermissionsArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsGetArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsListArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsLogScopesCreateArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsLogScopesDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsLogScopesGetArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsLogScopesListArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsLogScopesPatchArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsOperationsListArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsRecentQueriesListArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsSavedQueriesCreateArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsSavedQueriesDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsSavedQueriesGetArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsSavedQueriesListArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLocationsSavedQueriesPatchArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLogsDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsLogsListArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsSinksCreateArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsSinksDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsSinksGetArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsSinksListArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsSinksPatchArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsSinksUpdateArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsUpdateCmekSettingsArgs;
use crate::providers::gcp::clients::logging::LoggingOrganizationsUpdateSettingsArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsExclusionsCreateArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsExclusionsDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsExclusionsGetArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsExclusionsListArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsExclusionsPatchArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsGetCmekSettingsArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsGetSettingsArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsBucketsCreateArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsBucketsCreateAsyncArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsBucketsDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsBucketsGetArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsBucketsLinksCreateArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsBucketsLinksDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsBucketsLinksGetArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsBucketsLinksListArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsBucketsListArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsBucketsPatchArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsBucketsUndeleteArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsBucketsUpdateAsyncArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsBucketsViewsCreateArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsBucketsViewsDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsBucketsViewsGetArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsBucketsViewsGetIamPolicyArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsBucketsViewsListArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsBucketsViewsLogsListArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsBucketsViewsPatchArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsBucketsViewsSetIamPolicyArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsBucketsViewsTestIamPermissionsArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsGetArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsListArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsLogScopesCreateArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsLogScopesDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsLogScopesGetArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsLogScopesListArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsLogScopesPatchArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsRecentQueriesListArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsSavedQueriesCreateArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsSavedQueriesDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsSavedQueriesGetArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsSavedQueriesListArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLocationsSavedQueriesPatchArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLogsDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsLogsListArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsMetricsCreateArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsMetricsDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsMetricsGetArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsMetricsListArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsMetricsUpdateArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsSinksCreateArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsSinksDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsSinksGetArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsSinksListArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsSinksPatchArgs;
use crate::providers::gcp::clients::logging::LoggingProjectsSinksUpdateArgs;
use crate::providers::gcp::clients::logging::LoggingSinksCreateArgs;
use crate::providers::gcp::clients::logging::LoggingSinksDeleteArgs;
use crate::providers::gcp::clients::logging::LoggingSinksGetArgs;
use crate::providers::gcp::clients::logging::LoggingSinksListArgs;
use crate::providers::gcp::clients::logging::LoggingSinksUpdateArgs;
use crate::providers::gcp::clients::logging::LoggingUpdateCmekSettingsArgs;
use crate::providers::gcp::clients::logging::LoggingUpdateSettingsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// LoggingProvider with automatic state tracking.
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
/// let provider = LoggingProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct LoggingProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> LoggingProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new LoggingProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Logging billing accounts get cmek settings.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CmekSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_billing_accounts_get_cmek_settings(
        &self,
        args: &LoggingBillingAccountsGetCmekSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CmekSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_get_cmek_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_get_cmek_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts get settings.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Settings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_billing_accounts_get_settings(
        &self,
        args: &LoggingBillingAccountsGetSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Settings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_get_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_get_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts exclusions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogExclusion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_billing_accounts_exclusions_create(
        &self,
        args: &LoggingBillingAccountsExclusionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogExclusion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_exclusions_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_exclusions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts exclusions delete.
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
    pub fn logging_billing_accounts_exclusions_delete(
        &self,
        args: &LoggingBillingAccountsExclusionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_exclusions_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_exclusions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts exclusions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogExclusion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_billing_accounts_exclusions_get(
        &self,
        args: &LoggingBillingAccountsExclusionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogExclusion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_exclusions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_exclusions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts exclusions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListExclusionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_billing_accounts_exclusions_list(
        &self,
        args: &LoggingBillingAccountsExclusionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListExclusionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_exclusions_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_exclusions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts exclusions patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogExclusion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_billing_accounts_exclusions_patch(
        &self,
        args: &LoggingBillingAccountsExclusionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogExclusion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_exclusions_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_exclusions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts locations get.
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
    pub fn logging_billing_accounts_locations_get(
        &self,
        args: &LoggingBillingAccountsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts locations list.
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
    pub fn logging_billing_accounts_locations_list(
        &self,
        args: &LoggingBillingAccountsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts locations buckets create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogBucket result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_billing_accounts_locations_buckets_create(
        &self,
        args: &LoggingBillingAccountsLocationsBucketsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogBucket, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_locations_buckets_create_builder(
            &self.http_client,
            &args.parent,
            &args.bucketId,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_locations_buckets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts locations buckets create async.
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
    pub fn logging_billing_accounts_locations_buckets_create_async(
        &self,
        args: &LoggingBillingAccountsLocationsBucketsCreateAsyncArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_locations_buckets_create_async_builder(
            &self.http_client,
            &args.parent,
            &args.bucketId,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_locations_buckets_create_async_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts locations buckets delete.
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
    pub fn logging_billing_accounts_locations_buckets_delete(
        &self,
        args: &LoggingBillingAccountsLocationsBucketsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_locations_buckets_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_locations_buckets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts locations buckets get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogBucket result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_billing_accounts_locations_buckets_get(
        &self,
        args: &LoggingBillingAccountsLocationsBucketsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogBucket, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_locations_buckets_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_locations_buckets_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts locations buckets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBucketsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_billing_accounts_locations_buckets_list(
        &self,
        args: &LoggingBillingAccountsLocationsBucketsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBucketsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_locations_buckets_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_locations_buckets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts locations buckets patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogBucket result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_billing_accounts_locations_buckets_patch(
        &self,
        args: &LoggingBillingAccountsLocationsBucketsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogBucket, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_locations_buckets_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_locations_buckets_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts locations buckets undelete.
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
    pub fn logging_billing_accounts_locations_buckets_undelete(
        &self,
        args: &LoggingBillingAccountsLocationsBucketsUndeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_locations_buckets_undelete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_locations_buckets_undelete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts locations buckets update async.
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
    pub fn logging_billing_accounts_locations_buckets_update_async(
        &self,
        args: &LoggingBillingAccountsLocationsBucketsUpdateAsyncArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_locations_buckets_update_async_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_locations_buckets_update_async_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts locations buckets links create.
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
    pub fn logging_billing_accounts_locations_buckets_links_create(
        &self,
        args: &LoggingBillingAccountsLocationsBucketsLinksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_locations_buckets_links_create_builder(
            &self.http_client,
            &args.parent,
            &args.linkId,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_locations_buckets_links_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts locations buckets links delete.
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
    pub fn logging_billing_accounts_locations_buckets_links_delete(
        &self,
        args: &LoggingBillingAccountsLocationsBucketsLinksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_locations_buckets_links_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_locations_buckets_links_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts locations buckets links get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Link result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_billing_accounts_locations_buckets_links_get(
        &self,
        args: &LoggingBillingAccountsLocationsBucketsLinksGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Link, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_locations_buckets_links_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_locations_buckets_links_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts locations buckets links list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLinksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_billing_accounts_locations_buckets_links_list(
        &self,
        args: &LoggingBillingAccountsLocationsBucketsLinksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLinksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_locations_buckets_links_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_locations_buckets_links_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts locations buckets views create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogView result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_billing_accounts_locations_buckets_views_create(
        &self,
        args: &LoggingBillingAccountsLocationsBucketsViewsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogView, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_locations_buckets_views_create_builder(
            &self.http_client,
            &args.parent,
            &args.viewId,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_locations_buckets_views_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts locations buckets views delete.
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
    pub fn logging_billing_accounts_locations_buckets_views_delete(
        &self,
        args: &LoggingBillingAccountsLocationsBucketsViewsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_locations_buckets_views_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_locations_buckets_views_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts locations buckets views get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogView result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_billing_accounts_locations_buckets_views_get(
        &self,
        args: &LoggingBillingAccountsLocationsBucketsViewsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogView, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_locations_buckets_views_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_locations_buckets_views_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts locations buckets views list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListViewsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_billing_accounts_locations_buckets_views_list(
        &self,
        args: &LoggingBillingAccountsLocationsBucketsViewsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListViewsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_locations_buckets_views_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_locations_buckets_views_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts locations buckets views patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogView result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_billing_accounts_locations_buckets_views_patch(
        &self,
        args: &LoggingBillingAccountsLocationsBucketsViewsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogView, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_locations_buckets_views_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_locations_buckets_views_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts locations buckets views logs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLogsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_billing_accounts_locations_buckets_views_logs_list(
        &self,
        args: &LoggingBillingAccountsLocationsBucketsViewsLogsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLogsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_locations_buckets_views_logs_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.resourceNames,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_locations_buckets_views_logs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts locations operations cancel.
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
    pub fn logging_billing_accounts_locations_operations_cancel(
        &self,
        args: &LoggingBillingAccountsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts locations operations get.
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
    pub fn logging_billing_accounts_locations_operations_get(
        &self,
        args: &LoggingBillingAccountsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts locations operations list.
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
    pub fn logging_billing_accounts_locations_operations_list(
        &self,
        args: &LoggingBillingAccountsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts locations recent queries list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListRecentQueriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_billing_accounts_locations_recent_queries_list(
        &self,
        args: &LoggingBillingAccountsLocationsRecentQueriesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListRecentQueriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_locations_recent_queries_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_locations_recent_queries_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts locations saved queries create.
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
    pub fn logging_billing_accounts_locations_saved_queries_create(
        &self,
        args: &LoggingBillingAccountsLocationsSavedQueriesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SavedQuery, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_locations_saved_queries_create_builder(
            &self.http_client,
            &args.parent,
            &args.savedQueryId,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_locations_saved_queries_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts locations saved queries delete.
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
    pub fn logging_billing_accounts_locations_saved_queries_delete(
        &self,
        args: &LoggingBillingAccountsLocationsSavedQueriesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_locations_saved_queries_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_locations_saved_queries_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts locations saved queries get.
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
    pub fn logging_billing_accounts_locations_saved_queries_get(
        &self,
        args: &LoggingBillingAccountsLocationsSavedQueriesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SavedQuery, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_locations_saved_queries_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_locations_saved_queries_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts locations saved queries list.
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
    pub fn logging_billing_accounts_locations_saved_queries_list(
        &self,
        args: &LoggingBillingAccountsLocationsSavedQueriesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSavedQueriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_locations_saved_queries_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_locations_saved_queries_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts locations saved queries patch.
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
    pub fn logging_billing_accounts_locations_saved_queries_patch(
        &self,
        args: &LoggingBillingAccountsLocationsSavedQueriesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SavedQuery, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_locations_saved_queries_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_locations_saved_queries_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts logs delete.
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
    pub fn logging_billing_accounts_logs_delete(
        &self,
        args: &LoggingBillingAccountsLogsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_logs_delete_builder(
            &self.http_client,
            &args.logName,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_logs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts logs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLogsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_billing_accounts_logs_list(
        &self,
        args: &LoggingBillingAccountsLogsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLogsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_logs_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.resourceNames,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_logs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts sinks create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogSink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_billing_accounts_sinks_create(
        &self,
        args: &LoggingBillingAccountsSinksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogSink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_sinks_create_builder(
            &self.http_client,
            &args.parent,
            &args.customWriterIdentity,
            &args.uniqueWriterIdentity,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_sinks_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts sinks delete.
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
    pub fn logging_billing_accounts_sinks_delete(
        &self,
        args: &LoggingBillingAccountsSinksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_sinks_delete_builder(
            &self.http_client,
            &args.sinkName,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_sinks_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts sinks get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogSink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_billing_accounts_sinks_get(
        &self,
        args: &LoggingBillingAccountsSinksGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogSink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_sinks_get_builder(
            &self.http_client,
            &args.sinkName,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_sinks_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts sinks list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSinksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_billing_accounts_sinks_list(
        &self,
        args: &LoggingBillingAccountsSinksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSinksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_sinks_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_sinks_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts sinks patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogSink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_billing_accounts_sinks_patch(
        &self,
        args: &LoggingBillingAccountsSinksPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogSink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_sinks_patch_builder(
            &self.http_client,
            &args.sinkName,
            &args.customWriterIdentity,
            &args.uniqueWriterIdentity,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_sinks_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging billing accounts sinks update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogSink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_billing_accounts_sinks_update(
        &self,
        args: &LoggingBillingAccountsSinksUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogSink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_billing_accounts_sinks_update_builder(
            &self.http_client,
            &args.sinkName,
            &args.customWriterIdentity,
            &args.uniqueWriterIdentity,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_billing_accounts_sinks_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging entries copy.
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
    pub fn logging_entries_copy(
        &self,
        args: &LoggingEntriesCopyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_entries_copy_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_entries_copy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging entries list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLogEntriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_entries_list(
        &self,
        args: &LoggingEntriesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLogEntriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_entries_list_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_entries_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging entries tail.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TailLogEntriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_entries_tail(
        &self,
        args: &LoggingEntriesTailArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TailLogEntriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_entries_tail_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_entries_tail_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging entries write.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WriteLogEntriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_entries_write(
        &self,
        args: &LoggingEntriesWriteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WriteLogEntriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_entries_write_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_entries_write_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging exclusions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogExclusion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_exclusions_create(
        &self,
        args: &LoggingExclusionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogExclusion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_exclusions_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_exclusions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging exclusions delete.
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
    pub fn logging_exclusions_delete(
        &self,
        args: &LoggingExclusionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_exclusions_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_exclusions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging exclusions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogExclusion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_exclusions_get(
        &self,
        args: &LoggingExclusionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogExclusion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_exclusions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_exclusions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging exclusions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListExclusionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_exclusions_list(
        &self,
        args: &LoggingExclusionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListExclusionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_exclusions_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_exclusions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging exclusions patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogExclusion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_exclusions_patch(
        &self,
        args: &LoggingExclusionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogExclusion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_exclusions_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_exclusions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders get cmek settings.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CmekSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_folders_get_cmek_settings(
        &self,
        args: &LoggingFoldersGetCmekSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CmekSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_get_cmek_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_get_cmek_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders get settings.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Settings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_folders_get_settings(
        &self,
        args: &LoggingFoldersGetSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Settings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_get_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_get_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders update settings.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Settings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_folders_update_settings(
        &self,
        args: &LoggingFoldersUpdateSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Settings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_update_settings_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_update_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders exclusions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogExclusion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_folders_exclusions_create(
        &self,
        args: &LoggingFoldersExclusionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogExclusion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_exclusions_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_exclusions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders exclusions delete.
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
    pub fn logging_folders_exclusions_delete(
        &self,
        args: &LoggingFoldersExclusionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_exclusions_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_exclusions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders exclusions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogExclusion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_folders_exclusions_get(
        &self,
        args: &LoggingFoldersExclusionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogExclusion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_exclusions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_exclusions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders exclusions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListExclusionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_folders_exclusions_list(
        &self,
        args: &LoggingFoldersExclusionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListExclusionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_exclusions_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_exclusions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders exclusions patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogExclusion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_folders_exclusions_patch(
        &self,
        args: &LoggingFoldersExclusionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogExclusion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_exclusions_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_exclusions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations get.
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
    pub fn logging_folders_locations_get(
        &self,
        args: &LoggingFoldersLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations list.
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
    pub fn logging_folders_locations_list(
        &self,
        args: &LoggingFoldersLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations buckets create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogBucket result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_folders_locations_buckets_create(
        &self,
        args: &LoggingFoldersLocationsBucketsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogBucket, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_buckets_create_builder(
            &self.http_client,
            &args.parent,
            &args.bucketId,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_buckets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations buckets create async.
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
    pub fn logging_folders_locations_buckets_create_async(
        &self,
        args: &LoggingFoldersLocationsBucketsCreateAsyncArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_buckets_create_async_builder(
            &self.http_client,
            &args.parent,
            &args.bucketId,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_buckets_create_async_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations buckets delete.
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
    pub fn logging_folders_locations_buckets_delete(
        &self,
        args: &LoggingFoldersLocationsBucketsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_buckets_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_buckets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations buckets get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogBucket result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_folders_locations_buckets_get(
        &self,
        args: &LoggingFoldersLocationsBucketsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogBucket, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_buckets_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_buckets_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations buckets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBucketsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_folders_locations_buckets_list(
        &self,
        args: &LoggingFoldersLocationsBucketsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBucketsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_buckets_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_buckets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations buckets patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogBucket result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_folders_locations_buckets_patch(
        &self,
        args: &LoggingFoldersLocationsBucketsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogBucket, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_buckets_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_buckets_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations buckets undelete.
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
    pub fn logging_folders_locations_buckets_undelete(
        &self,
        args: &LoggingFoldersLocationsBucketsUndeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_buckets_undelete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_buckets_undelete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations buckets update async.
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
    pub fn logging_folders_locations_buckets_update_async(
        &self,
        args: &LoggingFoldersLocationsBucketsUpdateAsyncArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_buckets_update_async_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_buckets_update_async_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations buckets links create.
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
    pub fn logging_folders_locations_buckets_links_create(
        &self,
        args: &LoggingFoldersLocationsBucketsLinksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_buckets_links_create_builder(
            &self.http_client,
            &args.parent,
            &args.linkId,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_buckets_links_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations buckets links delete.
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
    pub fn logging_folders_locations_buckets_links_delete(
        &self,
        args: &LoggingFoldersLocationsBucketsLinksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_buckets_links_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_buckets_links_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations buckets links get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Link result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_folders_locations_buckets_links_get(
        &self,
        args: &LoggingFoldersLocationsBucketsLinksGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Link, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_buckets_links_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_buckets_links_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations buckets links list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLinksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_folders_locations_buckets_links_list(
        &self,
        args: &LoggingFoldersLocationsBucketsLinksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLinksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_buckets_links_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_buckets_links_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations buckets views create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogView result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_folders_locations_buckets_views_create(
        &self,
        args: &LoggingFoldersLocationsBucketsViewsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogView, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_buckets_views_create_builder(
            &self.http_client,
            &args.parent,
            &args.viewId,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_buckets_views_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations buckets views delete.
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
    pub fn logging_folders_locations_buckets_views_delete(
        &self,
        args: &LoggingFoldersLocationsBucketsViewsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_buckets_views_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_buckets_views_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations buckets views get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogView result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_folders_locations_buckets_views_get(
        &self,
        args: &LoggingFoldersLocationsBucketsViewsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogView, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_buckets_views_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_buckets_views_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations buckets views get iam policy.
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
    pub fn logging_folders_locations_buckets_views_get_iam_policy(
        &self,
        args: &LoggingFoldersLocationsBucketsViewsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_buckets_views_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_buckets_views_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations buckets views list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListViewsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_folders_locations_buckets_views_list(
        &self,
        args: &LoggingFoldersLocationsBucketsViewsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListViewsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_buckets_views_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_buckets_views_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations buckets views patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogView result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_folders_locations_buckets_views_patch(
        &self,
        args: &LoggingFoldersLocationsBucketsViewsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogView, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_buckets_views_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_buckets_views_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations buckets views set iam policy.
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
    pub fn logging_folders_locations_buckets_views_set_iam_policy(
        &self,
        args: &LoggingFoldersLocationsBucketsViewsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_buckets_views_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_buckets_views_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations buckets views test iam permissions.
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
    pub fn logging_folders_locations_buckets_views_test_iam_permissions(
        &self,
        args: &LoggingFoldersLocationsBucketsViewsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_buckets_views_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_buckets_views_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations buckets views logs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLogsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_folders_locations_buckets_views_logs_list(
        &self,
        args: &LoggingFoldersLocationsBucketsViewsLogsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLogsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_buckets_views_logs_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.resourceNames,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_buckets_views_logs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations log scopes create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogScope result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_folders_locations_log_scopes_create(
        &self,
        args: &LoggingFoldersLocationsLogScopesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogScope, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_log_scopes_create_builder(
            &self.http_client,
            &args.parent,
            &args.logScopeId,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_log_scopes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations log scopes delete.
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
    pub fn logging_folders_locations_log_scopes_delete(
        &self,
        args: &LoggingFoldersLocationsLogScopesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_log_scopes_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_log_scopes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations log scopes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogScope result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_folders_locations_log_scopes_get(
        &self,
        args: &LoggingFoldersLocationsLogScopesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogScope, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_log_scopes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_log_scopes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations log scopes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLogScopesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_folders_locations_log_scopes_list(
        &self,
        args: &LoggingFoldersLocationsLogScopesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLogScopesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_log_scopes_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_log_scopes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations log scopes patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogScope result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_folders_locations_log_scopes_patch(
        &self,
        args: &LoggingFoldersLocationsLogScopesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogScope, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_log_scopes_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_log_scopes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations operations cancel.
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
    pub fn logging_folders_locations_operations_cancel(
        &self,
        args: &LoggingFoldersLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations operations get.
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
    pub fn logging_folders_locations_operations_get(
        &self,
        args: &LoggingFoldersLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations operations list.
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
    pub fn logging_folders_locations_operations_list(
        &self,
        args: &LoggingFoldersLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations recent queries list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListRecentQueriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_folders_locations_recent_queries_list(
        &self,
        args: &LoggingFoldersLocationsRecentQueriesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListRecentQueriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_recent_queries_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_recent_queries_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations saved queries create.
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
    pub fn logging_folders_locations_saved_queries_create(
        &self,
        args: &LoggingFoldersLocationsSavedQueriesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SavedQuery, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_saved_queries_create_builder(
            &self.http_client,
            &args.parent,
            &args.savedQueryId,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_saved_queries_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations saved queries delete.
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
    pub fn logging_folders_locations_saved_queries_delete(
        &self,
        args: &LoggingFoldersLocationsSavedQueriesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_saved_queries_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_saved_queries_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations saved queries get.
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
    pub fn logging_folders_locations_saved_queries_get(
        &self,
        args: &LoggingFoldersLocationsSavedQueriesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SavedQuery, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_saved_queries_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_saved_queries_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations saved queries list.
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
    pub fn logging_folders_locations_saved_queries_list(
        &self,
        args: &LoggingFoldersLocationsSavedQueriesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSavedQueriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_saved_queries_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_saved_queries_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders locations saved queries patch.
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
    pub fn logging_folders_locations_saved_queries_patch(
        &self,
        args: &LoggingFoldersLocationsSavedQueriesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SavedQuery, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_locations_saved_queries_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_locations_saved_queries_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders logs delete.
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
    pub fn logging_folders_logs_delete(
        &self,
        args: &LoggingFoldersLogsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_logs_delete_builder(
            &self.http_client,
            &args.logName,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_logs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders logs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLogsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_folders_logs_list(
        &self,
        args: &LoggingFoldersLogsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLogsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_logs_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.resourceNames,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_logs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders sinks create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogSink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_folders_sinks_create(
        &self,
        args: &LoggingFoldersSinksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogSink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_sinks_create_builder(
            &self.http_client,
            &args.parent,
            &args.customWriterIdentity,
            &args.uniqueWriterIdentity,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_sinks_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders sinks delete.
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
    pub fn logging_folders_sinks_delete(
        &self,
        args: &LoggingFoldersSinksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_sinks_delete_builder(
            &self.http_client,
            &args.sinkName,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_sinks_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders sinks get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogSink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_folders_sinks_get(
        &self,
        args: &LoggingFoldersSinksGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogSink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_sinks_get_builder(
            &self.http_client,
            &args.sinkName,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_sinks_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders sinks list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSinksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_folders_sinks_list(
        &self,
        args: &LoggingFoldersSinksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSinksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_sinks_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_sinks_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders sinks patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogSink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_folders_sinks_patch(
        &self,
        args: &LoggingFoldersSinksPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogSink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_sinks_patch_builder(
            &self.http_client,
            &args.sinkName,
            &args.customWriterIdentity,
            &args.uniqueWriterIdentity,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_sinks_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging folders sinks update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogSink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_folders_sinks_update(
        &self,
        args: &LoggingFoldersSinksUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogSink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_folders_sinks_update_builder(
            &self.http_client,
            &args.sinkName,
            &args.customWriterIdentity,
            &args.uniqueWriterIdentity,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_folders_sinks_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging locations get.
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
    pub fn logging_locations_get(
        &self,
        args: &LoggingLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging locations list.
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
    pub fn logging_locations_list(
        &self,
        args: &LoggingLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging locations buckets create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogBucket result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_locations_buckets_create(
        &self,
        args: &LoggingLocationsBucketsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogBucket, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_locations_buckets_create_builder(
            &self.http_client,
            &args.parent,
            &args.bucketId,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_locations_buckets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging locations buckets create async.
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
    pub fn logging_locations_buckets_create_async(
        &self,
        args: &LoggingLocationsBucketsCreateAsyncArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_locations_buckets_create_async_builder(
            &self.http_client,
            &args.parent,
            &args.bucketId,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_locations_buckets_create_async_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging locations buckets delete.
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
    pub fn logging_locations_buckets_delete(
        &self,
        args: &LoggingLocationsBucketsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_locations_buckets_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_locations_buckets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging locations buckets get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogBucket result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_locations_buckets_get(
        &self,
        args: &LoggingLocationsBucketsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogBucket, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_locations_buckets_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_locations_buckets_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging locations buckets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBucketsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_locations_buckets_list(
        &self,
        args: &LoggingLocationsBucketsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBucketsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_locations_buckets_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_locations_buckets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging locations buckets patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogBucket result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_locations_buckets_patch(
        &self,
        args: &LoggingLocationsBucketsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogBucket, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_locations_buckets_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_locations_buckets_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging locations buckets undelete.
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
    pub fn logging_locations_buckets_undelete(
        &self,
        args: &LoggingLocationsBucketsUndeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_locations_buckets_undelete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_locations_buckets_undelete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging locations buckets update async.
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
    pub fn logging_locations_buckets_update_async(
        &self,
        args: &LoggingLocationsBucketsUpdateAsyncArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_locations_buckets_update_async_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_locations_buckets_update_async_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging locations buckets links create.
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
    pub fn logging_locations_buckets_links_create(
        &self,
        args: &LoggingLocationsBucketsLinksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_locations_buckets_links_create_builder(
            &self.http_client,
            &args.parent,
            &args.linkId,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_locations_buckets_links_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging locations buckets links delete.
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
    pub fn logging_locations_buckets_links_delete(
        &self,
        args: &LoggingLocationsBucketsLinksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_locations_buckets_links_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_locations_buckets_links_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging locations buckets links get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Link result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_locations_buckets_links_get(
        &self,
        args: &LoggingLocationsBucketsLinksGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Link, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_locations_buckets_links_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_locations_buckets_links_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging locations buckets links list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLinksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_locations_buckets_links_list(
        &self,
        args: &LoggingLocationsBucketsLinksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLinksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_locations_buckets_links_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_locations_buckets_links_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging locations buckets views create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogView result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_locations_buckets_views_create(
        &self,
        args: &LoggingLocationsBucketsViewsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogView, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_locations_buckets_views_create_builder(
            &self.http_client,
            &args.parent,
            &args.viewId,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_locations_buckets_views_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging locations buckets views delete.
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
    pub fn logging_locations_buckets_views_delete(
        &self,
        args: &LoggingLocationsBucketsViewsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_locations_buckets_views_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_locations_buckets_views_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging locations buckets views get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogView result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_locations_buckets_views_get(
        &self,
        args: &LoggingLocationsBucketsViewsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogView, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_locations_buckets_views_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_locations_buckets_views_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging locations buckets views get iam policy.
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
    pub fn logging_locations_buckets_views_get_iam_policy(
        &self,
        args: &LoggingLocationsBucketsViewsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_locations_buckets_views_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_locations_buckets_views_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging locations buckets views list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListViewsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_locations_buckets_views_list(
        &self,
        args: &LoggingLocationsBucketsViewsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListViewsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_locations_buckets_views_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_locations_buckets_views_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging locations buckets views patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogView result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_locations_buckets_views_patch(
        &self,
        args: &LoggingLocationsBucketsViewsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogView, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_locations_buckets_views_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_locations_buckets_views_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging locations buckets views set iam policy.
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
    pub fn logging_locations_buckets_views_set_iam_policy(
        &self,
        args: &LoggingLocationsBucketsViewsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_locations_buckets_views_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_locations_buckets_views_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging locations buckets views test iam permissions.
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
    pub fn logging_locations_buckets_views_test_iam_permissions(
        &self,
        args: &LoggingLocationsBucketsViewsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_locations_buckets_views_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_locations_buckets_views_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging locations operations cancel.
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
    pub fn logging_locations_operations_cancel(
        &self,
        args: &LoggingLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging locations operations get.
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
    pub fn logging_locations_operations_get(
        &self,
        args: &LoggingLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging locations operations list.
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
    pub fn logging_locations_operations_list(
        &self,
        args: &LoggingLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging logs delete.
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
    pub fn logging_logs_delete(
        &self,
        args: &LoggingLogsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_logs_delete_builder(
            &self.http_client,
            &args.logName,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_logs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging logs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLogsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_logs_list(
        &self,
        args: &LoggingLogsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLogsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_logs_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.resourceNames,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_logs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging monitored resource descriptors list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListMonitoredResourceDescriptorsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_monitored_resource_descriptors_list(
        &self,
        args: &LoggingMonitoredResourceDescriptorsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListMonitoredResourceDescriptorsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_monitored_resource_descriptors_list_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_monitored_resource_descriptors_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations get cmek settings.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CmekSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_organizations_get_cmek_settings(
        &self,
        args: &LoggingOrganizationsGetCmekSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CmekSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_get_cmek_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_get_cmek_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations get settings.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Settings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_organizations_get_settings(
        &self,
        args: &LoggingOrganizationsGetSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Settings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_get_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_get_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations update cmek settings.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CmekSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_organizations_update_cmek_settings(
        &self,
        args: &LoggingOrganizationsUpdateCmekSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CmekSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_update_cmek_settings_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_update_cmek_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations update settings.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Settings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_organizations_update_settings(
        &self,
        args: &LoggingOrganizationsUpdateSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Settings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_update_settings_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_update_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations exclusions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogExclusion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_organizations_exclusions_create(
        &self,
        args: &LoggingOrganizationsExclusionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogExclusion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_exclusions_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_exclusions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations exclusions delete.
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
    pub fn logging_organizations_exclusions_delete(
        &self,
        args: &LoggingOrganizationsExclusionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_exclusions_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_exclusions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations exclusions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogExclusion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_organizations_exclusions_get(
        &self,
        args: &LoggingOrganizationsExclusionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogExclusion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_exclusions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_exclusions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations exclusions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListExclusionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_organizations_exclusions_list(
        &self,
        args: &LoggingOrganizationsExclusionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListExclusionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_exclusions_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_exclusions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations exclusions patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogExclusion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_organizations_exclusions_patch(
        &self,
        args: &LoggingOrganizationsExclusionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogExclusion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_exclusions_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_exclusions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations get.
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
    pub fn logging_organizations_locations_get(
        &self,
        args: &LoggingOrganizationsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations list.
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
    pub fn logging_organizations_locations_list(
        &self,
        args: &LoggingOrganizationsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations buckets create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogBucket result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_organizations_locations_buckets_create(
        &self,
        args: &LoggingOrganizationsLocationsBucketsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogBucket, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_buckets_create_builder(
            &self.http_client,
            &args.parent,
            &args.bucketId,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_buckets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations buckets create async.
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
    pub fn logging_organizations_locations_buckets_create_async(
        &self,
        args: &LoggingOrganizationsLocationsBucketsCreateAsyncArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_buckets_create_async_builder(
            &self.http_client,
            &args.parent,
            &args.bucketId,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_buckets_create_async_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations buckets delete.
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
    pub fn logging_organizations_locations_buckets_delete(
        &self,
        args: &LoggingOrganizationsLocationsBucketsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_buckets_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_buckets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations buckets get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogBucket result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_organizations_locations_buckets_get(
        &self,
        args: &LoggingOrganizationsLocationsBucketsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogBucket, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_buckets_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_buckets_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations buckets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBucketsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_organizations_locations_buckets_list(
        &self,
        args: &LoggingOrganizationsLocationsBucketsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBucketsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_buckets_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_buckets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations buckets patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogBucket result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_organizations_locations_buckets_patch(
        &self,
        args: &LoggingOrganizationsLocationsBucketsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogBucket, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_buckets_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_buckets_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations buckets undelete.
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
    pub fn logging_organizations_locations_buckets_undelete(
        &self,
        args: &LoggingOrganizationsLocationsBucketsUndeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_buckets_undelete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_buckets_undelete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations buckets update async.
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
    pub fn logging_organizations_locations_buckets_update_async(
        &self,
        args: &LoggingOrganizationsLocationsBucketsUpdateAsyncArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_buckets_update_async_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_buckets_update_async_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations buckets links create.
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
    pub fn logging_organizations_locations_buckets_links_create(
        &self,
        args: &LoggingOrganizationsLocationsBucketsLinksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_buckets_links_create_builder(
            &self.http_client,
            &args.parent,
            &args.linkId,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_buckets_links_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations buckets links delete.
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
    pub fn logging_organizations_locations_buckets_links_delete(
        &self,
        args: &LoggingOrganizationsLocationsBucketsLinksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_buckets_links_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_buckets_links_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations buckets links get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Link result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_organizations_locations_buckets_links_get(
        &self,
        args: &LoggingOrganizationsLocationsBucketsLinksGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Link, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_buckets_links_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_buckets_links_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations buckets links list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLinksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_organizations_locations_buckets_links_list(
        &self,
        args: &LoggingOrganizationsLocationsBucketsLinksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLinksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_buckets_links_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_buckets_links_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations buckets views create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogView result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_organizations_locations_buckets_views_create(
        &self,
        args: &LoggingOrganizationsLocationsBucketsViewsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogView, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_buckets_views_create_builder(
            &self.http_client,
            &args.parent,
            &args.viewId,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_buckets_views_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations buckets views delete.
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
    pub fn logging_organizations_locations_buckets_views_delete(
        &self,
        args: &LoggingOrganizationsLocationsBucketsViewsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_buckets_views_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_buckets_views_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations buckets views get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogView result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_organizations_locations_buckets_views_get(
        &self,
        args: &LoggingOrganizationsLocationsBucketsViewsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogView, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_buckets_views_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_buckets_views_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations buckets views get iam policy.
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
    pub fn logging_organizations_locations_buckets_views_get_iam_policy(
        &self,
        args: &LoggingOrganizationsLocationsBucketsViewsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_buckets_views_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_buckets_views_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations buckets views list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListViewsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_organizations_locations_buckets_views_list(
        &self,
        args: &LoggingOrganizationsLocationsBucketsViewsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListViewsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_buckets_views_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_buckets_views_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations buckets views patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogView result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_organizations_locations_buckets_views_patch(
        &self,
        args: &LoggingOrganizationsLocationsBucketsViewsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogView, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_buckets_views_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_buckets_views_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations buckets views set iam policy.
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
    pub fn logging_organizations_locations_buckets_views_set_iam_policy(
        &self,
        args: &LoggingOrganizationsLocationsBucketsViewsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_buckets_views_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_buckets_views_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations buckets views test iam permissions.
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
    pub fn logging_organizations_locations_buckets_views_test_iam_permissions(
        &self,
        args: &LoggingOrganizationsLocationsBucketsViewsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_buckets_views_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_buckets_views_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations buckets views logs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLogsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_organizations_locations_buckets_views_logs_list(
        &self,
        args: &LoggingOrganizationsLocationsBucketsViewsLogsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLogsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_buckets_views_logs_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.resourceNames,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_buckets_views_logs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations log scopes create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogScope result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_organizations_locations_log_scopes_create(
        &self,
        args: &LoggingOrganizationsLocationsLogScopesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogScope, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_log_scopes_create_builder(
            &self.http_client,
            &args.parent,
            &args.logScopeId,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_log_scopes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations log scopes delete.
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
    pub fn logging_organizations_locations_log_scopes_delete(
        &self,
        args: &LoggingOrganizationsLocationsLogScopesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_log_scopes_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_log_scopes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations log scopes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogScope result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_organizations_locations_log_scopes_get(
        &self,
        args: &LoggingOrganizationsLocationsLogScopesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogScope, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_log_scopes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_log_scopes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations log scopes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLogScopesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_organizations_locations_log_scopes_list(
        &self,
        args: &LoggingOrganizationsLocationsLogScopesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLogScopesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_log_scopes_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_log_scopes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations log scopes patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogScope result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_organizations_locations_log_scopes_patch(
        &self,
        args: &LoggingOrganizationsLocationsLogScopesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogScope, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_log_scopes_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_log_scopes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations operations cancel.
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
    pub fn logging_organizations_locations_operations_cancel(
        &self,
        args: &LoggingOrganizationsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations operations get.
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
    pub fn logging_organizations_locations_operations_get(
        &self,
        args: &LoggingOrganizationsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations operations list.
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
    pub fn logging_organizations_locations_operations_list(
        &self,
        args: &LoggingOrganizationsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations recent queries list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListRecentQueriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_organizations_locations_recent_queries_list(
        &self,
        args: &LoggingOrganizationsLocationsRecentQueriesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListRecentQueriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_recent_queries_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_recent_queries_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations saved queries create.
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
    pub fn logging_organizations_locations_saved_queries_create(
        &self,
        args: &LoggingOrganizationsLocationsSavedQueriesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SavedQuery, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_saved_queries_create_builder(
            &self.http_client,
            &args.parent,
            &args.savedQueryId,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_saved_queries_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations saved queries delete.
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
    pub fn logging_organizations_locations_saved_queries_delete(
        &self,
        args: &LoggingOrganizationsLocationsSavedQueriesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_saved_queries_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_saved_queries_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations saved queries get.
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
    pub fn logging_organizations_locations_saved_queries_get(
        &self,
        args: &LoggingOrganizationsLocationsSavedQueriesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SavedQuery, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_saved_queries_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_saved_queries_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations saved queries list.
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
    pub fn logging_organizations_locations_saved_queries_list(
        &self,
        args: &LoggingOrganizationsLocationsSavedQueriesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSavedQueriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_saved_queries_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_saved_queries_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations locations saved queries patch.
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
    pub fn logging_organizations_locations_saved_queries_patch(
        &self,
        args: &LoggingOrganizationsLocationsSavedQueriesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SavedQuery, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_locations_saved_queries_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_locations_saved_queries_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations logs delete.
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
    pub fn logging_organizations_logs_delete(
        &self,
        args: &LoggingOrganizationsLogsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_logs_delete_builder(
            &self.http_client,
            &args.logName,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_logs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations logs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLogsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_organizations_logs_list(
        &self,
        args: &LoggingOrganizationsLogsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLogsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_logs_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.resourceNames,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_logs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations sinks create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogSink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_organizations_sinks_create(
        &self,
        args: &LoggingOrganizationsSinksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogSink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_sinks_create_builder(
            &self.http_client,
            &args.parent,
            &args.customWriterIdentity,
            &args.uniqueWriterIdentity,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_sinks_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations sinks delete.
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
    pub fn logging_organizations_sinks_delete(
        &self,
        args: &LoggingOrganizationsSinksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_sinks_delete_builder(
            &self.http_client,
            &args.sinkName,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_sinks_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations sinks get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogSink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_organizations_sinks_get(
        &self,
        args: &LoggingOrganizationsSinksGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogSink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_sinks_get_builder(
            &self.http_client,
            &args.sinkName,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_sinks_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations sinks list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSinksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_organizations_sinks_list(
        &self,
        args: &LoggingOrganizationsSinksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSinksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_sinks_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_sinks_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations sinks patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogSink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_organizations_sinks_patch(
        &self,
        args: &LoggingOrganizationsSinksPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogSink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_sinks_patch_builder(
            &self.http_client,
            &args.sinkName,
            &args.customWriterIdentity,
            &args.uniqueWriterIdentity,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_sinks_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging organizations sinks update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogSink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_organizations_sinks_update(
        &self,
        args: &LoggingOrganizationsSinksUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogSink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_organizations_sinks_update_builder(
            &self.http_client,
            &args.sinkName,
            &args.customWriterIdentity,
            &args.uniqueWriterIdentity,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_organizations_sinks_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects get cmek settings.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CmekSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_projects_get_cmek_settings(
        &self,
        args: &LoggingProjectsGetCmekSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CmekSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_get_cmek_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_get_cmek_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects get settings.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Settings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_projects_get_settings(
        &self,
        args: &LoggingProjectsGetSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Settings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_get_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_get_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects exclusions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogExclusion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_projects_exclusions_create(
        &self,
        args: &LoggingProjectsExclusionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogExclusion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_exclusions_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_exclusions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects exclusions delete.
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
    pub fn logging_projects_exclusions_delete(
        &self,
        args: &LoggingProjectsExclusionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_exclusions_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_exclusions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects exclusions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogExclusion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_projects_exclusions_get(
        &self,
        args: &LoggingProjectsExclusionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogExclusion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_exclusions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_exclusions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects exclusions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListExclusionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_projects_exclusions_list(
        &self,
        args: &LoggingProjectsExclusionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListExclusionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_exclusions_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_exclusions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects exclusions patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogExclusion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_projects_exclusions_patch(
        &self,
        args: &LoggingProjectsExclusionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogExclusion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_exclusions_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_exclusions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations get.
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
    pub fn logging_projects_locations_get(
        &self,
        args: &LoggingProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations list.
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
    pub fn logging_projects_locations_list(
        &self,
        args: &LoggingProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations buckets create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogBucket result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_projects_locations_buckets_create(
        &self,
        args: &LoggingProjectsLocationsBucketsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogBucket, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_buckets_create_builder(
            &self.http_client,
            &args.parent,
            &args.bucketId,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_buckets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations buckets create async.
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
    pub fn logging_projects_locations_buckets_create_async(
        &self,
        args: &LoggingProjectsLocationsBucketsCreateAsyncArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_buckets_create_async_builder(
            &self.http_client,
            &args.parent,
            &args.bucketId,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_buckets_create_async_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations buckets delete.
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
    pub fn logging_projects_locations_buckets_delete(
        &self,
        args: &LoggingProjectsLocationsBucketsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_buckets_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_buckets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations buckets get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogBucket result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_projects_locations_buckets_get(
        &self,
        args: &LoggingProjectsLocationsBucketsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogBucket, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_buckets_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_buckets_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations buckets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBucketsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_projects_locations_buckets_list(
        &self,
        args: &LoggingProjectsLocationsBucketsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBucketsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_buckets_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_buckets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations buckets patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogBucket result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_projects_locations_buckets_patch(
        &self,
        args: &LoggingProjectsLocationsBucketsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogBucket, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_buckets_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_buckets_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations buckets undelete.
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
    pub fn logging_projects_locations_buckets_undelete(
        &self,
        args: &LoggingProjectsLocationsBucketsUndeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_buckets_undelete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_buckets_undelete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations buckets update async.
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
    pub fn logging_projects_locations_buckets_update_async(
        &self,
        args: &LoggingProjectsLocationsBucketsUpdateAsyncArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_buckets_update_async_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_buckets_update_async_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations buckets links create.
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
    pub fn logging_projects_locations_buckets_links_create(
        &self,
        args: &LoggingProjectsLocationsBucketsLinksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_buckets_links_create_builder(
            &self.http_client,
            &args.parent,
            &args.linkId,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_buckets_links_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations buckets links delete.
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
    pub fn logging_projects_locations_buckets_links_delete(
        &self,
        args: &LoggingProjectsLocationsBucketsLinksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_buckets_links_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_buckets_links_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations buckets links get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Link result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_projects_locations_buckets_links_get(
        &self,
        args: &LoggingProjectsLocationsBucketsLinksGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Link, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_buckets_links_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_buckets_links_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations buckets links list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLinksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_projects_locations_buckets_links_list(
        &self,
        args: &LoggingProjectsLocationsBucketsLinksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLinksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_buckets_links_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_buckets_links_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations buckets views create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogView result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_projects_locations_buckets_views_create(
        &self,
        args: &LoggingProjectsLocationsBucketsViewsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogView, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_buckets_views_create_builder(
            &self.http_client,
            &args.parent,
            &args.viewId,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_buckets_views_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations buckets views delete.
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
    pub fn logging_projects_locations_buckets_views_delete(
        &self,
        args: &LoggingProjectsLocationsBucketsViewsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_buckets_views_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_buckets_views_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations buckets views get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogView result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_projects_locations_buckets_views_get(
        &self,
        args: &LoggingProjectsLocationsBucketsViewsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogView, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_buckets_views_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_buckets_views_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations buckets views get iam policy.
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
    pub fn logging_projects_locations_buckets_views_get_iam_policy(
        &self,
        args: &LoggingProjectsLocationsBucketsViewsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_buckets_views_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_buckets_views_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations buckets views list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListViewsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_projects_locations_buckets_views_list(
        &self,
        args: &LoggingProjectsLocationsBucketsViewsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListViewsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_buckets_views_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_buckets_views_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations buckets views patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogView result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_projects_locations_buckets_views_patch(
        &self,
        args: &LoggingProjectsLocationsBucketsViewsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogView, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_buckets_views_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_buckets_views_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations buckets views set iam policy.
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
    pub fn logging_projects_locations_buckets_views_set_iam_policy(
        &self,
        args: &LoggingProjectsLocationsBucketsViewsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_buckets_views_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_buckets_views_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations buckets views test iam permissions.
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
    pub fn logging_projects_locations_buckets_views_test_iam_permissions(
        &self,
        args: &LoggingProjectsLocationsBucketsViewsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_buckets_views_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_buckets_views_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations buckets views logs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLogsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_projects_locations_buckets_views_logs_list(
        &self,
        args: &LoggingProjectsLocationsBucketsViewsLogsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLogsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_buckets_views_logs_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.resourceNames,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_buckets_views_logs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations log scopes create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogScope result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_projects_locations_log_scopes_create(
        &self,
        args: &LoggingProjectsLocationsLogScopesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogScope, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_log_scopes_create_builder(
            &self.http_client,
            &args.parent,
            &args.logScopeId,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_log_scopes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations log scopes delete.
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
    pub fn logging_projects_locations_log_scopes_delete(
        &self,
        args: &LoggingProjectsLocationsLogScopesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_log_scopes_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_log_scopes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations log scopes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogScope result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_projects_locations_log_scopes_get(
        &self,
        args: &LoggingProjectsLocationsLogScopesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogScope, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_log_scopes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_log_scopes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations log scopes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLogScopesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_projects_locations_log_scopes_list(
        &self,
        args: &LoggingProjectsLocationsLogScopesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLogScopesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_log_scopes_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_log_scopes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations log scopes patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogScope result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_projects_locations_log_scopes_patch(
        &self,
        args: &LoggingProjectsLocationsLogScopesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogScope, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_log_scopes_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_log_scopes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations operations cancel.
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
    pub fn logging_projects_locations_operations_cancel(
        &self,
        args: &LoggingProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations operations get.
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
    pub fn logging_projects_locations_operations_get(
        &self,
        args: &LoggingProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations operations list.
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
    pub fn logging_projects_locations_operations_list(
        &self,
        args: &LoggingProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations recent queries list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListRecentQueriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_projects_locations_recent_queries_list(
        &self,
        args: &LoggingProjectsLocationsRecentQueriesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListRecentQueriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_recent_queries_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_recent_queries_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations saved queries create.
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
    pub fn logging_projects_locations_saved_queries_create(
        &self,
        args: &LoggingProjectsLocationsSavedQueriesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SavedQuery, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_saved_queries_create_builder(
            &self.http_client,
            &args.parent,
            &args.savedQueryId,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_saved_queries_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations saved queries delete.
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
    pub fn logging_projects_locations_saved_queries_delete(
        &self,
        args: &LoggingProjectsLocationsSavedQueriesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_saved_queries_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_saved_queries_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations saved queries get.
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
    pub fn logging_projects_locations_saved_queries_get(
        &self,
        args: &LoggingProjectsLocationsSavedQueriesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SavedQuery, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_saved_queries_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_saved_queries_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations saved queries list.
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
    pub fn logging_projects_locations_saved_queries_list(
        &self,
        args: &LoggingProjectsLocationsSavedQueriesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSavedQueriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_saved_queries_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_saved_queries_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects locations saved queries patch.
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
    pub fn logging_projects_locations_saved_queries_patch(
        &self,
        args: &LoggingProjectsLocationsSavedQueriesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SavedQuery, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_locations_saved_queries_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_locations_saved_queries_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects logs delete.
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
    pub fn logging_projects_logs_delete(
        &self,
        args: &LoggingProjectsLogsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_logs_delete_builder(
            &self.http_client,
            &args.logName,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_logs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects logs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLogsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_projects_logs_list(
        &self,
        args: &LoggingProjectsLogsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLogsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_logs_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.resourceNames,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_logs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects metrics create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogMetric result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_projects_metrics_create(
        &self,
        args: &LoggingProjectsMetricsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogMetric, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_metrics_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_metrics_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects metrics delete.
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
    pub fn logging_projects_metrics_delete(
        &self,
        args: &LoggingProjectsMetricsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_metrics_delete_builder(
            &self.http_client,
            &args.metricName,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_metrics_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects metrics get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogMetric result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_projects_metrics_get(
        &self,
        args: &LoggingProjectsMetricsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogMetric, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_metrics_get_builder(
            &self.http_client,
            &args.metricName,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_metrics_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects metrics list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLogMetricsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_projects_metrics_list(
        &self,
        args: &LoggingProjectsMetricsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLogMetricsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_metrics_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_metrics_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects metrics update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogMetric result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_projects_metrics_update(
        &self,
        args: &LoggingProjectsMetricsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogMetric, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_metrics_update_builder(
            &self.http_client,
            &args.metricName,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_metrics_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects sinks create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogSink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_projects_sinks_create(
        &self,
        args: &LoggingProjectsSinksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogSink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_sinks_create_builder(
            &self.http_client,
            &args.parent,
            &args.customWriterIdentity,
            &args.uniqueWriterIdentity,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_sinks_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects sinks delete.
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
    pub fn logging_projects_sinks_delete(
        &self,
        args: &LoggingProjectsSinksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_sinks_delete_builder(
            &self.http_client,
            &args.sinkName,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_sinks_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects sinks get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogSink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_projects_sinks_get(
        &self,
        args: &LoggingProjectsSinksGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogSink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_sinks_get_builder(
            &self.http_client,
            &args.sinkName,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_sinks_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects sinks list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSinksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_projects_sinks_list(
        &self,
        args: &LoggingProjectsSinksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSinksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_sinks_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_sinks_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects sinks patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogSink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_projects_sinks_patch(
        &self,
        args: &LoggingProjectsSinksPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogSink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_sinks_patch_builder(
            &self.http_client,
            &args.sinkName,
            &args.customWriterIdentity,
            &args.uniqueWriterIdentity,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_sinks_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging projects sinks update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogSink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_projects_sinks_update(
        &self,
        args: &LoggingProjectsSinksUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogSink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_projects_sinks_update_builder(
            &self.http_client,
            &args.sinkName,
            &args.customWriterIdentity,
            &args.uniqueWriterIdentity,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_projects_sinks_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging sinks create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogSink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_sinks_create(
        &self,
        args: &LoggingSinksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogSink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_sinks_create_builder(
            &self.http_client,
            &args.parent,
            &args.customWriterIdentity,
            &args.uniqueWriterIdentity,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_sinks_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging sinks delete.
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
    pub fn logging_sinks_delete(
        &self,
        args: &LoggingSinksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_sinks_delete_builder(
            &self.http_client,
            &args.sinkName,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_sinks_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging sinks get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogSink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_sinks_get(
        &self,
        args: &LoggingSinksGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogSink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_sinks_get_builder(
            &self.http_client,
            &args.sinkName,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_sinks_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging sinks list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSinksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_sinks_list(
        &self,
        args: &LoggingSinksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSinksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_sinks_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_sinks_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging sinks update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogSink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_sinks_update(
        &self,
        args: &LoggingSinksUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogSink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_sinks_update_builder(
            &self.http_client,
            &args.sinkName,
            &args.customWriterIdentity,
            &args.uniqueWriterIdentity,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_sinks_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging get cmek settings.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CmekSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_get_cmek_settings(
        &self,
        args: &LoggingGetCmekSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CmekSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_get_cmek_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_get_cmek_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging get settings.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Settings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn logging_get_settings(
        &self,
        args: &LoggingGetSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Settings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_get_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_get_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging update cmek settings.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CmekSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_update_cmek_settings(
        &self,
        args: &LoggingUpdateCmekSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CmekSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_update_cmek_settings_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_update_cmek_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Logging update settings.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Settings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn logging_update_settings(
        &self,
        args: &LoggingUpdateSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Settings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = logging_update_settings_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = logging_update_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
