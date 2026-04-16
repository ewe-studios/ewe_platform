//! SheetsProvider - State-aware sheets API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       sheets API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::sheets::{
    sheets_spreadsheets_batch_update_builder, sheets_spreadsheets_batch_update_task,
    sheets_spreadsheets_create_builder, sheets_spreadsheets_create_task,
    sheets_spreadsheets_get_builder, sheets_spreadsheets_get_task,
    sheets_spreadsheets_get_by_data_filter_builder, sheets_spreadsheets_get_by_data_filter_task,
    sheets_spreadsheets_developer_metadata_get_builder, sheets_spreadsheets_developer_metadata_get_task,
    sheets_spreadsheets_developer_metadata_search_builder, sheets_spreadsheets_developer_metadata_search_task,
    sheets_spreadsheets_sheets_copy_to_builder, sheets_spreadsheets_sheets_copy_to_task,
    sheets_spreadsheets_values_append_builder, sheets_spreadsheets_values_append_task,
    sheets_spreadsheets_values_batch_clear_builder, sheets_spreadsheets_values_batch_clear_task,
    sheets_spreadsheets_values_batch_clear_by_data_filter_builder, sheets_spreadsheets_values_batch_clear_by_data_filter_task,
    sheets_spreadsheets_values_batch_get_builder, sheets_spreadsheets_values_batch_get_task,
    sheets_spreadsheets_values_batch_get_by_data_filter_builder, sheets_spreadsheets_values_batch_get_by_data_filter_task,
    sheets_spreadsheets_values_batch_update_builder, sheets_spreadsheets_values_batch_update_task,
    sheets_spreadsheets_values_batch_update_by_data_filter_builder, sheets_spreadsheets_values_batch_update_by_data_filter_task,
    sheets_spreadsheets_values_clear_builder, sheets_spreadsheets_values_clear_task,
    sheets_spreadsheets_values_get_builder, sheets_spreadsheets_values_get_task,
    sheets_spreadsheets_values_update_builder, sheets_spreadsheets_values_update_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::sheets::AppendValuesResponse;
use crate::providers::gcp::clients::sheets::BatchClearValuesByDataFilterResponse;
use crate::providers::gcp::clients::sheets::BatchClearValuesResponse;
use crate::providers::gcp::clients::sheets::BatchGetValuesByDataFilterResponse;
use crate::providers::gcp::clients::sheets::BatchGetValuesResponse;
use crate::providers::gcp::clients::sheets::BatchUpdateSpreadsheetResponse;
use crate::providers::gcp::clients::sheets::BatchUpdateValuesByDataFilterResponse;
use crate::providers::gcp::clients::sheets::BatchUpdateValuesResponse;
use crate::providers::gcp::clients::sheets::ClearValuesResponse;
use crate::providers::gcp::clients::sheets::DeveloperMetadata;
use crate::providers::gcp::clients::sheets::SearchDeveloperMetadataResponse;
use crate::providers::gcp::clients::sheets::SheetProperties;
use crate::providers::gcp::clients::sheets::Spreadsheet;
use crate::providers::gcp::clients::sheets::UpdateValuesResponse;
use crate::providers::gcp::clients::sheets::ValueRange;
use crate::providers::gcp::clients::sheets::SheetsSpreadsheetsBatchUpdateArgs;
use crate::providers::gcp::clients::sheets::SheetsSpreadsheetsDeveloperMetadataGetArgs;
use crate::providers::gcp::clients::sheets::SheetsSpreadsheetsDeveloperMetadataSearchArgs;
use crate::providers::gcp::clients::sheets::SheetsSpreadsheetsGetArgs;
use crate::providers::gcp::clients::sheets::SheetsSpreadsheetsGetByDataFilterArgs;
use crate::providers::gcp::clients::sheets::SheetsSpreadsheetsSheetsCopyToArgs;
use crate::providers::gcp::clients::sheets::SheetsSpreadsheetsValuesAppendArgs;
use crate::providers::gcp::clients::sheets::SheetsSpreadsheetsValuesBatchClearArgs;
use crate::providers::gcp::clients::sheets::SheetsSpreadsheetsValuesBatchClearByDataFilterArgs;
use crate::providers::gcp::clients::sheets::SheetsSpreadsheetsValuesBatchGetArgs;
use crate::providers::gcp::clients::sheets::SheetsSpreadsheetsValuesBatchGetByDataFilterArgs;
use crate::providers::gcp::clients::sheets::SheetsSpreadsheetsValuesBatchUpdateArgs;
use crate::providers::gcp::clients::sheets::SheetsSpreadsheetsValuesBatchUpdateByDataFilterArgs;
use crate::providers::gcp::clients::sheets::SheetsSpreadsheetsValuesClearArgs;
use crate::providers::gcp::clients::sheets::SheetsSpreadsheetsValuesGetArgs;
use crate::providers::gcp::clients::sheets::SheetsSpreadsheetsValuesUpdateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// SheetsProvider with automatic state tracking.
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
/// let provider = SheetsProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct SheetsProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> SheetsProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new SheetsProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new SheetsProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Sheets spreadsheets batch update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchUpdateSpreadsheetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sheets_spreadsheets_batch_update(
        &self,
        args: &SheetsSpreadsheetsBatchUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchUpdateSpreadsheetResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sheets_spreadsheets_batch_update_builder(
            &self.http_client,
            &args.spreadsheetId,
        )
        .map_err(ProviderError::Api)?;

        let task = sheets_spreadsheets_batch_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sheets spreadsheets create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Spreadsheet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sheets_spreadsheets_create(
        &self,
        args: &SheetsSpreadsheetsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Spreadsheet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sheets_spreadsheets_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = sheets_spreadsheets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sheets spreadsheets get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Spreadsheet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn sheets_spreadsheets_get(
        &self,
        args: &SheetsSpreadsheetsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Spreadsheet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sheets_spreadsheets_get_builder(
            &self.http_client,
            &args.spreadsheetId,
            &args.excludeTablesInBandedRanges,
            &args.includeGridData,
            &args.ranges,
        )
        .map_err(ProviderError::Api)?;

        let task = sheets_spreadsheets_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sheets spreadsheets get by data filter.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Spreadsheet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn sheets_spreadsheets_get_by_data_filter(
        &self,
        args: &SheetsSpreadsheetsGetByDataFilterArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Spreadsheet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sheets_spreadsheets_get_by_data_filter_builder(
            &self.http_client,
            &args.spreadsheetId,
        )
        .map_err(ProviderError::Api)?;

        let task = sheets_spreadsheets_get_by_data_filter_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sheets spreadsheets developer metadata get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeveloperMetadata result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn sheets_spreadsheets_developer_metadata_get(
        &self,
        args: &SheetsSpreadsheetsDeveloperMetadataGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeveloperMetadata, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sheets_spreadsheets_developer_metadata_get_builder(
            &self.http_client,
            &args.spreadsheetId,
            &args.metadataId,
        )
        .map_err(ProviderError::Api)?;

        let task = sheets_spreadsheets_developer_metadata_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sheets spreadsheets developer metadata search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchDeveloperMetadataResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn sheets_spreadsheets_developer_metadata_search(
        &self,
        args: &SheetsSpreadsheetsDeveloperMetadataSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchDeveloperMetadataResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sheets_spreadsheets_developer_metadata_search_builder(
            &self.http_client,
            &args.spreadsheetId,
        )
        .map_err(ProviderError::Api)?;

        let task = sheets_spreadsheets_developer_metadata_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sheets spreadsheets sheets copy to.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SheetProperties result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sheets_spreadsheets_sheets_copy_to(
        &self,
        args: &SheetsSpreadsheetsSheetsCopyToArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SheetProperties, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sheets_spreadsheets_sheets_copy_to_builder(
            &self.http_client,
            &args.spreadsheetId,
            &args.sheetId,
        )
        .map_err(ProviderError::Api)?;

        let task = sheets_spreadsheets_sheets_copy_to_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sheets spreadsheets values append.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AppendValuesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sheets_spreadsheets_values_append(
        &self,
        args: &SheetsSpreadsheetsValuesAppendArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AppendValuesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sheets_spreadsheets_values_append_builder(
            &self.http_client,
            &args.spreadsheetId,
            &args.range,
            &args.includeValuesInResponse,
            &args.insertDataOption,
            &args.responseDateTimeRenderOption,
            &args.responseValueRenderOption,
            &args.valueInputOption,
        )
        .map_err(ProviderError::Api)?;

        let task = sheets_spreadsheets_values_append_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sheets spreadsheets values batch clear.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchClearValuesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sheets_spreadsheets_values_batch_clear(
        &self,
        args: &SheetsSpreadsheetsValuesBatchClearArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchClearValuesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sheets_spreadsheets_values_batch_clear_builder(
            &self.http_client,
            &args.spreadsheetId,
        )
        .map_err(ProviderError::Api)?;

        let task = sheets_spreadsheets_values_batch_clear_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sheets spreadsheets values batch clear by data filter.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchClearValuesByDataFilterResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sheets_spreadsheets_values_batch_clear_by_data_filter(
        &self,
        args: &SheetsSpreadsheetsValuesBatchClearByDataFilterArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchClearValuesByDataFilterResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sheets_spreadsheets_values_batch_clear_by_data_filter_builder(
            &self.http_client,
            &args.spreadsheetId,
        )
        .map_err(ProviderError::Api)?;

        let task = sheets_spreadsheets_values_batch_clear_by_data_filter_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sheets spreadsheets values batch get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchGetValuesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn sheets_spreadsheets_values_batch_get(
        &self,
        args: &SheetsSpreadsheetsValuesBatchGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchGetValuesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sheets_spreadsheets_values_batch_get_builder(
            &self.http_client,
            &args.spreadsheetId,
            &args.dateTimeRenderOption,
            &args.majorDimension,
            &args.ranges,
            &args.valueRenderOption,
        )
        .map_err(ProviderError::Api)?;

        let task = sheets_spreadsheets_values_batch_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sheets spreadsheets values batch get by data filter.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchGetValuesByDataFilterResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn sheets_spreadsheets_values_batch_get_by_data_filter(
        &self,
        args: &SheetsSpreadsheetsValuesBatchGetByDataFilterArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchGetValuesByDataFilterResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sheets_spreadsheets_values_batch_get_by_data_filter_builder(
            &self.http_client,
            &args.spreadsheetId,
        )
        .map_err(ProviderError::Api)?;

        let task = sheets_spreadsheets_values_batch_get_by_data_filter_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sheets spreadsheets values batch update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchUpdateValuesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sheets_spreadsheets_values_batch_update(
        &self,
        args: &SheetsSpreadsheetsValuesBatchUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchUpdateValuesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sheets_spreadsheets_values_batch_update_builder(
            &self.http_client,
            &args.spreadsheetId,
        )
        .map_err(ProviderError::Api)?;

        let task = sheets_spreadsheets_values_batch_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sheets spreadsheets values batch update by data filter.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchUpdateValuesByDataFilterResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sheets_spreadsheets_values_batch_update_by_data_filter(
        &self,
        args: &SheetsSpreadsheetsValuesBatchUpdateByDataFilterArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchUpdateValuesByDataFilterResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sheets_spreadsheets_values_batch_update_by_data_filter_builder(
            &self.http_client,
            &args.spreadsheetId,
        )
        .map_err(ProviderError::Api)?;

        let task = sheets_spreadsheets_values_batch_update_by_data_filter_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sheets spreadsheets values clear.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ClearValuesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sheets_spreadsheets_values_clear(
        &self,
        args: &SheetsSpreadsheetsValuesClearArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ClearValuesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sheets_spreadsheets_values_clear_builder(
            &self.http_client,
            &args.spreadsheetId,
            &args.range,
        )
        .map_err(ProviderError::Api)?;

        let task = sheets_spreadsheets_values_clear_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sheets spreadsheets values get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ValueRange result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn sheets_spreadsheets_values_get(
        &self,
        args: &SheetsSpreadsheetsValuesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ValueRange, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sheets_spreadsheets_values_get_builder(
            &self.http_client,
            &args.spreadsheetId,
            &args.range,
            &args.dateTimeRenderOption,
            &args.majorDimension,
            &args.valueRenderOption,
        )
        .map_err(ProviderError::Api)?;

        let task = sheets_spreadsheets_values_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sheets spreadsheets values update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UpdateValuesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sheets_spreadsheets_values_update(
        &self,
        args: &SheetsSpreadsheetsValuesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UpdateValuesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sheets_spreadsheets_values_update_builder(
            &self.http_client,
            &args.spreadsheetId,
            &args.range,
            &args.includeValuesInResponse,
            &args.responseDateTimeRenderOption,
            &args.responseValueRenderOption,
            &args.valueInputOption,
        )
        .map_err(ProviderError::Api)?;

        let task = sheets_spreadsheets_values_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
