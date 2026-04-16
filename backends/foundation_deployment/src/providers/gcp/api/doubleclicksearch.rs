//! DoubleclicksearchProvider - State-aware doubleclicksearch API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       doubleclicksearch API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::doubleclicksearch::{
    doubleclicksearch_conversion_get_builder, doubleclicksearch_conversion_get_task,
    doubleclicksearch_conversion_get_by_customer_id_builder, doubleclicksearch_conversion_get_by_customer_id_task,
    doubleclicksearch_conversion_insert_builder, doubleclicksearch_conversion_insert_task,
    doubleclicksearch_conversion_update_builder, doubleclicksearch_conversion_update_task,
    doubleclicksearch_conversion_update_availability_builder, doubleclicksearch_conversion_update_availability_task,
    doubleclicksearch_reports_generate_builder, doubleclicksearch_reports_generate_task,
    doubleclicksearch_reports_get_builder, doubleclicksearch_reports_get_task,
    doubleclicksearch_reports_get_file_builder, doubleclicksearch_reports_get_file_task,
    doubleclicksearch_reports_get_id_mapping_file_builder, doubleclicksearch_reports_get_id_mapping_file_task,
    doubleclicksearch_reports_request_builder, doubleclicksearch_reports_request_task,
    doubleclicksearch_saved_columns_list_builder, doubleclicksearch_saved_columns_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::doubleclicksearch::ConversionList;
use crate::providers::gcp::clients::doubleclicksearch::IdMappingFile;
use crate::providers::gcp::clients::doubleclicksearch::Report;
use crate::providers::gcp::clients::doubleclicksearch::SavedColumnList;
use crate::providers::gcp::clients::doubleclicksearch::UpdateAvailabilityResponse;
use crate::providers::gcp::clients::doubleclicksearch::DoubleclicksearchConversionGetArgs;
use crate::providers::gcp::clients::doubleclicksearch::DoubleclicksearchConversionGetByCustomerIdArgs;
use crate::providers::gcp::clients::doubleclicksearch::DoubleclicksearchReportsGetArgs;
use crate::providers::gcp::clients::doubleclicksearch::DoubleclicksearchReportsGetFileArgs;
use crate::providers::gcp::clients::doubleclicksearch::DoubleclicksearchReportsGetIdMappingFileArgs;
use crate::providers::gcp::clients::doubleclicksearch::DoubleclicksearchSavedColumnsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DoubleclicksearchProvider with automatic state tracking.
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
/// let provider = DoubleclicksearchProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct DoubleclicksearchProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> DoubleclicksearchProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new DoubleclicksearchProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new DoubleclicksearchProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Doubleclicksearch conversion get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConversionList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn doubleclicksearch_conversion_get(
        &self,
        args: &DoubleclicksearchConversionGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConversionList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = doubleclicksearch_conversion_get_builder(
            &self.http_client,
            &args.agencyId,
            &args.advertiserId,
            &args.engineAccountId,
            &args.adGroupId,
            &args.adId,
            &args.campaignId,
            &args.criterionId,
            &args.customerId,
            &args.endDate,
            &args.rowCount,
            &args.startDate,
            &args.startRow,
        )
        .map_err(ProviderError::Api)?;

        let task = doubleclicksearch_conversion_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Doubleclicksearch conversion get by customer id.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConversionList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn doubleclicksearch_conversion_get_by_customer_id(
        &self,
        args: &DoubleclicksearchConversionGetByCustomerIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConversionList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = doubleclicksearch_conversion_get_by_customer_id_builder(
            &self.http_client,
            &args.customerId,
            &args.adGroupId,
            &args.adId,
            &args.advertiserId,
            &args.agencyId,
            &args.campaignId,
            &args.criterionId,
            &args.endDate,
            &args.engineAccountId,
            &args.rowCount,
            &args.startDate,
            &args.startRow,
        )
        .map_err(ProviderError::Api)?;

        let task = doubleclicksearch_conversion_get_by_customer_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Doubleclicksearch conversion insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConversionList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn doubleclicksearch_conversion_insert(
        &self,
        args: &DoubleclicksearchConversionInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConversionList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = doubleclicksearch_conversion_insert_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = doubleclicksearch_conversion_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Doubleclicksearch conversion update.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConversionList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn doubleclicksearch_conversion_update(
        &self,
        args: &DoubleclicksearchConversionUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConversionList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = doubleclicksearch_conversion_update_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = doubleclicksearch_conversion_update_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Doubleclicksearch conversion update availability.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UpdateAvailabilityResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn doubleclicksearch_conversion_update_availability(
        &self,
        args: &DoubleclicksearchConversionUpdateAvailabilityArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UpdateAvailabilityResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = doubleclicksearch_conversion_update_availability_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = doubleclicksearch_conversion_update_availability_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Doubleclicksearch reports generate.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Report result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn doubleclicksearch_reports_generate(
        &self,
        args: &DoubleclicksearchReportsGenerateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Report, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = doubleclicksearch_reports_generate_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = doubleclicksearch_reports_generate_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Doubleclicksearch reports get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Report result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn doubleclicksearch_reports_get(
        &self,
        args: &DoubleclicksearchReportsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Report, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = doubleclicksearch_reports_get_builder(
            &self.http_client,
            &args.reportId,
        )
        .map_err(ProviderError::Api)?;

        let task = doubleclicksearch_reports_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Doubleclicksearch reports get file.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn doubleclicksearch_reports_get_file(
        &self,
        args: &DoubleclicksearchReportsGetFileArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = doubleclicksearch_reports_get_file_builder(
            &self.http_client,
            &args.reportId,
            &args.reportFragment,
        )
        .map_err(ProviderError::Api)?;

        let task = doubleclicksearch_reports_get_file_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Doubleclicksearch reports get id mapping file.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IdMappingFile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn doubleclicksearch_reports_get_id_mapping_file(
        &self,
        args: &DoubleclicksearchReportsGetIdMappingFileArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IdMappingFile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = doubleclicksearch_reports_get_id_mapping_file_builder(
            &self.http_client,
            &args.agencyId,
            &args.advertiserId,
        )
        .map_err(ProviderError::Api)?;

        let task = doubleclicksearch_reports_get_id_mapping_file_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Doubleclicksearch reports request.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Report result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn doubleclicksearch_reports_request(
        &self,
        args: &DoubleclicksearchReportsRequestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Report, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = doubleclicksearch_reports_request_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = doubleclicksearch_reports_request_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Doubleclicksearch saved columns list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SavedColumnList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn doubleclicksearch_saved_columns_list(
        &self,
        args: &DoubleclicksearchSavedColumnsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SavedColumnList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = doubleclicksearch_saved_columns_list_builder(
            &self.http_client,
            &args.agencyId,
            &args.advertiserId,
        )
        .map_err(ProviderError::Api)?;

        let task = doubleclicksearch_saved_columns_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
