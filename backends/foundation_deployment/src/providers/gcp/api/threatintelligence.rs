//! ThreatintelligenceProvider - State-aware threatintelligence API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       threatintelligence API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::threatintelligence::{
    threatintelligence_projects_generate_org_profile_builder, threatintelligence_projects_generate_org_profile_task,
    threatintelligence_projects_alerts_benign_builder, threatintelligence_projects_alerts_benign_task,
    threatintelligence_projects_alerts_duplicate_builder, threatintelligence_projects_alerts_duplicate_task,
    threatintelligence_projects_alerts_enumerate_facets_builder, threatintelligence_projects_alerts_enumerate_facets_task,
    threatintelligence_projects_alerts_escalate_builder, threatintelligence_projects_alerts_escalate_task,
    threatintelligence_projects_alerts_false_positive_builder, threatintelligence_projects_alerts_false_positive_task,
    threatintelligence_projects_alerts_get_builder, threatintelligence_projects_alerts_get_task,
    threatintelligence_projects_alerts_list_builder, threatintelligence_projects_alerts_list_task,
    threatintelligence_projects_alerts_not_actionable_builder, threatintelligence_projects_alerts_not_actionable_task,
    threatintelligence_projects_alerts_read_builder, threatintelligence_projects_alerts_read_task,
    threatintelligence_projects_alerts_resolve_builder, threatintelligence_projects_alerts_resolve_task,
    threatintelligence_projects_alerts_track_externally_builder, threatintelligence_projects_alerts_track_externally_task,
    threatintelligence_projects_alerts_triage_builder, threatintelligence_projects_alerts_triage_task,
    threatintelligence_projects_alerts_documents_get_builder, threatintelligence_projects_alerts_documents_get_task,
    threatintelligence_projects_configurations_get_builder, threatintelligence_projects_configurations_get_task,
    threatintelligence_projects_configurations_list_builder, threatintelligence_projects_configurations_list_task,
    threatintelligence_projects_configurations_upsert_builder, threatintelligence_projects_configurations_upsert_task,
    threatintelligence_projects_configurations_revisions_list_builder, threatintelligence_projects_configurations_revisions_list_task,
    threatintelligence_projects_findings_get_builder, threatintelligence_projects_findings_get_task,
    threatintelligence_projects_findings_list_builder, threatintelligence_projects_findings_list_task,
    threatintelligence_projects_findings_search_builder, threatintelligence_projects_findings_search_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::threatintelligence::Alert;
use crate::providers::gcp::clients::threatintelligence::AlertDocument;
use crate::providers::gcp::clients::threatintelligence::Configuration;
use crate::providers::gcp::clients::threatintelligence::EnumerateAlertFacetsResponse;
use crate::providers::gcp::clients::threatintelligence::Finding;
use crate::providers::gcp::clients::threatintelligence::ListAlertsResponse;
use crate::providers::gcp::clients::threatintelligence::ListConfigurationRevisionsResponse;
use crate::providers::gcp::clients::threatintelligence::ListConfigurationsResponse;
use crate::providers::gcp::clients::threatintelligence::ListFindingsResponse;
use crate::providers::gcp::clients::threatintelligence::Operation;
use crate::providers::gcp::clients::threatintelligence::SearchFindingsResponse;
use crate::providers::gcp::clients::threatintelligence::UpsertConfigurationResponse;
use crate::providers::gcp::clients::threatintelligence::ThreatintelligenceProjectsAlertsBenignArgs;
use crate::providers::gcp::clients::threatintelligence::ThreatintelligenceProjectsAlertsDocumentsGetArgs;
use crate::providers::gcp::clients::threatintelligence::ThreatintelligenceProjectsAlertsDuplicateArgs;
use crate::providers::gcp::clients::threatintelligence::ThreatintelligenceProjectsAlertsEnumerateFacetsArgs;
use crate::providers::gcp::clients::threatintelligence::ThreatintelligenceProjectsAlertsEscalateArgs;
use crate::providers::gcp::clients::threatintelligence::ThreatintelligenceProjectsAlertsFalsePositiveArgs;
use crate::providers::gcp::clients::threatintelligence::ThreatintelligenceProjectsAlertsGetArgs;
use crate::providers::gcp::clients::threatintelligence::ThreatintelligenceProjectsAlertsListArgs;
use crate::providers::gcp::clients::threatintelligence::ThreatintelligenceProjectsAlertsNotActionableArgs;
use crate::providers::gcp::clients::threatintelligence::ThreatintelligenceProjectsAlertsReadArgs;
use crate::providers::gcp::clients::threatintelligence::ThreatintelligenceProjectsAlertsResolveArgs;
use crate::providers::gcp::clients::threatintelligence::ThreatintelligenceProjectsAlertsTrackExternallyArgs;
use crate::providers::gcp::clients::threatintelligence::ThreatintelligenceProjectsAlertsTriageArgs;
use crate::providers::gcp::clients::threatintelligence::ThreatintelligenceProjectsConfigurationsGetArgs;
use crate::providers::gcp::clients::threatintelligence::ThreatintelligenceProjectsConfigurationsListArgs;
use crate::providers::gcp::clients::threatintelligence::ThreatintelligenceProjectsConfigurationsRevisionsListArgs;
use crate::providers::gcp::clients::threatintelligence::ThreatintelligenceProjectsConfigurationsUpsertArgs;
use crate::providers::gcp::clients::threatintelligence::ThreatintelligenceProjectsFindingsGetArgs;
use crate::providers::gcp::clients::threatintelligence::ThreatintelligenceProjectsFindingsListArgs;
use crate::providers::gcp::clients::threatintelligence::ThreatintelligenceProjectsFindingsSearchArgs;
use crate::providers::gcp::clients::threatintelligence::ThreatintelligenceProjectsGenerateOrgProfileArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ThreatintelligenceProvider with automatic state tracking.
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
/// let provider = ThreatintelligenceProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ThreatintelligenceProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ThreatintelligenceProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ThreatintelligenceProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Threatintelligence projects generate org profile.
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
    pub fn threatintelligence_projects_generate_org_profile(
        &self,
        args: &ThreatintelligenceProjectsGenerateOrgProfileArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = threatintelligence_projects_generate_org_profile_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = threatintelligence_projects_generate_org_profile_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Threatintelligence projects alerts benign.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Alert result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn threatintelligence_projects_alerts_benign(
        &self,
        args: &ThreatintelligenceProjectsAlertsBenignArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Alert, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = threatintelligence_projects_alerts_benign_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = threatintelligence_projects_alerts_benign_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Threatintelligence projects alerts duplicate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Alert result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn threatintelligence_projects_alerts_duplicate(
        &self,
        args: &ThreatintelligenceProjectsAlertsDuplicateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Alert, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = threatintelligence_projects_alerts_duplicate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = threatintelligence_projects_alerts_duplicate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Threatintelligence projects alerts enumerate facets.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EnumerateAlertFacetsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn threatintelligence_projects_alerts_enumerate_facets(
        &self,
        args: &ThreatintelligenceProjectsAlertsEnumerateFacetsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EnumerateAlertFacetsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = threatintelligence_projects_alerts_enumerate_facets_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
        )
        .map_err(ProviderError::Api)?;

        let task = threatintelligence_projects_alerts_enumerate_facets_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Threatintelligence projects alerts escalate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Alert result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn threatintelligence_projects_alerts_escalate(
        &self,
        args: &ThreatintelligenceProjectsAlertsEscalateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Alert, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = threatintelligence_projects_alerts_escalate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = threatintelligence_projects_alerts_escalate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Threatintelligence projects alerts false positive.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Alert result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn threatintelligence_projects_alerts_false_positive(
        &self,
        args: &ThreatintelligenceProjectsAlertsFalsePositiveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Alert, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = threatintelligence_projects_alerts_false_positive_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = threatintelligence_projects_alerts_false_positive_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Threatintelligence projects alerts get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Alert result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn threatintelligence_projects_alerts_get(
        &self,
        args: &ThreatintelligenceProjectsAlertsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Alert, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = threatintelligence_projects_alerts_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = threatintelligence_projects_alerts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Threatintelligence projects alerts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAlertsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn threatintelligence_projects_alerts_list(
        &self,
        args: &ThreatintelligenceProjectsAlertsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAlertsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = threatintelligence_projects_alerts_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = threatintelligence_projects_alerts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Threatintelligence projects alerts not actionable.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Alert result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn threatintelligence_projects_alerts_not_actionable(
        &self,
        args: &ThreatintelligenceProjectsAlertsNotActionableArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Alert, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = threatintelligence_projects_alerts_not_actionable_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = threatintelligence_projects_alerts_not_actionable_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Threatintelligence projects alerts read.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Alert result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn threatintelligence_projects_alerts_read(
        &self,
        args: &ThreatintelligenceProjectsAlertsReadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Alert, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = threatintelligence_projects_alerts_read_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = threatintelligence_projects_alerts_read_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Threatintelligence projects alerts resolve.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Alert result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn threatintelligence_projects_alerts_resolve(
        &self,
        args: &ThreatintelligenceProjectsAlertsResolveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Alert, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = threatintelligence_projects_alerts_resolve_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = threatintelligence_projects_alerts_resolve_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Threatintelligence projects alerts track externally.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Alert result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn threatintelligence_projects_alerts_track_externally(
        &self,
        args: &ThreatintelligenceProjectsAlertsTrackExternallyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Alert, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = threatintelligence_projects_alerts_track_externally_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = threatintelligence_projects_alerts_track_externally_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Threatintelligence projects alerts triage.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Alert result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn threatintelligence_projects_alerts_triage(
        &self,
        args: &ThreatintelligenceProjectsAlertsTriageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Alert, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = threatintelligence_projects_alerts_triage_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = threatintelligence_projects_alerts_triage_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Threatintelligence projects alerts documents get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AlertDocument result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn threatintelligence_projects_alerts_documents_get(
        &self,
        args: &ThreatintelligenceProjectsAlertsDocumentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AlertDocument, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = threatintelligence_projects_alerts_documents_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = threatintelligence_projects_alerts_documents_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Threatintelligence projects configurations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Configuration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn threatintelligence_projects_configurations_get(
        &self,
        args: &ThreatintelligenceProjectsConfigurationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Configuration, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = threatintelligence_projects_configurations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = threatintelligence_projects_configurations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Threatintelligence projects configurations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListConfigurationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn threatintelligence_projects_configurations_list(
        &self,
        args: &ThreatintelligenceProjectsConfigurationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListConfigurationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = threatintelligence_projects_configurations_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = threatintelligence_projects_configurations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Threatintelligence projects configurations upsert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UpsertConfigurationResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn threatintelligence_projects_configurations_upsert(
        &self,
        args: &ThreatintelligenceProjectsConfigurationsUpsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UpsertConfigurationResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = threatintelligence_projects_configurations_upsert_builder(
            &self.http_client,
            &args.parent,
            &args.publishTime,
        )
        .map_err(ProviderError::Api)?;

        let task = threatintelligence_projects_configurations_upsert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Threatintelligence projects configurations revisions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListConfigurationRevisionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn threatintelligence_projects_configurations_revisions_list(
        &self,
        args: &ThreatintelligenceProjectsConfigurationsRevisionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListConfigurationRevisionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = threatintelligence_projects_configurations_revisions_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = threatintelligence_projects_configurations_revisions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Threatintelligence projects findings get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Finding result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn threatintelligence_projects_findings_get(
        &self,
        args: &ThreatintelligenceProjectsFindingsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Finding, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = threatintelligence_projects_findings_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = threatintelligence_projects_findings_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Threatintelligence projects findings list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListFindingsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn threatintelligence_projects_findings_list(
        &self,
        args: &ThreatintelligenceProjectsFindingsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListFindingsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = threatintelligence_projects_findings_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = threatintelligence_projects_findings_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Threatintelligence projects findings search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchFindingsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn threatintelligence_projects_findings_search(
        &self,
        args: &ThreatintelligenceProjectsFindingsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchFindingsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = threatintelligence_projects_findings_search_builder(
            &self.http_client,
            &args.parent,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.query,
        )
        .map_err(ProviderError::Api)?;

        let task = threatintelligence_projects_findings_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
