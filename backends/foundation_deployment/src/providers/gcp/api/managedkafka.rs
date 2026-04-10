//! ManagedkafkaProvider - State-aware managedkafka API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       managedkafka API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::managedkafka::{
    managedkafka_projects_locations_get_builder, managedkafka_projects_locations_get_task,
    managedkafka_projects_locations_list_builder, managedkafka_projects_locations_list_task,
    managedkafka_projects_locations_clusters_create_builder, managedkafka_projects_locations_clusters_create_task,
    managedkafka_projects_locations_clusters_delete_builder, managedkafka_projects_locations_clusters_delete_task,
    managedkafka_projects_locations_clusters_get_builder, managedkafka_projects_locations_clusters_get_task,
    managedkafka_projects_locations_clusters_list_builder, managedkafka_projects_locations_clusters_list_task,
    managedkafka_projects_locations_clusters_patch_builder, managedkafka_projects_locations_clusters_patch_task,
    managedkafka_projects_locations_clusters_acls_add_acl_entry_builder, managedkafka_projects_locations_clusters_acls_add_acl_entry_task,
    managedkafka_projects_locations_clusters_acls_create_builder, managedkafka_projects_locations_clusters_acls_create_task,
    managedkafka_projects_locations_clusters_acls_delete_builder, managedkafka_projects_locations_clusters_acls_delete_task,
    managedkafka_projects_locations_clusters_acls_get_builder, managedkafka_projects_locations_clusters_acls_get_task,
    managedkafka_projects_locations_clusters_acls_list_builder, managedkafka_projects_locations_clusters_acls_list_task,
    managedkafka_projects_locations_clusters_acls_patch_builder, managedkafka_projects_locations_clusters_acls_patch_task,
    managedkafka_projects_locations_clusters_acls_remove_acl_entry_builder, managedkafka_projects_locations_clusters_acls_remove_acl_entry_task,
    managedkafka_projects_locations_clusters_consumer_groups_delete_builder, managedkafka_projects_locations_clusters_consumer_groups_delete_task,
    managedkafka_projects_locations_clusters_consumer_groups_get_builder, managedkafka_projects_locations_clusters_consumer_groups_get_task,
    managedkafka_projects_locations_clusters_consumer_groups_list_builder, managedkafka_projects_locations_clusters_consumer_groups_list_task,
    managedkafka_projects_locations_clusters_consumer_groups_patch_builder, managedkafka_projects_locations_clusters_consumer_groups_patch_task,
    managedkafka_projects_locations_clusters_topics_create_builder, managedkafka_projects_locations_clusters_topics_create_task,
    managedkafka_projects_locations_clusters_topics_delete_builder, managedkafka_projects_locations_clusters_topics_delete_task,
    managedkafka_projects_locations_clusters_topics_get_builder, managedkafka_projects_locations_clusters_topics_get_task,
    managedkafka_projects_locations_clusters_topics_list_builder, managedkafka_projects_locations_clusters_topics_list_task,
    managedkafka_projects_locations_clusters_topics_patch_builder, managedkafka_projects_locations_clusters_topics_patch_task,
    managedkafka_projects_locations_connect_clusters_create_builder, managedkafka_projects_locations_connect_clusters_create_task,
    managedkafka_projects_locations_connect_clusters_delete_builder, managedkafka_projects_locations_connect_clusters_delete_task,
    managedkafka_projects_locations_connect_clusters_get_builder, managedkafka_projects_locations_connect_clusters_get_task,
    managedkafka_projects_locations_connect_clusters_list_builder, managedkafka_projects_locations_connect_clusters_list_task,
    managedkafka_projects_locations_connect_clusters_patch_builder, managedkafka_projects_locations_connect_clusters_patch_task,
    managedkafka_projects_locations_connect_clusters_connectors_create_builder, managedkafka_projects_locations_connect_clusters_connectors_create_task,
    managedkafka_projects_locations_connect_clusters_connectors_delete_builder, managedkafka_projects_locations_connect_clusters_connectors_delete_task,
    managedkafka_projects_locations_connect_clusters_connectors_get_builder, managedkafka_projects_locations_connect_clusters_connectors_get_task,
    managedkafka_projects_locations_connect_clusters_connectors_list_builder, managedkafka_projects_locations_connect_clusters_connectors_list_task,
    managedkafka_projects_locations_connect_clusters_connectors_patch_builder, managedkafka_projects_locations_connect_clusters_connectors_patch_task,
    managedkafka_projects_locations_connect_clusters_connectors_pause_builder, managedkafka_projects_locations_connect_clusters_connectors_pause_task,
    managedkafka_projects_locations_connect_clusters_connectors_restart_builder, managedkafka_projects_locations_connect_clusters_connectors_restart_task,
    managedkafka_projects_locations_connect_clusters_connectors_resume_builder, managedkafka_projects_locations_connect_clusters_connectors_resume_task,
    managedkafka_projects_locations_connect_clusters_connectors_stop_builder, managedkafka_projects_locations_connect_clusters_connectors_stop_task,
    managedkafka_projects_locations_operations_cancel_builder, managedkafka_projects_locations_operations_cancel_task,
    managedkafka_projects_locations_operations_delete_builder, managedkafka_projects_locations_operations_delete_task,
    managedkafka_projects_locations_operations_get_builder, managedkafka_projects_locations_operations_get_task,
    managedkafka_projects_locations_operations_list_builder, managedkafka_projects_locations_operations_list_task,
    managedkafka_projects_locations_schema_registries_create_builder, managedkafka_projects_locations_schema_registries_create_task,
    managedkafka_projects_locations_schema_registries_delete_builder, managedkafka_projects_locations_schema_registries_delete_task,
    managedkafka_projects_locations_schema_registries_get_builder, managedkafka_projects_locations_schema_registries_get_task,
    managedkafka_projects_locations_schema_registries_list_builder, managedkafka_projects_locations_schema_registries_list_task,
    managedkafka_projects_locations_schema_registries_compatibility_check_compatibility_builder, managedkafka_projects_locations_schema_registries_compatibility_check_compatibility_task,
    managedkafka_projects_locations_schema_registries_config_delete_builder, managedkafka_projects_locations_schema_registries_config_delete_task,
    managedkafka_projects_locations_schema_registries_config_get_builder, managedkafka_projects_locations_schema_registries_config_get_task,
    managedkafka_projects_locations_schema_registries_config_update_builder, managedkafka_projects_locations_schema_registries_config_update_task,
    managedkafka_projects_locations_schema_registries_contexts_get_builder, managedkafka_projects_locations_schema_registries_contexts_get_task,
    managedkafka_projects_locations_schema_registries_contexts_list_builder, managedkafka_projects_locations_schema_registries_contexts_list_task,
    managedkafka_projects_locations_schema_registries_contexts_compatibility_check_compatibility_builder, managedkafka_projects_locations_schema_registries_contexts_compatibility_check_compatibility_task,
    managedkafka_projects_locations_schema_registries_contexts_config_delete_builder, managedkafka_projects_locations_schema_registries_contexts_config_delete_task,
    managedkafka_projects_locations_schema_registries_contexts_config_get_builder, managedkafka_projects_locations_schema_registries_contexts_config_get_task,
    managedkafka_projects_locations_schema_registries_contexts_config_update_builder, managedkafka_projects_locations_schema_registries_contexts_config_update_task,
    managedkafka_projects_locations_schema_registries_contexts_mode_delete_builder, managedkafka_projects_locations_schema_registries_contexts_mode_delete_task,
    managedkafka_projects_locations_schema_registries_contexts_mode_get_builder, managedkafka_projects_locations_schema_registries_contexts_mode_get_task,
    managedkafka_projects_locations_schema_registries_contexts_mode_update_builder, managedkafka_projects_locations_schema_registries_contexts_mode_update_task,
    managedkafka_projects_locations_schema_registries_contexts_schemas_get_builder, managedkafka_projects_locations_schema_registries_contexts_schemas_get_task,
    managedkafka_projects_locations_schema_registries_contexts_schemas_get_schema_builder, managedkafka_projects_locations_schema_registries_contexts_schemas_get_schema_task,
    managedkafka_projects_locations_schema_registries_contexts_schemas_subjects_list_builder, managedkafka_projects_locations_schema_registries_contexts_schemas_subjects_list_task,
    managedkafka_projects_locations_schema_registries_contexts_schemas_types_list_builder, managedkafka_projects_locations_schema_registries_contexts_schemas_types_list_task,
    managedkafka_projects_locations_schema_registries_contexts_schemas_versions_list_builder, managedkafka_projects_locations_schema_registries_contexts_schemas_versions_list_task,
    managedkafka_projects_locations_schema_registries_contexts_subjects_delete_builder, managedkafka_projects_locations_schema_registries_contexts_subjects_delete_task,
    managedkafka_projects_locations_schema_registries_contexts_subjects_list_builder, managedkafka_projects_locations_schema_registries_contexts_subjects_list_task,
    managedkafka_projects_locations_schema_registries_contexts_subjects_lookup_version_builder, managedkafka_projects_locations_schema_registries_contexts_subjects_lookup_version_task,
    managedkafka_projects_locations_schema_registries_contexts_subjects_versions_create_builder, managedkafka_projects_locations_schema_registries_contexts_subjects_versions_create_task,
    managedkafka_projects_locations_schema_registries_contexts_subjects_versions_delete_builder, managedkafka_projects_locations_schema_registries_contexts_subjects_versions_delete_task,
    managedkafka_projects_locations_schema_registries_contexts_subjects_versions_get_builder, managedkafka_projects_locations_schema_registries_contexts_subjects_versions_get_task,
    managedkafka_projects_locations_schema_registries_contexts_subjects_versions_get_schema_builder, managedkafka_projects_locations_schema_registries_contexts_subjects_versions_get_schema_task,
    managedkafka_projects_locations_schema_registries_contexts_subjects_versions_list_builder, managedkafka_projects_locations_schema_registries_contexts_subjects_versions_list_task,
    managedkafka_projects_locations_schema_registries_contexts_subjects_versions_referencedby_list_builder, managedkafka_projects_locations_schema_registries_contexts_subjects_versions_referencedby_list_task,
    managedkafka_projects_locations_schema_registries_mode_delete_builder, managedkafka_projects_locations_schema_registries_mode_delete_task,
    managedkafka_projects_locations_schema_registries_mode_get_builder, managedkafka_projects_locations_schema_registries_mode_get_task,
    managedkafka_projects_locations_schema_registries_mode_update_builder, managedkafka_projects_locations_schema_registries_mode_update_task,
    managedkafka_projects_locations_schema_registries_schemas_get_builder, managedkafka_projects_locations_schema_registries_schemas_get_task,
    managedkafka_projects_locations_schema_registries_schemas_get_schema_builder, managedkafka_projects_locations_schema_registries_schemas_get_schema_task,
    managedkafka_projects_locations_schema_registries_schemas_subjects_list_builder, managedkafka_projects_locations_schema_registries_schemas_subjects_list_task,
    managedkafka_projects_locations_schema_registries_schemas_types_list_builder, managedkafka_projects_locations_schema_registries_schemas_types_list_task,
    managedkafka_projects_locations_schema_registries_schemas_versions_list_builder, managedkafka_projects_locations_schema_registries_schemas_versions_list_task,
    managedkafka_projects_locations_schema_registries_subjects_delete_builder, managedkafka_projects_locations_schema_registries_subjects_delete_task,
    managedkafka_projects_locations_schema_registries_subjects_list_builder, managedkafka_projects_locations_schema_registries_subjects_list_task,
    managedkafka_projects_locations_schema_registries_subjects_lookup_version_builder, managedkafka_projects_locations_schema_registries_subjects_lookup_version_task,
    managedkafka_projects_locations_schema_registries_subjects_versions_create_builder, managedkafka_projects_locations_schema_registries_subjects_versions_create_task,
    managedkafka_projects_locations_schema_registries_subjects_versions_delete_builder, managedkafka_projects_locations_schema_registries_subjects_versions_delete_task,
    managedkafka_projects_locations_schema_registries_subjects_versions_get_builder, managedkafka_projects_locations_schema_registries_subjects_versions_get_task,
    managedkafka_projects_locations_schema_registries_subjects_versions_get_schema_builder, managedkafka_projects_locations_schema_registries_subjects_versions_get_schema_task,
    managedkafka_projects_locations_schema_registries_subjects_versions_list_builder, managedkafka_projects_locations_schema_registries_subjects_versions_list_task,
    managedkafka_projects_locations_schema_registries_subjects_versions_referencedby_list_builder, managedkafka_projects_locations_schema_registries_subjects_versions_referencedby_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::managedkafka::Acl;
use crate::providers::gcp::clients::managedkafka::AddAclEntryResponse;
use crate::providers::gcp::clients::managedkafka::CheckCompatibilityResponse;
use crate::providers::gcp::clients::managedkafka::Cluster;
use crate::providers::gcp::clients::managedkafka::ConnectCluster;
use crate::providers::gcp::clients::managedkafka::Connector;
use crate::providers::gcp::clients::managedkafka::ConsumerGroup;
use crate::providers::gcp::clients::managedkafka::Context;
use crate::providers::gcp::clients::managedkafka::CreateVersionResponse;
use crate::providers::gcp::clients::managedkafka::Empty;
use crate::providers::gcp::clients::managedkafka::HttpBody;
use crate::providers::gcp::clients::managedkafka::ListAclsResponse;
use crate::providers::gcp::clients::managedkafka::ListClustersResponse;
use crate::providers::gcp::clients::managedkafka::ListConnectClustersResponse;
use crate::providers::gcp::clients::managedkafka::ListConnectorsResponse;
use crate::providers::gcp::clients::managedkafka::ListConsumerGroupsResponse;
use crate::providers::gcp::clients::managedkafka::ListLocationsResponse;
use crate::providers::gcp::clients::managedkafka::ListOperationsResponse;
use crate::providers::gcp::clients::managedkafka::ListSchemaRegistriesResponse;
use crate::providers::gcp::clients::managedkafka::ListTopicsResponse;
use crate::providers::gcp::clients::managedkafka::Location;
use crate::providers::gcp::clients::managedkafka::Operation;
use crate::providers::gcp::clients::managedkafka::PauseConnectorResponse;
use crate::providers::gcp::clients::managedkafka::RemoveAclEntryResponse;
use crate::providers::gcp::clients::managedkafka::RestartConnectorResponse;
use crate::providers::gcp::clients::managedkafka::ResumeConnectorResponse;
use crate::providers::gcp::clients::managedkafka::Schema;
use crate::providers::gcp::clients::managedkafka::SchemaConfig;
use crate::providers::gcp::clients::managedkafka::SchemaMode;
use crate::providers::gcp::clients::managedkafka::SchemaRegistry;
use crate::providers::gcp::clients::managedkafka::SchemaVersion;
use crate::providers::gcp::clients::managedkafka::StopConnectorResponse;
use crate::providers::gcp::clients::managedkafka::Topic;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersAclsAddAclEntryArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersAclsCreateArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersAclsDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersAclsGetArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersAclsListArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersAclsPatchArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersAclsRemoveAclEntryArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersConsumerGroupsDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersConsumerGroupsGetArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersConsumerGroupsListArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersConsumerGroupsPatchArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersCreateArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersGetArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersListArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersPatchArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersTopicsCreateArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersTopicsDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersTopicsGetArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersTopicsListArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersTopicsPatchArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsConnectClustersConnectorsCreateArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsConnectClustersConnectorsDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsConnectClustersConnectorsGetArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsConnectClustersConnectorsListArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsConnectClustersConnectorsPatchArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsConnectClustersConnectorsPauseArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsConnectClustersConnectorsRestartArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsConnectClustersConnectorsResumeArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsConnectClustersConnectorsStopArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsConnectClustersCreateArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsConnectClustersDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsConnectClustersGetArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsConnectClustersListArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsConnectClustersPatchArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsGetArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsListArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesCompatibilityCheckCompatibilityArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesConfigDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesConfigGetArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesConfigUpdateArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsCompatibilityCheckCompatibilityArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsConfigDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsConfigGetArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsConfigUpdateArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsGetArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsListArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsModeDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsModeGetArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsModeUpdateArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsSchemasGetArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsSchemasGetSchemaArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsSchemasSubjectsListArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsSchemasTypesListArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsSchemasVersionsListArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsSubjectsDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsSubjectsListArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsSubjectsLookupVersionArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsSubjectsVersionsCreateArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsSubjectsVersionsDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsSubjectsVersionsGetArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsSubjectsVersionsGetSchemaArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsSubjectsVersionsListArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsSubjectsVersionsReferencedbyListArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesCreateArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesGetArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesListArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesModeDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesModeGetArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesModeUpdateArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesSchemasGetArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesSchemasGetSchemaArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesSchemasSubjectsListArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesSchemasTypesListArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesSchemasVersionsListArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesSubjectsDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesSubjectsListArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesSubjectsLookupVersionArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesSubjectsVersionsCreateArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesSubjectsVersionsDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesSubjectsVersionsGetArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesSubjectsVersionsGetSchemaArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesSubjectsVersionsListArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesSubjectsVersionsReferencedbyListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ManagedkafkaProvider with automatic state tracking.
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
/// let provider = ManagedkafkaProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ManagedkafkaProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ManagedkafkaProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ManagedkafkaProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Managedkafka projects locations get.
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
    pub fn managedkafka_projects_locations_get(
        &self,
        args: &ManagedkafkaProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations list.
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
    pub fn managedkafka_projects_locations_list(
        &self,
        args: &ManagedkafkaProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters create.
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
    pub fn managedkafka_projects_locations_clusters_create(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_create_builder(
            &self.http_client,
            &args.parent,
            &args.clusterId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters delete.
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
    pub fn managedkafka_projects_locations_clusters_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Cluster result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_clusters_get(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Cluster, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_get_builder(
            &self.http_client,
            &args.name,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListClustersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_clusters_list(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListClustersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters patch.
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
    pub fn managedkafka_projects_locations_clusters_patch(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters acls add acl entry.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AddAclEntryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_clusters_acls_add_acl_entry(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersAclsAddAclEntryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AddAclEntryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_acls_add_acl_entry_builder(
            &self.http_client,
            &args.acl,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_acls_add_acl_entry_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters acls create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Acl result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_clusters_acls_create(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersAclsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Acl, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_acls_create_builder(
            &self.http_client,
            &args.parent,
            &args.aclId,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_acls_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters acls delete.
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
    pub fn managedkafka_projects_locations_clusters_acls_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersAclsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_acls_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_acls_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters acls get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Acl result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_clusters_acls_get(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersAclsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Acl, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_acls_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_acls_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters acls list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAclsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_clusters_acls_list(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersAclsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAclsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_acls_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_acls_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters acls patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Acl result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_clusters_acls_patch(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersAclsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Acl, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_acls_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_acls_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters acls remove acl entry.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RemoveAclEntryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_clusters_acls_remove_acl_entry(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersAclsRemoveAclEntryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RemoveAclEntryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_acls_remove_acl_entry_builder(
            &self.http_client,
            &args.acl,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_acls_remove_acl_entry_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters consumer groups delete.
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
    pub fn managedkafka_projects_locations_clusters_consumer_groups_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersConsumerGroupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_consumer_groups_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_consumer_groups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters consumer groups get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConsumerGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_clusters_consumer_groups_get(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersConsumerGroupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConsumerGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_consumer_groups_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_consumer_groups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters consumer groups list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListConsumerGroupsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_clusters_consumer_groups_list(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersConsumerGroupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListConsumerGroupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_consumer_groups_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_consumer_groups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters consumer groups patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConsumerGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_clusters_consumer_groups_patch(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersConsumerGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConsumerGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_consumer_groups_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_consumer_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters topics create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Topic result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_clusters_topics_create(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersTopicsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Topic, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_topics_create_builder(
            &self.http_client,
            &args.parent,
            &args.topicId,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_topics_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters topics delete.
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
    pub fn managedkafka_projects_locations_clusters_topics_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersTopicsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_topics_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_topics_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters topics get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Topic result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_clusters_topics_get(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersTopicsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Topic, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_topics_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_topics_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters topics list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTopicsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_clusters_topics_list(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersTopicsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTopicsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_topics_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_topics_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters topics patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Topic result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_clusters_topics_patch(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersTopicsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Topic, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_topics_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_topics_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations connect clusters create.
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
    pub fn managedkafka_projects_locations_connect_clusters_create(
        &self,
        args: &ManagedkafkaProjectsLocationsConnectClustersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_connect_clusters_create_builder(
            &self.http_client,
            &args.parent,
            &args.connectClusterId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_connect_clusters_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations connect clusters delete.
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
    pub fn managedkafka_projects_locations_connect_clusters_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsConnectClustersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_connect_clusters_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_connect_clusters_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations connect clusters get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConnectCluster result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_connect_clusters_get(
        &self,
        args: &ManagedkafkaProjectsLocationsConnectClustersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConnectCluster, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_connect_clusters_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_connect_clusters_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations connect clusters list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListConnectClustersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_connect_clusters_list(
        &self,
        args: &ManagedkafkaProjectsLocationsConnectClustersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListConnectClustersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_connect_clusters_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_connect_clusters_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations connect clusters patch.
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
    pub fn managedkafka_projects_locations_connect_clusters_patch(
        &self,
        args: &ManagedkafkaProjectsLocationsConnectClustersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_connect_clusters_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_connect_clusters_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations connect clusters connectors create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Connector result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_connect_clusters_connectors_create(
        &self,
        args: &ManagedkafkaProjectsLocationsConnectClustersConnectorsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Connector, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_connect_clusters_connectors_create_builder(
            &self.http_client,
            &args.parent,
            &args.connectorId,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_connect_clusters_connectors_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations connect clusters connectors delete.
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
    pub fn managedkafka_projects_locations_connect_clusters_connectors_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsConnectClustersConnectorsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_connect_clusters_connectors_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_connect_clusters_connectors_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations connect clusters connectors get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Connector result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_connect_clusters_connectors_get(
        &self,
        args: &ManagedkafkaProjectsLocationsConnectClustersConnectorsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Connector, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_connect_clusters_connectors_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_connect_clusters_connectors_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations connect clusters connectors list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListConnectorsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_connect_clusters_connectors_list(
        &self,
        args: &ManagedkafkaProjectsLocationsConnectClustersConnectorsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListConnectorsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_connect_clusters_connectors_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_connect_clusters_connectors_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations connect clusters connectors patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Connector result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_connect_clusters_connectors_patch(
        &self,
        args: &ManagedkafkaProjectsLocationsConnectClustersConnectorsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Connector, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_connect_clusters_connectors_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_connect_clusters_connectors_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations connect clusters connectors pause.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PauseConnectorResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_connect_clusters_connectors_pause(
        &self,
        args: &ManagedkafkaProjectsLocationsConnectClustersConnectorsPauseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PauseConnectorResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_connect_clusters_connectors_pause_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_connect_clusters_connectors_pause_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations connect clusters connectors restart.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RestartConnectorResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_connect_clusters_connectors_restart(
        &self,
        args: &ManagedkafkaProjectsLocationsConnectClustersConnectorsRestartArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RestartConnectorResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_connect_clusters_connectors_restart_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_connect_clusters_connectors_restart_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations connect clusters connectors resume.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ResumeConnectorResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_connect_clusters_connectors_resume(
        &self,
        args: &ManagedkafkaProjectsLocationsConnectClustersConnectorsResumeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ResumeConnectorResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_connect_clusters_connectors_resume_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_connect_clusters_connectors_resume_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations connect clusters connectors stop.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StopConnectorResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_connect_clusters_connectors_stop(
        &self,
        args: &ManagedkafkaProjectsLocationsConnectClustersConnectorsStopArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StopConnectorResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_connect_clusters_connectors_stop_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_connect_clusters_connectors_stop_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations operations cancel.
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
    pub fn managedkafka_projects_locations_operations_cancel(
        &self,
        args: &ManagedkafkaProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations operations delete.
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
    pub fn managedkafka_projects_locations_operations_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations operations get.
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
    pub fn managedkafka_projects_locations_operations_get(
        &self,
        args: &ManagedkafkaProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations operations list.
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
    pub fn managedkafka_projects_locations_operations_list(
        &self,
        args: &ManagedkafkaProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaRegistry result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_create(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaRegistry, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries delete.
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
    pub fn managedkafka_projects_locations_schema_registries_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaRegistry result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_schema_registries_get(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaRegistry, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSchemaRegistriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_schema_registries_list(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSchemaRegistriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_list_builder(
            &self.http_client,
            &args.parent,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries compatibility check compatibility.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CheckCompatibilityResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_compatibility_check_compatibility(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesCompatibilityCheckCompatibilityArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CheckCompatibilityResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_compatibility_check_compatibility_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_compatibility_check_compatibility_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries config delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_config_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesConfigDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_config_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_config_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries config get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_schema_registries_config_get(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesConfigGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_config_get_builder(
            &self.http_client,
            &args.name,
            &args.defaultToGlobal,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_config_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries config update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_config_update(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesConfigUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_config_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_config_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Context result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_get(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Context, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_list(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_list_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts compatibility check compatibility.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CheckCompatibilityResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_compatibility_check_compatibility(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsCompatibilityCheckCompatibilityArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CheckCompatibilityResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_compatibility_check_compatibility_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_compatibility_check_compatibility_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts config delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_config_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsConfigDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_config_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_config_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts config get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_config_get(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsConfigGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_config_get_builder(
            &self.http_client,
            &args.name,
            &args.defaultToGlobal,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_config_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts config update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_config_update(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsConfigUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_config_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_config_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts mode delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaMode result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_mode_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsModeDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaMode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_mode_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_mode_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts mode get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaMode result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_mode_get(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsModeGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaMode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_mode_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_mode_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts mode update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaMode result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_mode_update(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsModeUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaMode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_mode_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_mode_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts schemas get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Schema result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_schemas_get(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsSchemasGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Schema, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_schemas_get_builder(
            &self.http_client,
            &args.name,
            &args.subject,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_schemas_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts schemas get schema.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_schemas_get_schema(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsSchemasGetSchemaArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_schemas_get_schema_builder(
            &self.http_client,
            &args.name,
            &args.subject,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_schemas_get_schema_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts schemas subjects list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_schemas_subjects_list(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsSchemasSubjectsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_schemas_subjects_list_builder(
            &self.http_client,
            &args.parent,
            &args.deleted,
            &args.subject,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_schemas_subjects_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts schemas types list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_schemas_types_list(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsSchemasTypesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_schemas_types_list_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_schemas_types_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts schemas versions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_schemas_versions_list(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsSchemasVersionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_schemas_versions_list_builder(
            &self.http_client,
            &args.parent,
            &args.deleted,
            &args.subject,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_schemas_versions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts subjects delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_subjects_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsSubjectsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_subjects_delete_builder(
            &self.http_client,
            &args.name,
            &args.permanent,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_subjects_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts subjects list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_subjects_list(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsSubjectsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_subjects_list_builder(
            &self.http_client,
            &args.parent,
            &args.deleted,
            &args.subjectPrefix,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_subjects_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts subjects lookup version.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_subjects_lookup_version(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsSubjectsLookupVersionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_subjects_lookup_version_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_subjects_lookup_version_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts subjects versions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreateVersionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_subjects_versions_create(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsSubjectsVersionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreateVersionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_subjects_versions_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_subjects_versions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts subjects versions delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_subjects_versions_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsSubjectsVersionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_subjects_versions_delete_builder(
            &self.http_client,
            &args.name,
            &args.permanent,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_subjects_versions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts subjects versions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_subjects_versions_get(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsSubjectsVersionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_subjects_versions_get_builder(
            &self.http_client,
            &args.name,
            &args.deleted,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_subjects_versions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts subjects versions get schema.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_subjects_versions_get_schema(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsSubjectsVersionsGetSchemaArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_subjects_versions_get_schema_builder(
            &self.http_client,
            &args.name,
            &args.deleted,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_subjects_versions_get_schema_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts subjects versions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_subjects_versions_list(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsSubjectsVersionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_subjects_versions_list_builder(
            &self.http_client,
            &args.parent,
            &args.deleted,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_subjects_versions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts subjects versions referencedby list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_subjects_versions_referencedby_list(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsSubjectsVersionsReferencedbyListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_subjects_versions_referencedby_list_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_subjects_versions_referencedby_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries mode delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaMode result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_mode_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesModeDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaMode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_mode_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_mode_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries mode get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaMode result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_schema_registries_mode_get(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesModeGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaMode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_mode_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_mode_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries mode update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaMode result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_mode_update(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesModeUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaMode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_mode_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_mode_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries schemas get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Schema result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_schema_registries_schemas_get(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesSchemasGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Schema, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_schemas_get_builder(
            &self.http_client,
            &args.name,
            &args.subject,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_schemas_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries schemas get schema.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_schema_registries_schemas_get_schema(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesSchemasGetSchemaArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_schemas_get_schema_builder(
            &self.http_client,
            &args.name,
            &args.subject,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_schemas_get_schema_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries schemas subjects list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_schema_registries_schemas_subjects_list(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesSchemasSubjectsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_schemas_subjects_list_builder(
            &self.http_client,
            &args.parent,
            &args.deleted,
            &args.subject,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_schemas_subjects_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries schemas types list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_schema_registries_schemas_types_list(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesSchemasTypesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_schemas_types_list_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_schemas_types_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries schemas versions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_schema_registries_schemas_versions_list(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesSchemasVersionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_schemas_versions_list_builder(
            &self.http_client,
            &args.parent,
            &args.deleted,
            &args.subject,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_schemas_versions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries subjects delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_subjects_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesSubjectsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_subjects_delete_builder(
            &self.http_client,
            &args.name,
            &args.permanent,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_subjects_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries subjects list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_schema_registries_subjects_list(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesSubjectsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_subjects_list_builder(
            &self.http_client,
            &args.parent,
            &args.deleted,
            &args.subjectPrefix,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_subjects_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries subjects lookup version.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_subjects_lookup_version(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesSubjectsLookupVersionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_subjects_lookup_version_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_subjects_lookup_version_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries subjects versions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreateVersionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_subjects_versions_create(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesSubjectsVersionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreateVersionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_subjects_versions_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_subjects_versions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries subjects versions delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_subjects_versions_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesSubjectsVersionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_subjects_versions_delete_builder(
            &self.http_client,
            &args.name,
            &args.permanent,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_subjects_versions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries subjects versions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_schema_registries_subjects_versions_get(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesSubjectsVersionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_subjects_versions_get_builder(
            &self.http_client,
            &args.name,
            &args.deleted,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_subjects_versions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries subjects versions get schema.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_schema_registries_subjects_versions_get_schema(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesSubjectsVersionsGetSchemaArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_subjects_versions_get_schema_builder(
            &self.http_client,
            &args.name,
            &args.deleted,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_subjects_versions_get_schema_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries subjects versions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_schema_registries_subjects_versions_list(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesSubjectsVersionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_subjects_versions_list_builder(
            &self.http_client,
            &args.parent,
            &args.deleted,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_subjects_versions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries subjects versions referencedby list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedkafka_projects_locations_schema_registries_subjects_versions_referencedby_list(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesSubjectsVersionsReferencedbyListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_subjects_versions_referencedby_list_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_subjects_versions_referencedby_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
