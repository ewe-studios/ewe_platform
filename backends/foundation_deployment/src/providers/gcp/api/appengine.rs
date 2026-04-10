//! AppengineProvider - State-aware appengine API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       appengine API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::appengine::{
    appengine_apps_create_builder, appengine_apps_create_task,
    appengine_apps_patch_builder, appengine_apps_patch_task,
    appengine_apps_repair_builder, appengine_apps_repair_task,
    appengine_apps_authorized_certificates_create_builder, appengine_apps_authorized_certificates_create_task,
    appengine_apps_authorized_certificates_delete_builder, appengine_apps_authorized_certificates_delete_task,
    appengine_apps_authorized_certificates_patch_builder, appengine_apps_authorized_certificates_patch_task,
    appengine_apps_domain_mappings_create_builder, appengine_apps_domain_mappings_create_task,
    appengine_apps_domain_mappings_delete_builder, appengine_apps_domain_mappings_delete_task,
    appengine_apps_domain_mappings_patch_builder, appengine_apps_domain_mappings_patch_task,
    appengine_apps_firewall_ingress_rules_batch_update_builder, appengine_apps_firewall_ingress_rules_batch_update_task,
    appengine_apps_firewall_ingress_rules_create_builder, appengine_apps_firewall_ingress_rules_create_task,
    appengine_apps_firewall_ingress_rules_delete_builder, appengine_apps_firewall_ingress_rules_delete_task,
    appengine_apps_firewall_ingress_rules_patch_builder, appengine_apps_firewall_ingress_rules_patch_task,
    appengine_apps_services_delete_builder, appengine_apps_services_delete_task,
    appengine_apps_services_patch_builder, appengine_apps_services_patch_task,
    appengine_apps_services_versions_create_builder, appengine_apps_services_versions_create_task,
    appengine_apps_services_versions_delete_builder, appengine_apps_services_versions_delete_task,
    appengine_apps_services_versions_export_app_image_builder, appengine_apps_services_versions_export_app_image_task,
    appengine_apps_services_versions_patch_builder, appengine_apps_services_versions_patch_task,
    appengine_apps_services_versions_instances_debug_builder, appengine_apps_services_versions_instances_debug_task,
    appengine_apps_services_versions_instances_delete_builder, appengine_apps_services_versions_instances_delete_task,
    appengine_projects_locations_applications_patch_builder, appengine_projects_locations_applications_patch_task,
    appengine_projects_locations_applications_authorized_certificates_create_builder, appengine_projects_locations_applications_authorized_certificates_create_task,
    appengine_projects_locations_applications_authorized_certificates_delete_builder, appengine_projects_locations_applications_authorized_certificates_delete_task,
    appengine_projects_locations_applications_authorized_certificates_patch_builder, appengine_projects_locations_applications_authorized_certificates_patch_task,
    appengine_projects_locations_applications_domain_mappings_create_builder, appengine_projects_locations_applications_domain_mappings_create_task,
    appengine_projects_locations_applications_domain_mappings_delete_builder, appengine_projects_locations_applications_domain_mappings_delete_task,
    appengine_projects_locations_applications_domain_mappings_patch_builder, appengine_projects_locations_applications_domain_mappings_patch_task,
    appengine_projects_locations_applications_services_delete_builder, appengine_projects_locations_applications_services_delete_task,
    appengine_projects_locations_applications_services_patch_builder, appengine_projects_locations_applications_services_patch_task,
    appengine_projects_locations_applications_services_versions_delete_builder, appengine_projects_locations_applications_services_versions_delete_task,
    appengine_projects_locations_applications_services_versions_export_app_image_builder, appengine_projects_locations_applications_services_versions_export_app_image_task,
    appengine_projects_locations_applications_services_versions_patch_builder, appengine_projects_locations_applications_services_versions_patch_task,
    appengine_projects_locations_applications_services_versions_instances_debug_builder, appengine_projects_locations_applications_services_versions_instances_debug_task,
    appengine_projects_locations_applications_services_versions_instances_delete_builder, appengine_projects_locations_applications_services_versions_instances_delete_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::appengine::AuthorizedCertificate;
use crate::providers::gcp::clients::appengine::BatchUpdateIngressRulesResponse;
use crate::providers::gcp::clients::appengine::Empty;
use crate::providers::gcp::clients::appengine::FirewallRule;
use crate::providers::gcp::clients::appengine::Operation;
use crate::providers::gcp::clients::appengine::AppengineAppsAuthorizedCertificatesCreateArgs;
use crate::providers::gcp::clients::appengine::AppengineAppsAuthorizedCertificatesDeleteArgs;
use crate::providers::gcp::clients::appengine::AppengineAppsAuthorizedCertificatesPatchArgs;
use crate::providers::gcp::clients::appengine::AppengineAppsCreateArgs;
use crate::providers::gcp::clients::appengine::AppengineAppsDomainMappingsCreateArgs;
use crate::providers::gcp::clients::appengine::AppengineAppsDomainMappingsDeleteArgs;
use crate::providers::gcp::clients::appengine::AppengineAppsDomainMappingsPatchArgs;
use crate::providers::gcp::clients::appengine::AppengineAppsFirewallIngressRulesBatchUpdateArgs;
use crate::providers::gcp::clients::appengine::AppengineAppsFirewallIngressRulesCreateArgs;
use crate::providers::gcp::clients::appengine::AppengineAppsFirewallIngressRulesDeleteArgs;
use crate::providers::gcp::clients::appengine::AppengineAppsFirewallIngressRulesPatchArgs;
use crate::providers::gcp::clients::appengine::AppengineAppsPatchArgs;
use crate::providers::gcp::clients::appengine::AppengineAppsRepairArgs;
use crate::providers::gcp::clients::appengine::AppengineAppsServicesDeleteArgs;
use crate::providers::gcp::clients::appengine::AppengineAppsServicesPatchArgs;
use crate::providers::gcp::clients::appengine::AppengineAppsServicesVersionsCreateArgs;
use crate::providers::gcp::clients::appengine::AppengineAppsServicesVersionsDeleteArgs;
use crate::providers::gcp::clients::appengine::AppengineAppsServicesVersionsExportAppImageArgs;
use crate::providers::gcp::clients::appengine::AppengineAppsServicesVersionsInstancesDebugArgs;
use crate::providers::gcp::clients::appengine::AppengineAppsServicesVersionsInstancesDeleteArgs;
use crate::providers::gcp::clients::appengine::AppengineAppsServicesVersionsPatchArgs;
use crate::providers::gcp::clients::appengine::AppengineProjectsLocationsApplicationsAuthorizedCertificatesCreateArgs;
use crate::providers::gcp::clients::appengine::AppengineProjectsLocationsApplicationsAuthorizedCertificatesDeleteArgs;
use crate::providers::gcp::clients::appengine::AppengineProjectsLocationsApplicationsAuthorizedCertificatesPatchArgs;
use crate::providers::gcp::clients::appengine::AppengineProjectsLocationsApplicationsDomainMappingsCreateArgs;
use crate::providers::gcp::clients::appengine::AppengineProjectsLocationsApplicationsDomainMappingsDeleteArgs;
use crate::providers::gcp::clients::appengine::AppengineProjectsLocationsApplicationsDomainMappingsPatchArgs;
use crate::providers::gcp::clients::appengine::AppengineProjectsLocationsApplicationsPatchArgs;
use crate::providers::gcp::clients::appengine::AppengineProjectsLocationsApplicationsServicesDeleteArgs;
use crate::providers::gcp::clients::appengine::AppengineProjectsLocationsApplicationsServicesPatchArgs;
use crate::providers::gcp::clients::appengine::AppengineProjectsLocationsApplicationsServicesVersionsDeleteArgs;
use crate::providers::gcp::clients::appengine::AppengineProjectsLocationsApplicationsServicesVersionsExportAppImageArgs;
use crate::providers::gcp::clients::appengine::AppengineProjectsLocationsApplicationsServicesVersionsInstancesDebugArgs;
use crate::providers::gcp::clients::appengine::AppengineProjectsLocationsApplicationsServicesVersionsInstancesDeleteArgs;
use crate::providers::gcp::clients::appengine::AppengineProjectsLocationsApplicationsServicesVersionsPatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// AppengineProvider with automatic state tracking.
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
/// let provider = AppengineProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct AppengineProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> AppengineProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new AppengineProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Appengine apps create.
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
    pub fn appengine_apps_create(
        &self,
        args: &AppengineAppsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_apps_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_apps_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine apps patch.
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
    pub fn appengine_apps_patch(
        &self,
        args: &AppengineAppsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_apps_patch_builder(
            &self.http_client,
            &args.appsId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_apps_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine apps repair.
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
    pub fn appengine_apps_repair(
        &self,
        args: &AppengineAppsRepairArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_apps_repair_builder(
            &self.http_client,
            &args.appsId,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_apps_repair_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine apps authorized certificates create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AuthorizedCertificate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn appengine_apps_authorized_certificates_create(
        &self,
        args: &AppengineAppsAuthorizedCertificatesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AuthorizedCertificate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_apps_authorized_certificates_create_builder(
            &self.http_client,
            &args.appsId,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_apps_authorized_certificates_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine apps authorized certificates delete.
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
    pub fn appengine_apps_authorized_certificates_delete(
        &self,
        args: &AppengineAppsAuthorizedCertificatesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_apps_authorized_certificates_delete_builder(
            &self.http_client,
            &args.appsId,
            &args.authorizedCertificatesId,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_apps_authorized_certificates_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine apps authorized certificates patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AuthorizedCertificate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn appengine_apps_authorized_certificates_patch(
        &self,
        args: &AppengineAppsAuthorizedCertificatesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AuthorizedCertificate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_apps_authorized_certificates_patch_builder(
            &self.http_client,
            &args.appsId,
            &args.authorizedCertificatesId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_apps_authorized_certificates_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine apps domain mappings create.
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
    pub fn appengine_apps_domain_mappings_create(
        &self,
        args: &AppengineAppsDomainMappingsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_apps_domain_mappings_create_builder(
            &self.http_client,
            &args.appsId,
            &args.overrideStrategy,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_apps_domain_mappings_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine apps domain mappings delete.
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
    pub fn appengine_apps_domain_mappings_delete(
        &self,
        args: &AppengineAppsDomainMappingsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_apps_domain_mappings_delete_builder(
            &self.http_client,
            &args.appsId,
            &args.domainMappingsId,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_apps_domain_mappings_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine apps domain mappings patch.
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
    pub fn appengine_apps_domain_mappings_patch(
        &self,
        args: &AppengineAppsDomainMappingsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_apps_domain_mappings_patch_builder(
            &self.http_client,
            &args.appsId,
            &args.domainMappingsId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_apps_domain_mappings_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine apps firewall ingress rules batch update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchUpdateIngressRulesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn appengine_apps_firewall_ingress_rules_batch_update(
        &self,
        args: &AppengineAppsFirewallIngressRulesBatchUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchUpdateIngressRulesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_apps_firewall_ingress_rules_batch_update_builder(
            &self.http_client,
            &args.appsId,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_apps_firewall_ingress_rules_batch_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine apps firewall ingress rules create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FirewallRule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn appengine_apps_firewall_ingress_rules_create(
        &self,
        args: &AppengineAppsFirewallIngressRulesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FirewallRule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_apps_firewall_ingress_rules_create_builder(
            &self.http_client,
            &args.appsId,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_apps_firewall_ingress_rules_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine apps firewall ingress rules delete.
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
    pub fn appengine_apps_firewall_ingress_rules_delete(
        &self,
        args: &AppengineAppsFirewallIngressRulesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_apps_firewall_ingress_rules_delete_builder(
            &self.http_client,
            &args.appsId,
            &args.ingressRulesId,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_apps_firewall_ingress_rules_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine apps firewall ingress rules patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FirewallRule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn appengine_apps_firewall_ingress_rules_patch(
        &self,
        args: &AppengineAppsFirewallIngressRulesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FirewallRule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_apps_firewall_ingress_rules_patch_builder(
            &self.http_client,
            &args.appsId,
            &args.ingressRulesId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_apps_firewall_ingress_rules_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine apps services delete.
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
    pub fn appengine_apps_services_delete(
        &self,
        args: &AppengineAppsServicesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_apps_services_delete_builder(
            &self.http_client,
            &args.appsId,
            &args.servicesId,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_apps_services_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine apps services patch.
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
    pub fn appengine_apps_services_patch(
        &self,
        args: &AppengineAppsServicesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_apps_services_patch_builder(
            &self.http_client,
            &args.appsId,
            &args.servicesId,
            &args.migrateTraffic,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_apps_services_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine apps services versions create.
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
    pub fn appengine_apps_services_versions_create(
        &self,
        args: &AppengineAppsServicesVersionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_apps_services_versions_create_builder(
            &self.http_client,
            &args.appsId,
            &args.servicesId,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_apps_services_versions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine apps services versions delete.
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
    pub fn appengine_apps_services_versions_delete(
        &self,
        args: &AppengineAppsServicesVersionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_apps_services_versions_delete_builder(
            &self.http_client,
            &args.appsId,
            &args.servicesId,
            &args.versionsId,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_apps_services_versions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine apps services versions export app image.
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
    pub fn appengine_apps_services_versions_export_app_image(
        &self,
        args: &AppengineAppsServicesVersionsExportAppImageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_apps_services_versions_export_app_image_builder(
            &self.http_client,
            &args.appsId,
            &args.servicesId,
            &args.versionsId,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_apps_services_versions_export_app_image_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine apps services versions patch.
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
    pub fn appengine_apps_services_versions_patch(
        &self,
        args: &AppengineAppsServicesVersionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_apps_services_versions_patch_builder(
            &self.http_client,
            &args.appsId,
            &args.servicesId,
            &args.versionsId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_apps_services_versions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine apps services versions instances debug.
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
    pub fn appengine_apps_services_versions_instances_debug(
        &self,
        args: &AppengineAppsServicesVersionsInstancesDebugArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_apps_services_versions_instances_debug_builder(
            &self.http_client,
            &args.appsId,
            &args.servicesId,
            &args.versionsId,
            &args.instancesId,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_apps_services_versions_instances_debug_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine apps services versions instances delete.
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
    pub fn appengine_apps_services_versions_instances_delete(
        &self,
        args: &AppengineAppsServicesVersionsInstancesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_apps_services_versions_instances_delete_builder(
            &self.http_client,
            &args.appsId,
            &args.servicesId,
            &args.versionsId,
            &args.instancesId,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_apps_services_versions_instances_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine projects locations applications patch.
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
    pub fn appengine_projects_locations_applications_patch(
        &self,
        args: &AppengineProjectsLocationsApplicationsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_projects_locations_applications_patch_builder(
            &self.http_client,
            &args.projectsId,
            &args.locationsId,
            &args.applicationsId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_projects_locations_applications_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine projects locations applications authorized certificates create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AuthorizedCertificate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn appengine_projects_locations_applications_authorized_certificates_create(
        &self,
        args: &AppengineProjectsLocationsApplicationsAuthorizedCertificatesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AuthorizedCertificate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_projects_locations_applications_authorized_certificates_create_builder(
            &self.http_client,
            &args.projectsId,
            &args.locationsId,
            &args.applicationsId,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_projects_locations_applications_authorized_certificates_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine projects locations applications authorized certificates delete.
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
    pub fn appengine_projects_locations_applications_authorized_certificates_delete(
        &self,
        args: &AppengineProjectsLocationsApplicationsAuthorizedCertificatesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_projects_locations_applications_authorized_certificates_delete_builder(
            &self.http_client,
            &args.projectsId,
            &args.locationsId,
            &args.applicationsId,
            &args.authorizedCertificatesId,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_projects_locations_applications_authorized_certificates_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine projects locations applications authorized certificates patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AuthorizedCertificate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn appengine_projects_locations_applications_authorized_certificates_patch(
        &self,
        args: &AppengineProjectsLocationsApplicationsAuthorizedCertificatesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AuthorizedCertificate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_projects_locations_applications_authorized_certificates_patch_builder(
            &self.http_client,
            &args.projectsId,
            &args.locationsId,
            &args.applicationsId,
            &args.authorizedCertificatesId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_projects_locations_applications_authorized_certificates_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine projects locations applications domain mappings create.
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
    pub fn appengine_projects_locations_applications_domain_mappings_create(
        &self,
        args: &AppengineProjectsLocationsApplicationsDomainMappingsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_projects_locations_applications_domain_mappings_create_builder(
            &self.http_client,
            &args.projectsId,
            &args.locationsId,
            &args.applicationsId,
            &args.overrideStrategy,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_projects_locations_applications_domain_mappings_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine projects locations applications domain mappings delete.
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
    pub fn appengine_projects_locations_applications_domain_mappings_delete(
        &self,
        args: &AppengineProjectsLocationsApplicationsDomainMappingsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_projects_locations_applications_domain_mappings_delete_builder(
            &self.http_client,
            &args.projectsId,
            &args.locationsId,
            &args.applicationsId,
            &args.domainMappingsId,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_projects_locations_applications_domain_mappings_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine projects locations applications domain mappings patch.
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
    pub fn appengine_projects_locations_applications_domain_mappings_patch(
        &self,
        args: &AppengineProjectsLocationsApplicationsDomainMappingsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_projects_locations_applications_domain_mappings_patch_builder(
            &self.http_client,
            &args.projectsId,
            &args.locationsId,
            &args.applicationsId,
            &args.domainMappingsId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_projects_locations_applications_domain_mappings_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine projects locations applications services delete.
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
    pub fn appengine_projects_locations_applications_services_delete(
        &self,
        args: &AppengineProjectsLocationsApplicationsServicesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_projects_locations_applications_services_delete_builder(
            &self.http_client,
            &args.projectsId,
            &args.locationsId,
            &args.applicationsId,
            &args.servicesId,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_projects_locations_applications_services_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine projects locations applications services patch.
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
    pub fn appengine_projects_locations_applications_services_patch(
        &self,
        args: &AppengineProjectsLocationsApplicationsServicesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_projects_locations_applications_services_patch_builder(
            &self.http_client,
            &args.projectsId,
            &args.locationsId,
            &args.applicationsId,
            &args.servicesId,
            &args.migrateTraffic,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_projects_locations_applications_services_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine projects locations applications services versions delete.
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
    pub fn appengine_projects_locations_applications_services_versions_delete(
        &self,
        args: &AppengineProjectsLocationsApplicationsServicesVersionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_projects_locations_applications_services_versions_delete_builder(
            &self.http_client,
            &args.projectsId,
            &args.locationsId,
            &args.applicationsId,
            &args.servicesId,
            &args.versionsId,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_projects_locations_applications_services_versions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine projects locations applications services versions export app image.
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
    pub fn appengine_projects_locations_applications_services_versions_export_app_image(
        &self,
        args: &AppengineProjectsLocationsApplicationsServicesVersionsExportAppImageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_projects_locations_applications_services_versions_export_app_image_builder(
            &self.http_client,
            &args.projectsId,
            &args.locationsId,
            &args.applicationsId,
            &args.servicesId,
            &args.versionsId,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_projects_locations_applications_services_versions_export_app_image_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine projects locations applications services versions patch.
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
    pub fn appengine_projects_locations_applications_services_versions_patch(
        &self,
        args: &AppengineProjectsLocationsApplicationsServicesVersionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_projects_locations_applications_services_versions_patch_builder(
            &self.http_client,
            &args.projectsId,
            &args.locationsId,
            &args.applicationsId,
            &args.servicesId,
            &args.versionsId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_projects_locations_applications_services_versions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine projects locations applications services versions instances debug.
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
    pub fn appengine_projects_locations_applications_services_versions_instances_debug(
        &self,
        args: &AppengineProjectsLocationsApplicationsServicesVersionsInstancesDebugArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_projects_locations_applications_services_versions_instances_debug_builder(
            &self.http_client,
            &args.projectsId,
            &args.locationsId,
            &args.applicationsId,
            &args.servicesId,
            &args.versionsId,
            &args.instancesId,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_projects_locations_applications_services_versions_instances_debug_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appengine projects locations applications services versions instances delete.
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
    pub fn appengine_projects_locations_applications_services_versions_instances_delete(
        &self,
        args: &AppengineProjectsLocationsApplicationsServicesVersionsInstancesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appengine_projects_locations_applications_services_versions_instances_delete_builder(
            &self.http_client,
            &args.projectsId,
            &args.locationsId,
            &args.applicationsId,
            &args.servicesId,
            &args.versionsId,
            &args.instancesId,
        )
        .map_err(ProviderError::Api)?;

        let task = appengine_projects_locations_applications_services_versions_instances_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
