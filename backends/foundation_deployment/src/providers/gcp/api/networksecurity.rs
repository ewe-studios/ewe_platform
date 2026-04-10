//! NetworksecurityProvider - State-aware networksecurity API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       networksecurity API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::networksecurity::{
    networksecurity_organizations_locations_address_groups_add_items_builder, networksecurity_organizations_locations_address_groups_add_items_task,
    networksecurity_organizations_locations_address_groups_clone_items_builder, networksecurity_organizations_locations_address_groups_clone_items_task,
    networksecurity_organizations_locations_address_groups_create_builder, networksecurity_organizations_locations_address_groups_create_task,
    networksecurity_organizations_locations_address_groups_delete_builder, networksecurity_organizations_locations_address_groups_delete_task,
    networksecurity_organizations_locations_address_groups_patch_builder, networksecurity_organizations_locations_address_groups_patch_task,
    networksecurity_organizations_locations_address_groups_remove_items_builder, networksecurity_organizations_locations_address_groups_remove_items_task,
    networksecurity_organizations_locations_firewall_endpoints_create_builder, networksecurity_organizations_locations_firewall_endpoints_create_task,
    networksecurity_organizations_locations_firewall_endpoints_delete_builder, networksecurity_organizations_locations_firewall_endpoints_delete_task,
    networksecurity_organizations_locations_firewall_endpoints_patch_builder, networksecurity_organizations_locations_firewall_endpoints_patch_task,
    networksecurity_organizations_locations_operations_cancel_builder, networksecurity_organizations_locations_operations_cancel_task,
    networksecurity_organizations_locations_operations_delete_builder, networksecurity_organizations_locations_operations_delete_task,
    networksecurity_organizations_locations_security_profile_groups_create_builder, networksecurity_organizations_locations_security_profile_groups_create_task,
    networksecurity_organizations_locations_security_profile_groups_delete_builder, networksecurity_organizations_locations_security_profile_groups_delete_task,
    networksecurity_organizations_locations_security_profile_groups_patch_builder, networksecurity_organizations_locations_security_profile_groups_patch_task,
    networksecurity_organizations_locations_security_profiles_create_builder, networksecurity_organizations_locations_security_profiles_create_task,
    networksecurity_organizations_locations_security_profiles_delete_builder, networksecurity_organizations_locations_security_profiles_delete_task,
    networksecurity_organizations_locations_security_profiles_patch_builder, networksecurity_organizations_locations_security_profiles_patch_task,
    networksecurity_projects_locations_address_groups_add_items_builder, networksecurity_projects_locations_address_groups_add_items_task,
    networksecurity_projects_locations_address_groups_clone_items_builder, networksecurity_projects_locations_address_groups_clone_items_task,
    networksecurity_projects_locations_address_groups_create_builder, networksecurity_projects_locations_address_groups_create_task,
    networksecurity_projects_locations_address_groups_delete_builder, networksecurity_projects_locations_address_groups_delete_task,
    networksecurity_projects_locations_address_groups_patch_builder, networksecurity_projects_locations_address_groups_patch_task,
    networksecurity_projects_locations_address_groups_remove_items_builder, networksecurity_projects_locations_address_groups_remove_items_task,
    networksecurity_projects_locations_address_groups_set_iam_policy_builder, networksecurity_projects_locations_address_groups_set_iam_policy_task,
    networksecurity_projects_locations_address_groups_test_iam_permissions_builder, networksecurity_projects_locations_address_groups_test_iam_permissions_task,
    networksecurity_projects_locations_authorization_policies_create_builder, networksecurity_projects_locations_authorization_policies_create_task,
    networksecurity_projects_locations_authorization_policies_delete_builder, networksecurity_projects_locations_authorization_policies_delete_task,
    networksecurity_projects_locations_authorization_policies_patch_builder, networksecurity_projects_locations_authorization_policies_patch_task,
    networksecurity_projects_locations_authorization_policies_set_iam_policy_builder, networksecurity_projects_locations_authorization_policies_set_iam_policy_task,
    networksecurity_projects_locations_authorization_policies_test_iam_permissions_builder, networksecurity_projects_locations_authorization_policies_test_iam_permissions_task,
    networksecurity_projects_locations_authz_policies_create_builder, networksecurity_projects_locations_authz_policies_create_task,
    networksecurity_projects_locations_authz_policies_delete_builder, networksecurity_projects_locations_authz_policies_delete_task,
    networksecurity_projects_locations_authz_policies_patch_builder, networksecurity_projects_locations_authz_policies_patch_task,
    networksecurity_projects_locations_authz_policies_set_iam_policy_builder, networksecurity_projects_locations_authz_policies_set_iam_policy_task,
    networksecurity_projects_locations_authz_policies_test_iam_permissions_builder, networksecurity_projects_locations_authz_policies_test_iam_permissions_task,
    networksecurity_projects_locations_backend_authentication_configs_create_builder, networksecurity_projects_locations_backend_authentication_configs_create_task,
    networksecurity_projects_locations_backend_authentication_configs_delete_builder, networksecurity_projects_locations_backend_authentication_configs_delete_task,
    networksecurity_projects_locations_backend_authentication_configs_patch_builder, networksecurity_projects_locations_backend_authentication_configs_patch_task,
    networksecurity_projects_locations_client_tls_policies_create_builder, networksecurity_projects_locations_client_tls_policies_create_task,
    networksecurity_projects_locations_client_tls_policies_delete_builder, networksecurity_projects_locations_client_tls_policies_delete_task,
    networksecurity_projects_locations_client_tls_policies_patch_builder, networksecurity_projects_locations_client_tls_policies_patch_task,
    networksecurity_projects_locations_client_tls_policies_set_iam_policy_builder, networksecurity_projects_locations_client_tls_policies_set_iam_policy_task,
    networksecurity_projects_locations_client_tls_policies_test_iam_permissions_builder, networksecurity_projects_locations_client_tls_policies_test_iam_permissions_task,
    networksecurity_projects_locations_dns_threat_detectors_create_builder, networksecurity_projects_locations_dns_threat_detectors_create_task,
    networksecurity_projects_locations_dns_threat_detectors_delete_builder, networksecurity_projects_locations_dns_threat_detectors_delete_task,
    networksecurity_projects_locations_dns_threat_detectors_patch_builder, networksecurity_projects_locations_dns_threat_detectors_patch_task,
    networksecurity_projects_locations_firewall_endpoint_associations_create_builder, networksecurity_projects_locations_firewall_endpoint_associations_create_task,
    networksecurity_projects_locations_firewall_endpoint_associations_delete_builder, networksecurity_projects_locations_firewall_endpoint_associations_delete_task,
    networksecurity_projects_locations_firewall_endpoint_associations_patch_builder, networksecurity_projects_locations_firewall_endpoint_associations_patch_task,
    networksecurity_projects_locations_gateway_security_policies_create_builder, networksecurity_projects_locations_gateway_security_policies_create_task,
    networksecurity_projects_locations_gateway_security_policies_delete_builder, networksecurity_projects_locations_gateway_security_policies_delete_task,
    networksecurity_projects_locations_gateway_security_policies_patch_builder, networksecurity_projects_locations_gateway_security_policies_patch_task,
    networksecurity_projects_locations_gateway_security_policies_rules_create_builder, networksecurity_projects_locations_gateway_security_policies_rules_create_task,
    networksecurity_projects_locations_gateway_security_policies_rules_delete_builder, networksecurity_projects_locations_gateway_security_policies_rules_delete_task,
    networksecurity_projects_locations_gateway_security_policies_rules_patch_builder, networksecurity_projects_locations_gateway_security_policies_rules_patch_task,
    networksecurity_projects_locations_intercept_deployment_groups_create_builder, networksecurity_projects_locations_intercept_deployment_groups_create_task,
    networksecurity_projects_locations_intercept_deployment_groups_delete_builder, networksecurity_projects_locations_intercept_deployment_groups_delete_task,
    networksecurity_projects_locations_intercept_deployment_groups_patch_builder, networksecurity_projects_locations_intercept_deployment_groups_patch_task,
    networksecurity_projects_locations_intercept_deployments_create_builder, networksecurity_projects_locations_intercept_deployments_create_task,
    networksecurity_projects_locations_intercept_deployments_delete_builder, networksecurity_projects_locations_intercept_deployments_delete_task,
    networksecurity_projects_locations_intercept_deployments_patch_builder, networksecurity_projects_locations_intercept_deployments_patch_task,
    networksecurity_projects_locations_intercept_endpoint_group_associations_create_builder, networksecurity_projects_locations_intercept_endpoint_group_associations_create_task,
    networksecurity_projects_locations_intercept_endpoint_group_associations_delete_builder, networksecurity_projects_locations_intercept_endpoint_group_associations_delete_task,
    networksecurity_projects_locations_intercept_endpoint_group_associations_patch_builder, networksecurity_projects_locations_intercept_endpoint_group_associations_patch_task,
    networksecurity_projects_locations_intercept_endpoint_groups_create_builder, networksecurity_projects_locations_intercept_endpoint_groups_create_task,
    networksecurity_projects_locations_intercept_endpoint_groups_delete_builder, networksecurity_projects_locations_intercept_endpoint_groups_delete_task,
    networksecurity_projects_locations_intercept_endpoint_groups_patch_builder, networksecurity_projects_locations_intercept_endpoint_groups_patch_task,
    networksecurity_projects_locations_mirroring_deployment_groups_create_builder, networksecurity_projects_locations_mirroring_deployment_groups_create_task,
    networksecurity_projects_locations_mirroring_deployment_groups_delete_builder, networksecurity_projects_locations_mirroring_deployment_groups_delete_task,
    networksecurity_projects_locations_mirroring_deployment_groups_patch_builder, networksecurity_projects_locations_mirroring_deployment_groups_patch_task,
    networksecurity_projects_locations_mirroring_deployments_create_builder, networksecurity_projects_locations_mirroring_deployments_create_task,
    networksecurity_projects_locations_mirroring_deployments_delete_builder, networksecurity_projects_locations_mirroring_deployments_delete_task,
    networksecurity_projects_locations_mirroring_deployments_patch_builder, networksecurity_projects_locations_mirroring_deployments_patch_task,
    networksecurity_projects_locations_mirroring_endpoint_group_associations_create_builder, networksecurity_projects_locations_mirroring_endpoint_group_associations_create_task,
    networksecurity_projects_locations_mirroring_endpoint_group_associations_delete_builder, networksecurity_projects_locations_mirroring_endpoint_group_associations_delete_task,
    networksecurity_projects_locations_mirroring_endpoint_group_associations_patch_builder, networksecurity_projects_locations_mirroring_endpoint_group_associations_patch_task,
    networksecurity_projects_locations_mirroring_endpoint_groups_create_builder, networksecurity_projects_locations_mirroring_endpoint_groups_create_task,
    networksecurity_projects_locations_mirroring_endpoint_groups_delete_builder, networksecurity_projects_locations_mirroring_endpoint_groups_delete_task,
    networksecurity_projects_locations_mirroring_endpoint_groups_patch_builder, networksecurity_projects_locations_mirroring_endpoint_groups_patch_task,
    networksecurity_projects_locations_operations_cancel_builder, networksecurity_projects_locations_operations_cancel_task,
    networksecurity_projects_locations_operations_delete_builder, networksecurity_projects_locations_operations_delete_task,
    networksecurity_projects_locations_server_tls_policies_create_builder, networksecurity_projects_locations_server_tls_policies_create_task,
    networksecurity_projects_locations_server_tls_policies_delete_builder, networksecurity_projects_locations_server_tls_policies_delete_task,
    networksecurity_projects_locations_server_tls_policies_patch_builder, networksecurity_projects_locations_server_tls_policies_patch_task,
    networksecurity_projects_locations_server_tls_policies_set_iam_policy_builder, networksecurity_projects_locations_server_tls_policies_set_iam_policy_task,
    networksecurity_projects_locations_server_tls_policies_test_iam_permissions_builder, networksecurity_projects_locations_server_tls_policies_test_iam_permissions_task,
    networksecurity_projects_locations_tls_inspection_policies_create_builder, networksecurity_projects_locations_tls_inspection_policies_create_task,
    networksecurity_projects_locations_tls_inspection_policies_delete_builder, networksecurity_projects_locations_tls_inspection_policies_delete_task,
    networksecurity_projects_locations_tls_inspection_policies_patch_builder, networksecurity_projects_locations_tls_inspection_policies_patch_task,
    networksecurity_projects_locations_url_lists_create_builder, networksecurity_projects_locations_url_lists_create_task,
    networksecurity_projects_locations_url_lists_delete_builder, networksecurity_projects_locations_url_lists_delete_task,
    networksecurity_projects_locations_url_lists_patch_builder, networksecurity_projects_locations_url_lists_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::networksecurity::DnsThreatDetector;
use crate::providers::gcp::clients::networksecurity::Empty;
use crate::providers::gcp::clients::networksecurity::GoogleIamV1Policy;
use crate::providers::gcp::clients::networksecurity::GoogleIamV1TestIamPermissionsResponse;
use crate::providers::gcp::clients::networksecurity::Operation;
use crate::providers::gcp::clients::networksecurity::NetworksecurityOrganizationsLocationsAddressGroupsAddItemsArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityOrganizationsLocationsAddressGroupsCloneItemsArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityOrganizationsLocationsAddressGroupsCreateArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityOrganizationsLocationsAddressGroupsDeleteArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityOrganizationsLocationsAddressGroupsPatchArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityOrganizationsLocationsAddressGroupsRemoveItemsArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityOrganizationsLocationsFirewallEndpointsCreateArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityOrganizationsLocationsFirewallEndpointsDeleteArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityOrganizationsLocationsFirewallEndpointsPatchArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityOrganizationsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityOrganizationsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityOrganizationsLocationsSecurityProfileGroupsCreateArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityOrganizationsLocationsSecurityProfileGroupsDeleteArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityOrganizationsLocationsSecurityProfileGroupsPatchArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityOrganizationsLocationsSecurityProfilesCreateArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityOrganizationsLocationsSecurityProfilesDeleteArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityOrganizationsLocationsSecurityProfilesPatchArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsAddressGroupsAddItemsArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsAddressGroupsCloneItemsArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsAddressGroupsCreateArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsAddressGroupsDeleteArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsAddressGroupsPatchArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsAddressGroupsRemoveItemsArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsAddressGroupsSetIamPolicyArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsAddressGroupsTestIamPermissionsArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsAuthorizationPoliciesCreateArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsAuthorizationPoliciesDeleteArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsAuthorizationPoliciesPatchArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsAuthorizationPoliciesSetIamPolicyArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsAuthorizationPoliciesTestIamPermissionsArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsAuthzPoliciesCreateArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsAuthzPoliciesDeleteArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsAuthzPoliciesPatchArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsAuthzPoliciesSetIamPolicyArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsAuthzPoliciesTestIamPermissionsArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsBackendAuthenticationConfigsCreateArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsBackendAuthenticationConfigsDeleteArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsBackendAuthenticationConfigsPatchArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsClientTlsPoliciesCreateArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsClientTlsPoliciesDeleteArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsClientTlsPoliciesPatchArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsClientTlsPoliciesSetIamPolicyArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsClientTlsPoliciesTestIamPermissionsArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsDnsThreatDetectorsCreateArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsDnsThreatDetectorsDeleteArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsDnsThreatDetectorsPatchArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsFirewallEndpointAssociationsCreateArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsFirewallEndpointAssociationsDeleteArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsFirewallEndpointAssociationsPatchArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsGatewaySecurityPoliciesCreateArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsGatewaySecurityPoliciesDeleteArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsGatewaySecurityPoliciesPatchArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsGatewaySecurityPoliciesRulesCreateArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsGatewaySecurityPoliciesRulesDeleteArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsGatewaySecurityPoliciesRulesPatchArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsInterceptDeploymentGroupsCreateArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsInterceptDeploymentGroupsDeleteArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsInterceptDeploymentGroupsPatchArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsInterceptDeploymentsCreateArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsInterceptDeploymentsDeleteArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsInterceptDeploymentsPatchArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsInterceptEndpointGroupAssociationsCreateArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsInterceptEndpointGroupAssociationsDeleteArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsInterceptEndpointGroupAssociationsPatchArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsInterceptEndpointGroupsCreateArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsInterceptEndpointGroupsDeleteArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsInterceptEndpointGroupsPatchArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsMirroringDeploymentGroupsCreateArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsMirroringDeploymentGroupsDeleteArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsMirroringDeploymentGroupsPatchArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsMirroringDeploymentsCreateArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsMirroringDeploymentsDeleteArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsMirroringDeploymentsPatchArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsMirroringEndpointGroupAssociationsCreateArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsMirroringEndpointGroupAssociationsDeleteArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsMirroringEndpointGroupAssociationsPatchArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsMirroringEndpointGroupsCreateArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsMirroringEndpointGroupsDeleteArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsMirroringEndpointGroupsPatchArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsServerTlsPoliciesCreateArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsServerTlsPoliciesDeleteArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsServerTlsPoliciesPatchArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsServerTlsPoliciesSetIamPolicyArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsServerTlsPoliciesTestIamPermissionsArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsTlsInspectionPoliciesCreateArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsTlsInspectionPoliciesDeleteArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsTlsInspectionPoliciesPatchArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsUrlListsCreateArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsUrlListsDeleteArgs;
use crate::providers::gcp::clients::networksecurity::NetworksecurityProjectsLocationsUrlListsPatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// NetworksecurityProvider with automatic state tracking.
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
/// let provider = NetworksecurityProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct NetworksecurityProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> NetworksecurityProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new NetworksecurityProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Networksecurity organizations locations address groups add items.
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
    pub fn networksecurity_organizations_locations_address_groups_add_items(
        &self,
        args: &NetworksecurityOrganizationsLocationsAddressGroupsAddItemsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_organizations_locations_address_groups_add_items_builder(
            &self.http_client,
            &args.addressGroup,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_organizations_locations_address_groups_add_items_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity organizations locations address groups clone items.
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
    pub fn networksecurity_organizations_locations_address_groups_clone_items(
        &self,
        args: &NetworksecurityOrganizationsLocationsAddressGroupsCloneItemsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_organizations_locations_address_groups_clone_items_builder(
            &self.http_client,
            &args.addressGroup,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_organizations_locations_address_groups_clone_items_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity organizations locations address groups create.
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
    pub fn networksecurity_organizations_locations_address_groups_create(
        &self,
        args: &NetworksecurityOrganizationsLocationsAddressGroupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_organizations_locations_address_groups_create_builder(
            &self.http_client,
            &args.parent,
            &args.addressGroupId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_organizations_locations_address_groups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity organizations locations address groups delete.
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
    pub fn networksecurity_organizations_locations_address_groups_delete(
        &self,
        args: &NetworksecurityOrganizationsLocationsAddressGroupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_organizations_locations_address_groups_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_organizations_locations_address_groups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity organizations locations address groups patch.
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
    pub fn networksecurity_organizations_locations_address_groups_patch(
        &self,
        args: &NetworksecurityOrganizationsLocationsAddressGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_organizations_locations_address_groups_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_organizations_locations_address_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity organizations locations address groups remove items.
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
    pub fn networksecurity_organizations_locations_address_groups_remove_items(
        &self,
        args: &NetworksecurityOrganizationsLocationsAddressGroupsRemoveItemsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_organizations_locations_address_groups_remove_items_builder(
            &self.http_client,
            &args.addressGroup,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_organizations_locations_address_groups_remove_items_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity organizations locations firewall endpoints create.
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
    pub fn networksecurity_organizations_locations_firewall_endpoints_create(
        &self,
        args: &NetworksecurityOrganizationsLocationsFirewallEndpointsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_organizations_locations_firewall_endpoints_create_builder(
            &self.http_client,
            &args.parent,
            &args.firewallEndpointId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_organizations_locations_firewall_endpoints_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity organizations locations firewall endpoints delete.
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
    pub fn networksecurity_organizations_locations_firewall_endpoints_delete(
        &self,
        args: &NetworksecurityOrganizationsLocationsFirewallEndpointsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_organizations_locations_firewall_endpoints_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_organizations_locations_firewall_endpoints_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity organizations locations firewall endpoints patch.
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
    pub fn networksecurity_organizations_locations_firewall_endpoints_patch(
        &self,
        args: &NetworksecurityOrganizationsLocationsFirewallEndpointsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_organizations_locations_firewall_endpoints_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_organizations_locations_firewall_endpoints_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity organizations locations operations cancel.
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
    pub fn networksecurity_organizations_locations_operations_cancel(
        &self,
        args: &NetworksecurityOrganizationsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_organizations_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_organizations_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity organizations locations operations delete.
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
    pub fn networksecurity_organizations_locations_operations_delete(
        &self,
        args: &NetworksecurityOrganizationsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_organizations_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_organizations_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity organizations locations security profile groups create.
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
    pub fn networksecurity_organizations_locations_security_profile_groups_create(
        &self,
        args: &NetworksecurityOrganizationsLocationsSecurityProfileGroupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_organizations_locations_security_profile_groups_create_builder(
            &self.http_client,
            &args.parent,
            &args.securityProfileGroupId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_organizations_locations_security_profile_groups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity organizations locations security profile groups delete.
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
    pub fn networksecurity_organizations_locations_security_profile_groups_delete(
        &self,
        args: &NetworksecurityOrganizationsLocationsSecurityProfileGroupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_organizations_locations_security_profile_groups_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_organizations_locations_security_profile_groups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity organizations locations security profile groups patch.
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
    pub fn networksecurity_organizations_locations_security_profile_groups_patch(
        &self,
        args: &NetworksecurityOrganizationsLocationsSecurityProfileGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_organizations_locations_security_profile_groups_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_organizations_locations_security_profile_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity organizations locations security profiles create.
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
    pub fn networksecurity_organizations_locations_security_profiles_create(
        &self,
        args: &NetworksecurityOrganizationsLocationsSecurityProfilesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_organizations_locations_security_profiles_create_builder(
            &self.http_client,
            &args.parent,
            &args.securityProfileId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_organizations_locations_security_profiles_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity organizations locations security profiles delete.
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
    pub fn networksecurity_organizations_locations_security_profiles_delete(
        &self,
        args: &NetworksecurityOrganizationsLocationsSecurityProfilesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_organizations_locations_security_profiles_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_organizations_locations_security_profiles_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity organizations locations security profiles patch.
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
    pub fn networksecurity_organizations_locations_security_profiles_patch(
        &self,
        args: &NetworksecurityOrganizationsLocationsSecurityProfilesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_organizations_locations_security_profiles_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_organizations_locations_security_profiles_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations address groups add items.
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
    pub fn networksecurity_projects_locations_address_groups_add_items(
        &self,
        args: &NetworksecurityProjectsLocationsAddressGroupsAddItemsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_address_groups_add_items_builder(
            &self.http_client,
            &args.addressGroup,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_address_groups_add_items_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations address groups clone items.
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
    pub fn networksecurity_projects_locations_address_groups_clone_items(
        &self,
        args: &NetworksecurityProjectsLocationsAddressGroupsCloneItemsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_address_groups_clone_items_builder(
            &self.http_client,
            &args.addressGroup,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_address_groups_clone_items_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations address groups create.
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
    pub fn networksecurity_projects_locations_address_groups_create(
        &self,
        args: &NetworksecurityProjectsLocationsAddressGroupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_address_groups_create_builder(
            &self.http_client,
            &args.parent,
            &args.addressGroupId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_address_groups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations address groups delete.
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
    pub fn networksecurity_projects_locations_address_groups_delete(
        &self,
        args: &NetworksecurityProjectsLocationsAddressGroupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_address_groups_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_address_groups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations address groups patch.
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
    pub fn networksecurity_projects_locations_address_groups_patch(
        &self,
        args: &NetworksecurityProjectsLocationsAddressGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_address_groups_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_address_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations address groups remove items.
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
    pub fn networksecurity_projects_locations_address_groups_remove_items(
        &self,
        args: &NetworksecurityProjectsLocationsAddressGroupsRemoveItemsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_address_groups_remove_items_builder(
            &self.http_client,
            &args.addressGroup,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_address_groups_remove_items_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations address groups set iam policy.
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
    pub fn networksecurity_projects_locations_address_groups_set_iam_policy(
        &self,
        args: &NetworksecurityProjectsLocationsAddressGroupsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_address_groups_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_address_groups_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations address groups test iam permissions.
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
    pub fn networksecurity_projects_locations_address_groups_test_iam_permissions(
        &self,
        args: &NetworksecurityProjectsLocationsAddressGroupsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_address_groups_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_address_groups_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations authorization policies create.
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
    pub fn networksecurity_projects_locations_authorization_policies_create(
        &self,
        args: &NetworksecurityProjectsLocationsAuthorizationPoliciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_authorization_policies_create_builder(
            &self.http_client,
            &args.parent,
            &args.authorizationPolicyId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_authorization_policies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations authorization policies delete.
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
    pub fn networksecurity_projects_locations_authorization_policies_delete(
        &self,
        args: &NetworksecurityProjectsLocationsAuthorizationPoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_authorization_policies_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_authorization_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations authorization policies patch.
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
    pub fn networksecurity_projects_locations_authorization_policies_patch(
        &self,
        args: &NetworksecurityProjectsLocationsAuthorizationPoliciesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_authorization_policies_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_authorization_policies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations authorization policies set iam policy.
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
    pub fn networksecurity_projects_locations_authorization_policies_set_iam_policy(
        &self,
        args: &NetworksecurityProjectsLocationsAuthorizationPoliciesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_authorization_policies_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_authorization_policies_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations authorization policies test iam permissions.
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
    pub fn networksecurity_projects_locations_authorization_policies_test_iam_permissions(
        &self,
        args: &NetworksecurityProjectsLocationsAuthorizationPoliciesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_authorization_policies_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_authorization_policies_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations authz policies create.
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
    pub fn networksecurity_projects_locations_authz_policies_create(
        &self,
        args: &NetworksecurityProjectsLocationsAuthzPoliciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_authz_policies_create_builder(
            &self.http_client,
            &args.parent,
            &args.authzPolicyId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_authz_policies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations authz policies delete.
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
    pub fn networksecurity_projects_locations_authz_policies_delete(
        &self,
        args: &NetworksecurityProjectsLocationsAuthzPoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_authz_policies_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_authz_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations authz policies patch.
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
    pub fn networksecurity_projects_locations_authz_policies_patch(
        &self,
        args: &NetworksecurityProjectsLocationsAuthzPoliciesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_authz_policies_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_authz_policies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations authz policies set iam policy.
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
    pub fn networksecurity_projects_locations_authz_policies_set_iam_policy(
        &self,
        args: &NetworksecurityProjectsLocationsAuthzPoliciesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_authz_policies_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_authz_policies_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations authz policies test iam permissions.
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
    pub fn networksecurity_projects_locations_authz_policies_test_iam_permissions(
        &self,
        args: &NetworksecurityProjectsLocationsAuthzPoliciesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_authz_policies_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_authz_policies_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations backend authentication configs create.
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
    pub fn networksecurity_projects_locations_backend_authentication_configs_create(
        &self,
        args: &NetworksecurityProjectsLocationsBackendAuthenticationConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_backend_authentication_configs_create_builder(
            &self.http_client,
            &args.parent,
            &args.backendAuthenticationConfigId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_backend_authentication_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations backend authentication configs delete.
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
    pub fn networksecurity_projects_locations_backend_authentication_configs_delete(
        &self,
        args: &NetworksecurityProjectsLocationsBackendAuthenticationConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_backend_authentication_configs_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_backend_authentication_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations backend authentication configs patch.
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
    pub fn networksecurity_projects_locations_backend_authentication_configs_patch(
        &self,
        args: &NetworksecurityProjectsLocationsBackendAuthenticationConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_backend_authentication_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_backend_authentication_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations client tls policies create.
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
    pub fn networksecurity_projects_locations_client_tls_policies_create(
        &self,
        args: &NetworksecurityProjectsLocationsClientTlsPoliciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_client_tls_policies_create_builder(
            &self.http_client,
            &args.parent,
            &args.clientTlsPolicyId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_client_tls_policies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations client tls policies delete.
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
    pub fn networksecurity_projects_locations_client_tls_policies_delete(
        &self,
        args: &NetworksecurityProjectsLocationsClientTlsPoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_client_tls_policies_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_client_tls_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations client tls policies patch.
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
    pub fn networksecurity_projects_locations_client_tls_policies_patch(
        &self,
        args: &NetworksecurityProjectsLocationsClientTlsPoliciesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_client_tls_policies_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_client_tls_policies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations client tls policies set iam policy.
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
    pub fn networksecurity_projects_locations_client_tls_policies_set_iam_policy(
        &self,
        args: &NetworksecurityProjectsLocationsClientTlsPoliciesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_client_tls_policies_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_client_tls_policies_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations client tls policies test iam permissions.
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
    pub fn networksecurity_projects_locations_client_tls_policies_test_iam_permissions(
        &self,
        args: &NetworksecurityProjectsLocationsClientTlsPoliciesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_client_tls_policies_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_client_tls_policies_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations dns threat detectors create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DnsThreatDetector result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networksecurity_projects_locations_dns_threat_detectors_create(
        &self,
        args: &NetworksecurityProjectsLocationsDnsThreatDetectorsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DnsThreatDetector, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_dns_threat_detectors_create_builder(
            &self.http_client,
            &args.parent,
            &args.dnsThreatDetectorId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_dns_threat_detectors_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations dns threat detectors delete.
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
    pub fn networksecurity_projects_locations_dns_threat_detectors_delete(
        &self,
        args: &NetworksecurityProjectsLocationsDnsThreatDetectorsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_dns_threat_detectors_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_dns_threat_detectors_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations dns threat detectors patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DnsThreatDetector result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networksecurity_projects_locations_dns_threat_detectors_patch(
        &self,
        args: &NetworksecurityProjectsLocationsDnsThreatDetectorsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DnsThreatDetector, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_dns_threat_detectors_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_dns_threat_detectors_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations firewall endpoint associations create.
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
    pub fn networksecurity_projects_locations_firewall_endpoint_associations_create(
        &self,
        args: &NetworksecurityProjectsLocationsFirewallEndpointAssociationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_firewall_endpoint_associations_create_builder(
            &self.http_client,
            &args.parent,
            &args.firewallEndpointAssociationId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_firewall_endpoint_associations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations firewall endpoint associations delete.
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
    pub fn networksecurity_projects_locations_firewall_endpoint_associations_delete(
        &self,
        args: &NetworksecurityProjectsLocationsFirewallEndpointAssociationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_firewall_endpoint_associations_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_firewall_endpoint_associations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations firewall endpoint associations patch.
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
    pub fn networksecurity_projects_locations_firewall_endpoint_associations_patch(
        &self,
        args: &NetworksecurityProjectsLocationsFirewallEndpointAssociationsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_firewall_endpoint_associations_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_firewall_endpoint_associations_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations gateway security policies create.
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
    pub fn networksecurity_projects_locations_gateway_security_policies_create(
        &self,
        args: &NetworksecurityProjectsLocationsGatewaySecurityPoliciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_gateway_security_policies_create_builder(
            &self.http_client,
            &args.parent,
            &args.gatewaySecurityPolicyId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_gateway_security_policies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations gateway security policies delete.
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
    pub fn networksecurity_projects_locations_gateway_security_policies_delete(
        &self,
        args: &NetworksecurityProjectsLocationsGatewaySecurityPoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_gateway_security_policies_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_gateway_security_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations gateway security policies patch.
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
    pub fn networksecurity_projects_locations_gateway_security_policies_patch(
        &self,
        args: &NetworksecurityProjectsLocationsGatewaySecurityPoliciesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_gateway_security_policies_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_gateway_security_policies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations gateway security policies rules create.
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
    pub fn networksecurity_projects_locations_gateway_security_policies_rules_create(
        &self,
        args: &NetworksecurityProjectsLocationsGatewaySecurityPoliciesRulesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_gateway_security_policies_rules_create_builder(
            &self.http_client,
            &args.parent,
            &args.gatewaySecurityPolicyRuleId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_gateway_security_policies_rules_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations gateway security policies rules delete.
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
    pub fn networksecurity_projects_locations_gateway_security_policies_rules_delete(
        &self,
        args: &NetworksecurityProjectsLocationsGatewaySecurityPoliciesRulesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_gateway_security_policies_rules_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_gateway_security_policies_rules_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations gateway security policies rules patch.
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
    pub fn networksecurity_projects_locations_gateway_security_policies_rules_patch(
        &self,
        args: &NetworksecurityProjectsLocationsGatewaySecurityPoliciesRulesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_gateway_security_policies_rules_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_gateway_security_policies_rules_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations intercept deployment groups create.
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
    pub fn networksecurity_projects_locations_intercept_deployment_groups_create(
        &self,
        args: &NetworksecurityProjectsLocationsInterceptDeploymentGroupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_intercept_deployment_groups_create_builder(
            &self.http_client,
            &args.parent,
            &args.interceptDeploymentGroupId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_intercept_deployment_groups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations intercept deployment groups delete.
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
    pub fn networksecurity_projects_locations_intercept_deployment_groups_delete(
        &self,
        args: &NetworksecurityProjectsLocationsInterceptDeploymentGroupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_intercept_deployment_groups_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_intercept_deployment_groups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations intercept deployment groups patch.
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
    pub fn networksecurity_projects_locations_intercept_deployment_groups_patch(
        &self,
        args: &NetworksecurityProjectsLocationsInterceptDeploymentGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_intercept_deployment_groups_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_intercept_deployment_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations intercept deployments create.
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
    pub fn networksecurity_projects_locations_intercept_deployments_create(
        &self,
        args: &NetworksecurityProjectsLocationsInterceptDeploymentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_intercept_deployments_create_builder(
            &self.http_client,
            &args.parent,
            &args.interceptDeploymentId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_intercept_deployments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations intercept deployments delete.
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
    pub fn networksecurity_projects_locations_intercept_deployments_delete(
        &self,
        args: &NetworksecurityProjectsLocationsInterceptDeploymentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_intercept_deployments_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_intercept_deployments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations intercept deployments patch.
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
    pub fn networksecurity_projects_locations_intercept_deployments_patch(
        &self,
        args: &NetworksecurityProjectsLocationsInterceptDeploymentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_intercept_deployments_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_intercept_deployments_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations intercept endpoint group associations create.
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
    pub fn networksecurity_projects_locations_intercept_endpoint_group_associations_create(
        &self,
        args: &NetworksecurityProjectsLocationsInterceptEndpointGroupAssociationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_intercept_endpoint_group_associations_create_builder(
            &self.http_client,
            &args.parent,
            &args.interceptEndpointGroupAssociationId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_intercept_endpoint_group_associations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations intercept endpoint group associations delete.
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
    pub fn networksecurity_projects_locations_intercept_endpoint_group_associations_delete(
        &self,
        args: &NetworksecurityProjectsLocationsInterceptEndpointGroupAssociationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_intercept_endpoint_group_associations_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_intercept_endpoint_group_associations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations intercept endpoint group associations patch.
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
    pub fn networksecurity_projects_locations_intercept_endpoint_group_associations_patch(
        &self,
        args: &NetworksecurityProjectsLocationsInterceptEndpointGroupAssociationsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_intercept_endpoint_group_associations_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_intercept_endpoint_group_associations_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations intercept endpoint groups create.
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
    pub fn networksecurity_projects_locations_intercept_endpoint_groups_create(
        &self,
        args: &NetworksecurityProjectsLocationsInterceptEndpointGroupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_intercept_endpoint_groups_create_builder(
            &self.http_client,
            &args.parent,
            &args.interceptEndpointGroupId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_intercept_endpoint_groups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations intercept endpoint groups delete.
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
    pub fn networksecurity_projects_locations_intercept_endpoint_groups_delete(
        &self,
        args: &NetworksecurityProjectsLocationsInterceptEndpointGroupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_intercept_endpoint_groups_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_intercept_endpoint_groups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations intercept endpoint groups patch.
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
    pub fn networksecurity_projects_locations_intercept_endpoint_groups_patch(
        &self,
        args: &NetworksecurityProjectsLocationsInterceptEndpointGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_intercept_endpoint_groups_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_intercept_endpoint_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations mirroring deployment groups create.
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
    pub fn networksecurity_projects_locations_mirroring_deployment_groups_create(
        &self,
        args: &NetworksecurityProjectsLocationsMirroringDeploymentGroupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_mirroring_deployment_groups_create_builder(
            &self.http_client,
            &args.parent,
            &args.mirroringDeploymentGroupId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_mirroring_deployment_groups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations mirroring deployment groups delete.
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
    pub fn networksecurity_projects_locations_mirroring_deployment_groups_delete(
        &self,
        args: &NetworksecurityProjectsLocationsMirroringDeploymentGroupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_mirroring_deployment_groups_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_mirroring_deployment_groups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations mirroring deployment groups patch.
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
    pub fn networksecurity_projects_locations_mirroring_deployment_groups_patch(
        &self,
        args: &NetworksecurityProjectsLocationsMirroringDeploymentGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_mirroring_deployment_groups_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_mirroring_deployment_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations mirroring deployments create.
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
    pub fn networksecurity_projects_locations_mirroring_deployments_create(
        &self,
        args: &NetworksecurityProjectsLocationsMirroringDeploymentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_mirroring_deployments_create_builder(
            &self.http_client,
            &args.parent,
            &args.mirroringDeploymentId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_mirroring_deployments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations mirroring deployments delete.
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
    pub fn networksecurity_projects_locations_mirroring_deployments_delete(
        &self,
        args: &NetworksecurityProjectsLocationsMirroringDeploymentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_mirroring_deployments_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_mirroring_deployments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations mirroring deployments patch.
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
    pub fn networksecurity_projects_locations_mirroring_deployments_patch(
        &self,
        args: &NetworksecurityProjectsLocationsMirroringDeploymentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_mirroring_deployments_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_mirroring_deployments_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations mirroring endpoint group associations create.
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
    pub fn networksecurity_projects_locations_mirroring_endpoint_group_associations_create(
        &self,
        args: &NetworksecurityProjectsLocationsMirroringEndpointGroupAssociationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_mirroring_endpoint_group_associations_create_builder(
            &self.http_client,
            &args.parent,
            &args.mirroringEndpointGroupAssociationId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_mirroring_endpoint_group_associations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations mirroring endpoint group associations delete.
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
    pub fn networksecurity_projects_locations_mirroring_endpoint_group_associations_delete(
        &self,
        args: &NetworksecurityProjectsLocationsMirroringEndpointGroupAssociationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_mirroring_endpoint_group_associations_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_mirroring_endpoint_group_associations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations mirroring endpoint group associations patch.
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
    pub fn networksecurity_projects_locations_mirroring_endpoint_group_associations_patch(
        &self,
        args: &NetworksecurityProjectsLocationsMirroringEndpointGroupAssociationsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_mirroring_endpoint_group_associations_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_mirroring_endpoint_group_associations_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations mirroring endpoint groups create.
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
    pub fn networksecurity_projects_locations_mirroring_endpoint_groups_create(
        &self,
        args: &NetworksecurityProjectsLocationsMirroringEndpointGroupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_mirroring_endpoint_groups_create_builder(
            &self.http_client,
            &args.parent,
            &args.mirroringEndpointGroupId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_mirroring_endpoint_groups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations mirroring endpoint groups delete.
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
    pub fn networksecurity_projects_locations_mirroring_endpoint_groups_delete(
        &self,
        args: &NetworksecurityProjectsLocationsMirroringEndpointGroupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_mirroring_endpoint_groups_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_mirroring_endpoint_groups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations mirroring endpoint groups patch.
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
    pub fn networksecurity_projects_locations_mirroring_endpoint_groups_patch(
        &self,
        args: &NetworksecurityProjectsLocationsMirroringEndpointGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_mirroring_endpoint_groups_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_mirroring_endpoint_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations operations cancel.
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
    pub fn networksecurity_projects_locations_operations_cancel(
        &self,
        args: &NetworksecurityProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations operations delete.
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
    pub fn networksecurity_projects_locations_operations_delete(
        &self,
        args: &NetworksecurityProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations server tls policies create.
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
    pub fn networksecurity_projects_locations_server_tls_policies_create(
        &self,
        args: &NetworksecurityProjectsLocationsServerTlsPoliciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_server_tls_policies_create_builder(
            &self.http_client,
            &args.parent,
            &args.serverTlsPolicyId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_server_tls_policies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations server tls policies delete.
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
    pub fn networksecurity_projects_locations_server_tls_policies_delete(
        &self,
        args: &NetworksecurityProjectsLocationsServerTlsPoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_server_tls_policies_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_server_tls_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations server tls policies patch.
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
    pub fn networksecurity_projects_locations_server_tls_policies_patch(
        &self,
        args: &NetworksecurityProjectsLocationsServerTlsPoliciesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_server_tls_policies_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_server_tls_policies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations server tls policies set iam policy.
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
    pub fn networksecurity_projects_locations_server_tls_policies_set_iam_policy(
        &self,
        args: &NetworksecurityProjectsLocationsServerTlsPoliciesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_server_tls_policies_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_server_tls_policies_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations server tls policies test iam permissions.
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
    pub fn networksecurity_projects_locations_server_tls_policies_test_iam_permissions(
        &self,
        args: &NetworksecurityProjectsLocationsServerTlsPoliciesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_server_tls_policies_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_server_tls_policies_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations tls inspection policies create.
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
    pub fn networksecurity_projects_locations_tls_inspection_policies_create(
        &self,
        args: &NetworksecurityProjectsLocationsTlsInspectionPoliciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_tls_inspection_policies_create_builder(
            &self.http_client,
            &args.parent,
            &args.tlsInspectionPolicyId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_tls_inspection_policies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations tls inspection policies delete.
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
    pub fn networksecurity_projects_locations_tls_inspection_policies_delete(
        &self,
        args: &NetworksecurityProjectsLocationsTlsInspectionPoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_tls_inspection_policies_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_tls_inspection_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations tls inspection policies patch.
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
    pub fn networksecurity_projects_locations_tls_inspection_policies_patch(
        &self,
        args: &NetworksecurityProjectsLocationsTlsInspectionPoliciesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_tls_inspection_policies_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_tls_inspection_policies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations url lists create.
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
    pub fn networksecurity_projects_locations_url_lists_create(
        &self,
        args: &NetworksecurityProjectsLocationsUrlListsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_url_lists_create_builder(
            &self.http_client,
            &args.parent,
            &args.urlListId,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_url_lists_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations url lists delete.
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
    pub fn networksecurity_projects_locations_url_lists_delete(
        &self,
        args: &NetworksecurityProjectsLocationsUrlListsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_url_lists_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_url_lists_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networksecurity projects locations url lists patch.
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
    pub fn networksecurity_projects_locations_url_lists_patch(
        &self,
        args: &NetworksecurityProjectsLocationsUrlListsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networksecurity_projects_locations_url_lists_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networksecurity_projects_locations_url_lists_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
