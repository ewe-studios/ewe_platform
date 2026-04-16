//! ApigeeregistryProvider - State-aware apigeeregistry API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       apigeeregistry API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::apigeeregistry::{
    apigeeregistry_projects_locations_get_builder, apigeeregistry_projects_locations_get_task,
    apigeeregistry_projects_locations_list_builder, apigeeregistry_projects_locations_list_task,
    apigeeregistry_projects_locations_apis_create_builder, apigeeregistry_projects_locations_apis_create_task,
    apigeeregistry_projects_locations_apis_delete_builder, apigeeregistry_projects_locations_apis_delete_task,
    apigeeregistry_projects_locations_apis_get_builder, apigeeregistry_projects_locations_apis_get_task,
    apigeeregistry_projects_locations_apis_get_iam_policy_builder, apigeeregistry_projects_locations_apis_get_iam_policy_task,
    apigeeregistry_projects_locations_apis_list_builder, apigeeregistry_projects_locations_apis_list_task,
    apigeeregistry_projects_locations_apis_patch_builder, apigeeregistry_projects_locations_apis_patch_task,
    apigeeregistry_projects_locations_apis_set_iam_policy_builder, apigeeregistry_projects_locations_apis_set_iam_policy_task,
    apigeeregistry_projects_locations_apis_test_iam_permissions_builder, apigeeregistry_projects_locations_apis_test_iam_permissions_task,
    apigeeregistry_projects_locations_apis_artifacts_create_builder, apigeeregistry_projects_locations_apis_artifacts_create_task,
    apigeeregistry_projects_locations_apis_artifacts_delete_builder, apigeeregistry_projects_locations_apis_artifacts_delete_task,
    apigeeregistry_projects_locations_apis_artifacts_get_builder, apigeeregistry_projects_locations_apis_artifacts_get_task,
    apigeeregistry_projects_locations_apis_artifacts_get_contents_builder, apigeeregistry_projects_locations_apis_artifacts_get_contents_task,
    apigeeregistry_projects_locations_apis_artifacts_get_iam_policy_builder, apigeeregistry_projects_locations_apis_artifacts_get_iam_policy_task,
    apigeeregistry_projects_locations_apis_artifacts_list_builder, apigeeregistry_projects_locations_apis_artifacts_list_task,
    apigeeregistry_projects_locations_apis_artifacts_replace_artifact_builder, apigeeregistry_projects_locations_apis_artifacts_replace_artifact_task,
    apigeeregistry_projects_locations_apis_artifacts_set_iam_policy_builder, apigeeregistry_projects_locations_apis_artifacts_set_iam_policy_task,
    apigeeregistry_projects_locations_apis_artifacts_test_iam_permissions_builder, apigeeregistry_projects_locations_apis_artifacts_test_iam_permissions_task,
    apigeeregistry_projects_locations_apis_deployments_create_builder, apigeeregistry_projects_locations_apis_deployments_create_task,
    apigeeregistry_projects_locations_apis_deployments_delete_builder, apigeeregistry_projects_locations_apis_deployments_delete_task,
    apigeeregistry_projects_locations_apis_deployments_delete_revision_builder, apigeeregistry_projects_locations_apis_deployments_delete_revision_task,
    apigeeregistry_projects_locations_apis_deployments_get_builder, apigeeregistry_projects_locations_apis_deployments_get_task,
    apigeeregistry_projects_locations_apis_deployments_get_iam_policy_builder, apigeeregistry_projects_locations_apis_deployments_get_iam_policy_task,
    apigeeregistry_projects_locations_apis_deployments_list_builder, apigeeregistry_projects_locations_apis_deployments_list_task,
    apigeeregistry_projects_locations_apis_deployments_list_revisions_builder, apigeeregistry_projects_locations_apis_deployments_list_revisions_task,
    apigeeregistry_projects_locations_apis_deployments_patch_builder, apigeeregistry_projects_locations_apis_deployments_patch_task,
    apigeeregistry_projects_locations_apis_deployments_rollback_builder, apigeeregistry_projects_locations_apis_deployments_rollback_task,
    apigeeregistry_projects_locations_apis_deployments_set_iam_policy_builder, apigeeregistry_projects_locations_apis_deployments_set_iam_policy_task,
    apigeeregistry_projects_locations_apis_deployments_tag_revision_builder, apigeeregistry_projects_locations_apis_deployments_tag_revision_task,
    apigeeregistry_projects_locations_apis_deployments_test_iam_permissions_builder, apigeeregistry_projects_locations_apis_deployments_test_iam_permissions_task,
    apigeeregistry_projects_locations_apis_deployments_artifacts_create_builder, apigeeregistry_projects_locations_apis_deployments_artifacts_create_task,
    apigeeregistry_projects_locations_apis_deployments_artifacts_delete_builder, apigeeregistry_projects_locations_apis_deployments_artifacts_delete_task,
    apigeeregistry_projects_locations_apis_deployments_artifacts_get_builder, apigeeregistry_projects_locations_apis_deployments_artifacts_get_task,
    apigeeregistry_projects_locations_apis_deployments_artifacts_get_contents_builder, apigeeregistry_projects_locations_apis_deployments_artifacts_get_contents_task,
    apigeeregistry_projects_locations_apis_deployments_artifacts_list_builder, apigeeregistry_projects_locations_apis_deployments_artifacts_list_task,
    apigeeregistry_projects_locations_apis_deployments_artifacts_replace_artifact_builder, apigeeregistry_projects_locations_apis_deployments_artifacts_replace_artifact_task,
    apigeeregistry_projects_locations_apis_versions_create_builder, apigeeregistry_projects_locations_apis_versions_create_task,
    apigeeregistry_projects_locations_apis_versions_delete_builder, apigeeregistry_projects_locations_apis_versions_delete_task,
    apigeeregistry_projects_locations_apis_versions_get_builder, apigeeregistry_projects_locations_apis_versions_get_task,
    apigeeregistry_projects_locations_apis_versions_get_iam_policy_builder, apigeeregistry_projects_locations_apis_versions_get_iam_policy_task,
    apigeeregistry_projects_locations_apis_versions_list_builder, apigeeregistry_projects_locations_apis_versions_list_task,
    apigeeregistry_projects_locations_apis_versions_patch_builder, apigeeregistry_projects_locations_apis_versions_patch_task,
    apigeeregistry_projects_locations_apis_versions_set_iam_policy_builder, apigeeregistry_projects_locations_apis_versions_set_iam_policy_task,
    apigeeregistry_projects_locations_apis_versions_test_iam_permissions_builder, apigeeregistry_projects_locations_apis_versions_test_iam_permissions_task,
    apigeeregistry_projects_locations_apis_versions_artifacts_create_builder, apigeeregistry_projects_locations_apis_versions_artifacts_create_task,
    apigeeregistry_projects_locations_apis_versions_artifacts_delete_builder, apigeeregistry_projects_locations_apis_versions_artifacts_delete_task,
    apigeeregistry_projects_locations_apis_versions_artifacts_get_builder, apigeeregistry_projects_locations_apis_versions_artifacts_get_task,
    apigeeregistry_projects_locations_apis_versions_artifacts_get_contents_builder, apigeeregistry_projects_locations_apis_versions_artifacts_get_contents_task,
    apigeeregistry_projects_locations_apis_versions_artifacts_get_iam_policy_builder, apigeeregistry_projects_locations_apis_versions_artifacts_get_iam_policy_task,
    apigeeregistry_projects_locations_apis_versions_artifacts_list_builder, apigeeregistry_projects_locations_apis_versions_artifacts_list_task,
    apigeeregistry_projects_locations_apis_versions_artifacts_replace_artifact_builder, apigeeregistry_projects_locations_apis_versions_artifacts_replace_artifact_task,
    apigeeregistry_projects_locations_apis_versions_artifacts_set_iam_policy_builder, apigeeregistry_projects_locations_apis_versions_artifacts_set_iam_policy_task,
    apigeeregistry_projects_locations_apis_versions_artifacts_test_iam_permissions_builder, apigeeregistry_projects_locations_apis_versions_artifacts_test_iam_permissions_task,
    apigeeregistry_projects_locations_apis_versions_specs_create_builder, apigeeregistry_projects_locations_apis_versions_specs_create_task,
    apigeeregistry_projects_locations_apis_versions_specs_delete_builder, apigeeregistry_projects_locations_apis_versions_specs_delete_task,
    apigeeregistry_projects_locations_apis_versions_specs_delete_revision_builder, apigeeregistry_projects_locations_apis_versions_specs_delete_revision_task,
    apigeeregistry_projects_locations_apis_versions_specs_get_builder, apigeeregistry_projects_locations_apis_versions_specs_get_task,
    apigeeregistry_projects_locations_apis_versions_specs_get_contents_builder, apigeeregistry_projects_locations_apis_versions_specs_get_contents_task,
    apigeeregistry_projects_locations_apis_versions_specs_get_iam_policy_builder, apigeeregistry_projects_locations_apis_versions_specs_get_iam_policy_task,
    apigeeregistry_projects_locations_apis_versions_specs_list_builder, apigeeregistry_projects_locations_apis_versions_specs_list_task,
    apigeeregistry_projects_locations_apis_versions_specs_list_revisions_builder, apigeeregistry_projects_locations_apis_versions_specs_list_revisions_task,
    apigeeregistry_projects_locations_apis_versions_specs_patch_builder, apigeeregistry_projects_locations_apis_versions_specs_patch_task,
    apigeeregistry_projects_locations_apis_versions_specs_rollback_builder, apigeeregistry_projects_locations_apis_versions_specs_rollback_task,
    apigeeregistry_projects_locations_apis_versions_specs_set_iam_policy_builder, apigeeregistry_projects_locations_apis_versions_specs_set_iam_policy_task,
    apigeeregistry_projects_locations_apis_versions_specs_tag_revision_builder, apigeeregistry_projects_locations_apis_versions_specs_tag_revision_task,
    apigeeregistry_projects_locations_apis_versions_specs_test_iam_permissions_builder, apigeeregistry_projects_locations_apis_versions_specs_test_iam_permissions_task,
    apigeeregistry_projects_locations_apis_versions_specs_artifacts_create_builder, apigeeregistry_projects_locations_apis_versions_specs_artifacts_create_task,
    apigeeregistry_projects_locations_apis_versions_specs_artifacts_delete_builder, apigeeregistry_projects_locations_apis_versions_specs_artifacts_delete_task,
    apigeeregistry_projects_locations_apis_versions_specs_artifacts_get_builder, apigeeregistry_projects_locations_apis_versions_specs_artifacts_get_task,
    apigeeregistry_projects_locations_apis_versions_specs_artifacts_get_contents_builder, apigeeregistry_projects_locations_apis_versions_specs_artifacts_get_contents_task,
    apigeeregistry_projects_locations_apis_versions_specs_artifacts_get_iam_policy_builder, apigeeregistry_projects_locations_apis_versions_specs_artifacts_get_iam_policy_task,
    apigeeregistry_projects_locations_apis_versions_specs_artifacts_list_builder, apigeeregistry_projects_locations_apis_versions_specs_artifacts_list_task,
    apigeeregistry_projects_locations_apis_versions_specs_artifacts_replace_artifact_builder, apigeeregistry_projects_locations_apis_versions_specs_artifacts_replace_artifact_task,
    apigeeregistry_projects_locations_apis_versions_specs_artifacts_set_iam_policy_builder, apigeeregistry_projects_locations_apis_versions_specs_artifacts_set_iam_policy_task,
    apigeeregistry_projects_locations_apis_versions_specs_artifacts_test_iam_permissions_builder, apigeeregistry_projects_locations_apis_versions_specs_artifacts_test_iam_permissions_task,
    apigeeregistry_projects_locations_artifacts_create_builder, apigeeregistry_projects_locations_artifacts_create_task,
    apigeeregistry_projects_locations_artifacts_delete_builder, apigeeregistry_projects_locations_artifacts_delete_task,
    apigeeregistry_projects_locations_artifacts_get_builder, apigeeregistry_projects_locations_artifacts_get_task,
    apigeeregistry_projects_locations_artifacts_get_contents_builder, apigeeregistry_projects_locations_artifacts_get_contents_task,
    apigeeregistry_projects_locations_artifacts_get_iam_policy_builder, apigeeregistry_projects_locations_artifacts_get_iam_policy_task,
    apigeeregistry_projects_locations_artifacts_list_builder, apigeeregistry_projects_locations_artifacts_list_task,
    apigeeregistry_projects_locations_artifacts_replace_artifact_builder, apigeeregistry_projects_locations_artifacts_replace_artifact_task,
    apigeeregistry_projects_locations_artifacts_set_iam_policy_builder, apigeeregistry_projects_locations_artifacts_set_iam_policy_task,
    apigeeregistry_projects_locations_artifacts_test_iam_permissions_builder, apigeeregistry_projects_locations_artifacts_test_iam_permissions_task,
    apigeeregistry_projects_locations_documents_get_iam_policy_builder, apigeeregistry_projects_locations_documents_get_iam_policy_task,
    apigeeregistry_projects_locations_documents_set_iam_policy_builder, apigeeregistry_projects_locations_documents_set_iam_policy_task,
    apigeeregistry_projects_locations_documents_test_iam_permissions_builder, apigeeregistry_projects_locations_documents_test_iam_permissions_task,
    apigeeregistry_projects_locations_instances_create_builder, apigeeregistry_projects_locations_instances_create_task,
    apigeeregistry_projects_locations_instances_delete_builder, apigeeregistry_projects_locations_instances_delete_task,
    apigeeregistry_projects_locations_instances_get_builder, apigeeregistry_projects_locations_instances_get_task,
    apigeeregistry_projects_locations_instances_get_iam_policy_builder, apigeeregistry_projects_locations_instances_get_iam_policy_task,
    apigeeregistry_projects_locations_instances_set_iam_policy_builder, apigeeregistry_projects_locations_instances_set_iam_policy_task,
    apigeeregistry_projects_locations_instances_test_iam_permissions_builder, apigeeregistry_projects_locations_instances_test_iam_permissions_task,
    apigeeregistry_projects_locations_operations_cancel_builder, apigeeregistry_projects_locations_operations_cancel_task,
    apigeeregistry_projects_locations_operations_delete_builder, apigeeregistry_projects_locations_operations_delete_task,
    apigeeregistry_projects_locations_operations_get_builder, apigeeregistry_projects_locations_operations_get_task,
    apigeeregistry_projects_locations_operations_list_builder, apigeeregistry_projects_locations_operations_list_task,
    apigeeregistry_projects_locations_runtime_get_iam_policy_builder, apigeeregistry_projects_locations_runtime_get_iam_policy_task,
    apigeeregistry_projects_locations_runtime_set_iam_policy_builder, apigeeregistry_projects_locations_runtime_set_iam_policy_task,
    apigeeregistry_projects_locations_runtime_test_iam_permissions_builder, apigeeregistry_projects_locations_runtime_test_iam_permissions_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::apigeeregistry::Api;
use crate::providers::gcp::clients::apigeeregistry::ApiDeployment;
use crate::providers::gcp::clients::apigeeregistry::ApiSpec;
use crate::providers::gcp::clients::apigeeregistry::ApiVersion;
use crate::providers::gcp::clients::apigeeregistry::Artifact;
use crate::providers::gcp::clients::apigeeregistry::Empty;
use crate::providers::gcp::clients::apigeeregistry::HttpBody;
use crate::providers::gcp::clients::apigeeregistry::Instance;
use crate::providers::gcp::clients::apigeeregistry::ListApiDeploymentRevisionsResponse;
use crate::providers::gcp::clients::apigeeregistry::ListApiDeploymentsResponse;
use crate::providers::gcp::clients::apigeeregistry::ListApiSpecRevisionsResponse;
use crate::providers::gcp::clients::apigeeregistry::ListApiSpecsResponse;
use crate::providers::gcp::clients::apigeeregistry::ListApiVersionsResponse;
use crate::providers::gcp::clients::apigeeregistry::ListApisResponse;
use crate::providers::gcp::clients::apigeeregistry::ListArtifactsResponse;
use crate::providers::gcp::clients::apigeeregistry::ListLocationsResponse;
use crate::providers::gcp::clients::apigeeregistry::ListOperationsResponse;
use crate::providers::gcp::clients::apigeeregistry::Location;
use crate::providers::gcp::clients::apigeeregistry::Operation;
use crate::providers::gcp::clients::apigeeregistry::Policy;
use crate::providers::gcp::clients::apigeeregistry::TestIamPermissionsResponse;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisArtifactsCreateArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisArtifactsDeleteArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisArtifactsGetArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisArtifactsGetContentsArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisArtifactsGetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisArtifactsListArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisArtifactsReplaceArtifactArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisArtifactsSetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisArtifactsTestIamPermissionsArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisCreateArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeleteArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeploymentsArtifactsCreateArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeploymentsArtifactsDeleteArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeploymentsArtifactsGetArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeploymentsArtifactsGetContentsArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeploymentsArtifactsListArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeploymentsArtifactsReplaceArtifactArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeploymentsCreateArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeploymentsDeleteArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeploymentsDeleteRevisionArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeploymentsGetArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeploymentsGetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeploymentsListArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeploymentsListRevisionsArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeploymentsPatchArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeploymentsRollbackArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeploymentsSetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeploymentsTagRevisionArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeploymentsTestIamPermissionsArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisGetArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisGetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisListArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisPatchArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisSetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisTestIamPermissionsArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsArtifactsCreateArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsArtifactsDeleteArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsArtifactsGetArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsArtifactsGetContentsArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsArtifactsGetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsArtifactsListArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsArtifactsReplaceArtifactArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsArtifactsSetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsArtifactsTestIamPermissionsArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsCreateArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsDeleteArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsGetArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsGetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsListArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsPatchArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsArtifactsCreateArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsArtifactsDeleteArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsArtifactsGetArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsArtifactsGetContentsArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsArtifactsGetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsArtifactsListArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsArtifactsReplaceArtifactArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsArtifactsSetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsArtifactsTestIamPermissionsArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsCreateArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsDeleteArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsDeleteRevisionArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsGetArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsGetContentsArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsGetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsListArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsListRevisionsArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsPatchArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsRollbackArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsSetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsTagRevisionArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsTestIamPermissionsArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsTestIamPermissionsArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsArtifactsCreateArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsArtifactsDeleteArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsArtifactsGetArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsArtifactsGetContentsArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsArtifactsGetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsArtifactsListArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsArtifactsReplaceArtifactArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsArtifactsSetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsArtifactsTestIamPermissionsArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsDocumentsGetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsDocumentsSetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsDocumentsTestIamPermissionsArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsGetArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsInstancesCreateArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsInstancesDeleteArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsInstancesGetArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsInstancesGetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsInstancesSetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsInstancesTestIamPermissionsArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsListArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsRuntimeGetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsRuntimeSetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsRuntimeTestIamPermissionsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ApigeeregistryProvider with automatic state tracking.
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
/// let provider = ApigeeregistryProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct ApigeeregistryProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> ApigeeregistryProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new ApigeeregistryProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new ApigeeregistryProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Apigeeregistry projects locations get.
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
    pub fn apigeeregistry_projects_locations_get(
        &self,
        args: &ApigeeregistryProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations list.
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
    pub fn apigeeregistry_projects_locations_list(
        &self,
        args: &ApigeeregistryProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Api result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_create(
        &self,
        args: &ApigeeregistryProjectsLocationsApisCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Api, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_create_builder(
            &self.http_client,
            &args.parent,
            &args.apiId,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis delete.
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
    pub fn apigeeregistry_projects_locations_apis_delete(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Api result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigeeregistry_projects_locations_apis_get(
        &self,
        args: &ApigeeregistryProjectsLocationsApisGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Api, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis get iam policy.
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
    pub fn apigeeregistry_projects_locations_apis_get_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsApisGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListApisResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigeeregistry_projects_locations_apis_list(
        &self,
        args: &ApigeeregistryProjectsLocationsApisListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListApisResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Api result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_patch(
        &self,
        args: &ApigeeregistryProjectsLocationsApisPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Api, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis set iam policy.
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
    pub fn apigeeregistry_projects_locations_apis_set_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsApisSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis test iam permissions.
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
    pub fn apigeeregistry_projects_locations_apis_test_iam_permissions(
        &self,
        args: &ApigeeregistryProjectsLocationsApisTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis artifacts create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Artifact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_artifacts_create(
        &self,
        args: &ApigeeregistryProjectsLocationsApisArtifactsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Artifact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_artifacts_create_builder(
            &self.http_client,
            &args.parent,
            &args.artifactId,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_artifacts_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis artifacts delete.
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
    pub fn apigeeregistry_projects_locations_apis_artifacts_delete(
        &self,
        args: &ApigeeregistryProjectsLocationsApisArtifactsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_artifacts_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_artifacts_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis artifacts get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Artifact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigeeregistry_projects_locations_apis_artifacts_get(
        &self,
        args: &ApigeeregistryProjectsLocationsApisArtifactsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Artifact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_artifacts_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_artifacts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis artifacts get contents.
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
    pub fn apigeeregistry_projects_locations_apis_artifacts_get_contents(
        &self,
        args: &ApigeeregistryProjectsLocationsApisArtifactsGetContentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_artifacts_get_contents_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_artifacts_get_contents_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis artifacts get iam policy.
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
    pub fn apigeeregistry_projects_locations_apis_artifacts_get_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsApisArtifactsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_artifacts_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_artifacts_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis artifacts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListArtifactsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigeeregistry_projects_locations_apis_artifacts_list(
        &self,
        args: &ApigeeregistryProjectsLocationsApisArtifactsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListArtifactsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_artifacts_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_artifacts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis artifacts replace artifact.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Artifact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_artifacts_replace_artifact(
        &self,
        args: &ApigeeregistryProjectsLocationsApisArtifactsReplaceArtifactArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Artifact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_artifacts_replace_artifact_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_artifacts_replace_artifact_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis artifacts set iam policy.
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
    pub fn apigeeregistry_projects_locations_apis_artifacts_set_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsApisArtifactsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_artifacts_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_artifacts_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis artifacts test iam permissions.
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
    pub fn apigeeregistry_projects_locations_apis_artifacts_test_iam_permissions(
        &self,
        args: &ApigeeregistryProjectsLocationsApisArtifactsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_artifacts_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_artifacts_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis deployments create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiDeployment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_deployments_create(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeploymentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiDeployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_deployments_create_builder(
            &self.http_client,
            &args.parent,
            &args.apiDeploymentId,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_deployments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis deployments delete.
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
    pub fn apigeeregistry_projects_locations_apis_deployments_delete(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeploymentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_deployments_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_deployments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis deployments delete revision.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiDeployment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_deployments_delete_revision(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeploymentsDeleteRevisionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiDeployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_deployments_delete_revision_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_deployments_delete_revision_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis deployments get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiDeployment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigeeregistry_projects_locations_apis_deployments_get(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeploymentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiDeployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_deployments_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_deployments_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis deployments get iam policy.
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
    pub fn apigeeregistry_projects_locations_apis_deployments_get_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeploymentsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_deployments_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_deployments_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis deployments list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListApiDeploymentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigeeregistry_projects_locations_apis_deployments_list(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeploymentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListApiDeploymentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_deployments_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_deployments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis deployments list revisions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListApiDeploymentRevisionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigeeregistry_projects_locations_apis_deployments_list_revisions(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeploymentsListRevisionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListApiDeploymentRevisionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_deployments_list_revisions_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_deployments_list_revisions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis deployments patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiDeployment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_deployments_patch(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeploymentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiDeployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_deployments_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_deployments_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis deployments rollback.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiDeployment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_deployments_rollback(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeploymentsRollbackArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiDeployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_deployments_rollback_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_deployments_rollback_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis deployments set iam policy.
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
    pub fn apigeeregistry_projects_locations_apis_deployments_set_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeploymentsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_deployments_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_deployments_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis deployments tag revision.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiDeployment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_deployments_tag_revision(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeploymentsTagRevisionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiDeployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_deployments_tag_revision_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_deployments_tag_revision_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis deployments test iam permissions.
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
    pub fn apigeeregistry_projects_locations_apis_deployments_test_iam_permissions(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeploymentsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_deployments_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_deployments_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis deployments artifacts create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Artifact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_deployments_artifacts_create(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeploymentsArtifactsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Artifact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_deployments_artifacts_create_builder(
            &self.http_client,
            &args.parent,
            &args.artifactId,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_deployments_artifacts_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis deployments artifacts delete.
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
    pub fn apigeeregistry_projects_locations_apis_deployments_artifacts_delete(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeploymentsArtifactsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_deployments_artifacts_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_deployments_artifacts_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis deployments artifacts get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Artifact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigeeregistry_projects_locations_apis_deployments_artifacts_get(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeploymentsArtifactsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Artifact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_deployments_artifacts_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_deployments_artifacts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis deployments artifacts get contents.
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
    pub fn apigeeregistry_projects_locations_apis_deployments_artifacts_get_contents(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeploymentsArtifactsGetContentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_deployments_artifacts_get_contents_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_deployments_artifacts_get_contents_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis deployments artifacts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListArtifactsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigeeregistry_projects_locations_apis_deployments_artifacts_list(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeploymentsArtifactsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListArtifactsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_deployments_artifacts_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_deployments_artifacts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis deployments artifacts replace artifact.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Artifact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_deployments_artifacts_replace_artifact(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeploymentsArtifactsReplaceArtifactArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Artifact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_deployments_artifacts_replace_artifact_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_deployments_artifacts_replace_artifact_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_versions_create(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_create_builder(
            &self.http_client,
            &args.parent,
            &args.apiVersionId,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions delete.
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
    pub fn apigeeregistry_projects_locations_apis_versions_delete(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigeeregistry_projects_locations_apis_versions_get(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions get iam policy.
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
    pub fn apigeeregistry_projects_locations_apis_versions_get_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListApiVersionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigeeregistry_projects_locations_apis_versions_list(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListApiVersionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_versions_patch(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions set iam policy.
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
    pub fn apigeeregistry_projects_locations_apis_versions_set_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions test iam permissions.
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
    pub fn apigeeregistry_projects_locations_apis_versions_test_iam_permissions(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions artifacts create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Artifact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_versions_artifacts_create(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsArtifactsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Artifact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_artifacts_create_builder(
            &self.http_client,
            &args.parent,
            &args.artifactId,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_artifacts_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions artifacts delete.
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
    pub fn apigeeregistry_projects_locations_apis_versions_artifacts_delete(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsArtifactsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_artifacts_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_artifacts_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions artifacts get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Artifact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigeeregistry_projects_locations_apis_versions_artifacts_get(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsArtifactsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Artifact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_artifacts_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_artifacts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions artifacts get contents.
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
    pub fn apigeeregistry_projects_locations_apis_versions_artifacts_get_contents(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsArtifactsGetContentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_artifacts_get_contents_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_artifacts_get_contents_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions artifacts get iam policy.
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
    pub fn apigeeregistry_projects_locations_apis_versions_artifacts_get_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsArtifactsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_artifacts_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_artifacts_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions artifacts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListArtifactsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigeeregistry_projects_locations_apis_versions_artifacts_list(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsArtifactsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListArtifactsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_artifacts_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_artifacts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions artifacts replace artifact.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Artifact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_versions_artifacts_replace_artifact(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsArtifactsReplaceArtifactArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Artifact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_artifacts_replace_artifact_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_artifacts_replace_artifact_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions artifacts set iam policy.
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
    pub fn apigeeregistry_projects_locations_apis_versions_artifacts_set_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsArtifactsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_artifacts_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_artifacts_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions artifacts test iam permissions.
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
    pub fn apigeeregistry_projects_locations_apis_versions_artifacts_test_iam_permissions(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsArtifactsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_artifacts_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_artifacts_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiSpec result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_versions_specs_create(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiSpec, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_create_builder(
            &self.http_client,
            &args.parent,
            &args.apiSpecId,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs delete.
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
    pub fn apigeeregistry_projects_locations_apis_versions_specs_delete(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs delete revision.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiSpec result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_versions_specs_delete_revision(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsDeleteRevisionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiSpec, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_delete_revision_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_delete_revision_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiSpec result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigeeregistry_projects_locations_apis_versions_specs_get(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiSpec, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs get contents.
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
    pub fn apigeeregistry_projects_locations_apis_versions_specs_get_contents(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsGetContentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_get_contents_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_get_contents_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs get iam policy.
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
    pub fn apigeeregistry_projects_locations_apis_versions_specs_get_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListApiSpecsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigeeregistry_projects_locations_apis_versions_specs_list(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListApiSpecsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs list revisions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListApiSpecRevisionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigeeregistry_projects_locations_apis_versions_specs_list_revisions(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsListRevisionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListApiSpecRevisionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_list_revisions_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_list_revisions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiSpec result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_versions_specs_patch(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiSpec, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs rollback.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiSpec result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_versions_specs_rollback(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsRollbackArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiSpec, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_rollback_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_rollback_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs set iam policy.
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
    pub fn apigeeregistry_projects_locations_apis_versions_specs_set_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs tag revision.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiSpec result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_versions_specs_tag_revision(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsTagRevisionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiSpec, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_tag_revision_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_tag_revision_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs test iam permissions.
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
    pub fn apigeeregistry_projects_locations_apis_versions_specs_test_iam_permissions(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs artifacts create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Artifact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_versions_specs_artifacts_create(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsArtifactsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Artifact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_artifacts_create_builder(
            &self.http_client,
            &args.parent,
            &args.artifactId,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_artifacts_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs artifacts delete.
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
    pub fn apigeeregistry_projects_locations_apis_versions_specs_artifacts_delete(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsArtifactsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_artifacts_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_artifacts_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs artifacts get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Artifact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigeeregistry_projects_locations_apis_versions_specs_artifacts_get(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsArtifactsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Artifact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_artifacts_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_artifacts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs artifacts get contents.
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
    pub fn apigeeregistry_projects_locations_apis_versions_specs_artifacts_get_contents(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsArtifactsGetContentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_artifacts_get_contents_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_artifacts_get_contents_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs artifacts get iam policy.
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
    pub fn apigeeregistry_projects_locations_apis_versions_specs_artifacts_get_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsArtifactsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_artifacts_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_artifacts_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs artifacts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListArtifactsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigeeregistry_projects_locations_apis_versions_specs_artifacts_list(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsArtifactsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListArtifactsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_artifacts_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_artifacts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs artifacts replace artifact.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Artifact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_versions_specs_artifacts_replace_artifact(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsArtifactsReplaceArtifactArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Artifact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_artifacts_replace_artifact_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_artifacts_replace_artifact_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs artifacts set iam policy.
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
    pub fn apigeeregistry_projects_locations_apis_versions_specs_artifacts_set_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsArtifactsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_artifacts_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_artifacts_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs artifacts test iam permissions.
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
    pub fn apigeeregistry_projects_locations_apis_versions_specs_artifacts_test_iam_permissions(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsArtifactsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_artifacts_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_artifacts_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations artifacts create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Artifact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_artifacts_create(
        &self,
        args: &ApigeeregistryProjectsLocationsArtifactsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Artifact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_artifacts_create_builder(
            &self.http_client,
            &args.parent,
            &args.artifactId,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_artifacts_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations artifacts delete.
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
    pub fn apigeeregistry_projects_locations_artifacts_delete(
        &self,
        args: &ApigeeregistryProjectsLocationsArtifactsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_artifacts_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_artifacts_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations artifacts get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Artifact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigeeregistry_projects_locations_artifacts_get(
        &self,
        args: &ApigeeregistryProjectsLocationsArtifactsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Artifact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_artifacts_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_artifacts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations artifacts get contents.
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
    pub fn apigeeregistry_projects_locations_artifacts_get_contents(
        &self,
        args: &ApigeeregistryProjectsLocationsArtifactsGetContentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_artifacts_get_contents_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_artifacts_get_contents_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations artifacts get iam policy.
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
    pub fn apigeeregistry_projects_locations_artifacts_get_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsArtifactsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_artifacts_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_artifacts_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations artifacts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListArtifactsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigeeregistry_projects_locations_artifacts_list(
        &self,
        args: &ApigeeregistryProjectsLocationsArtifactsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListArtifactsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_artifacts_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_artifacts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations artifacts replace artifact.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Artifact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_artifacts_replace_artifact(
        &self,
        args: &ApigeeregistryProjectsLocationsArtifactsReplaceArtifactArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Artifact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_artifacts_replace_artifact_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_artifacts_replace_artifact_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations artifacts set iam policy.
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
    pub fn apigeeregistry_projects_locations_artifacts_set_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsArtifactsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_artifacts_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_artifacts_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations artifacts test iam permissions.
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
    pub fn apigeeregistry_projects_locations_artifacts_test_iam_permissions(
        &self,
        args: &ApigeeregistryProjectsLocationsArtifactsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_artifacts_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_artifacts_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations documents get iam policy.
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
    pub fn apigeeregistry_projects_locations_documents_get_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsDocumentsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_documents_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_documents_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations documents set iam policy.
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
    pub fn apigeeregistry_projects_locations_documents_set_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsDocumentsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_documents_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_documents_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations documents test iam permissions.
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
    pub fn apigeeregistry_projects_locations_documents_test_iam_permissions(
        &self,
        args: &ApigeeregistryProjectsLocationsDocumentsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_documents_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_documents_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations instances create.
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
    pub fn apigeeregistry_projects_locations_instances_create(
        &self,
        args: &ApigeeregistryProjectsLocationsInstancesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_instances_create_builder(
            &self.http_client,
            &args.parent,
            &args.instanceId,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_instances_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations instances delete.
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
    pub fn apigeeregistry_projects_locations_instances_delete(
        &self,
        args: &ApigeeregistryProjectsLocationsInstancesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_instances_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_instances_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations instances get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Instance result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigeeregistry_projects_locations_instances_get(
        &self,
        args: &ApigeeregistryProjectsLocationsInstancesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Instance, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_instances_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_instances_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations instances get iam policy.
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
    pub fn apigeeregistry_projects_locations_instances_get_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsInstancesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_instances_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_instances_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations instances set iam policy.
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
    pub fn apigeeregistry_projects_locations_instances_set_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsInstancesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_instances_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_instances_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations instances test iam permissions.
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
    pub fn apigeeregistry_projects_locations_instances_test_iam_permissions(
        &self,
        args: &ApigeeregistryProjectsLocationsInstancesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_instances_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_instances_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations operations cancel.
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
    pub fn apigeeregistry_projects_locations_operations_cancel(
        &self,
        args: &ApigeeregistryProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations operations delete.
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
    pub fn apigeeregistry_projects_locations_operations_delete(
        &self,
        args: &ApigeeregistryProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations operations get.
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
    pub fn apigeeregistry_projects_locations_operations_get(
        &self,
        args: &ApigeeregistryProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations operations list.
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
    pub fn apigeeregistry_projects_locations_operations_list(
        &self,
        args: &ApigeeregistryProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations runtime get iam policy.
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
    pub fn apigeeregistry_projects_locations_runtime_get_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsRuntimeGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_runtime_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_runtime_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations runtime set iam policy.
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
    pub fn apigeeregistry_projects_locations_runtime_set_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsRuntimeSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_runtime_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_runtime_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations runtime test iam permissions.
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
    pub fn apigeeregistry_projects_locations_runtime_test_iam_permissions(
        &self,
        args: &ApigeeregistryProjectsLocationsRuntimeTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_runtime_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_runtime_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
