//! AdsenseProvider - State-aware adsense API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       adsense API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::adsense::{
    adsense_accounts_get_builder, adsense_accounts_get_task,
    adsense_accounts_get_ad_blocking_recovery_tag_builder, adsense_accounts_get_ad_blocking_recovery_tag_task,
    adsense_accounts_list_builder, adsense_accounts_list_task,
    adsense_accounts_list_child_accounts_builder, adsense_accounts_list_child_accounts_task,
    adsense_accounts_adclients_get_builder, adsense_accounts_adclients_get_task,
    adsense_accounts_adclients_get_adcode_builder, adsense_accounts_adclients_get_adcode_task,
    adsense_accounts_adclients_list_builder, adsense_accounts_adclients_list_task,
    adsense_accounts_adclients_adunits_create_builder, adsense_accounts_adclients_adunits_create_task,
    adsense_accounts_adclients_adunits_get_builder, adsense_accounts_adclients_adunits_get_task,
    adsense_accounts_adclients_adunits_get_adcode_builder, adsense_accounts_adclients_adunits_get_adcode_task,
    adsense_accounts_adclients_adunits_list_builder, adsense_accounts_adclients_adunits_list_task,
    adsense_accounts_adclients_adunits_list_linked_custom_channels_builder, adsense_accounts_adclients_adunits_list_linked_custom_channels_task,
    adsense_accounts_adclients_adunits_patch_builder, adsense_accounts_adclients_adunits_patch_task,
    adsense_accounts_adclients_customchannels_create_builder, adsense_accounts_adclients_customchannels_create_task,
    adsense_accounts_adclients_customchannels_delete_builder, adsense_accounts_adclients_customchannels_delete_task,
    adsense_accounts_adclients_customchannels_get_builder, adsense_accounts_adclients_customchannels_get_task,
    adsense_accounts_adclients_customchannels_list_builder, adsense_accounts_adclients_customchannels_list_task,
    adsense_accounts_adclients_customchannels_list_linked_ad_units_builder, adsense_accounts_adclients_customchannels_list_linked_ad_units_task,
    adsense_accounts_adclients_customchannels_patch_builder, adsense_accounts_adclients_customchannels_patch_task,
    adsense_accounts_adclients_urlchannels_get_builder, adsense_accounts_adclients_urlchannels_get_task,
    adsense_accounts_adclients_urlchannels_list_builder, adsense_accounts_adclients_urlchannels_list_task,
    adsense_accounts_alerts_list_builder, adsense_accounts_alerts_list_task,
    adsense_accounts_payments_list_builder, adsense_accounts_payments_list_task,
    adsense_accounts_policy_issues_get_builder, adsense_accounts_policy_issues_get_task,
    adsense_accounts_policy_issues_list_builder, adsense_accounts_policy_issues_list_task,
    adsense_accounts_reports_generate_builder, adsense_accounts_reports_generate_task,
    adsense_accounts_reports_generate_csv_builder, adsense_accounts_reports_generate_csv_task,
    adsense_accounts_reports_get_saved_builder, adsense_accounts_reports_get_saved_task,
    adsense_accounts_reports_saved_generate_builder, adsense_accounts_reports_saved_generate_task,
    adsense_accounts_reports_saved_generate_csv_builder, adsense_accounts_reports_saved_generate_csv_task,
    adsense_accounts_reports_saved_list_builder, adsense_accounts_reports_saved_list_task,
    adsense_accounts_sites_get_builder, adsense_accounts_sites_get_task,
    adsense_accounts_sites_list_builder, adsense_accounts_sites_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::adsense::Account;
use crate::providers::gcp::clients::adsense::AdBlockingRecoveryTag;
use crate::providers::gcp::clients::adsense::AdClient;
use crate::providers::gcp::clients::adsense::AdClientAdCode;
use crate::providers::gcp::clients::adsense::AdUnit;
use crate::providers::gcp::clients::adsense::AdUnitAdCode;
use crate::providers::gcp::clients::adsense::CustomChannel;
use crate::providers::gcp::clients::adsense::Empty;
use crate::providers::gcp::clients::adsense::HttpBody;
use crate::providers::gcp::clients::adsense::ListAccountsResponse;
use crate::providers::gcp::clients::adsense::ListAdClientsResponse;
use crate::providers::gcp::clients::adsense::ListAdUnitsResponse;
use crate::providers::gcp::clients::adsense::ListAlertsResponse;
use crate::providers::gcp::clients::adsense::ListChildAccountsResponse;
use crate::providers::gcp::clients::adsense::ListCustomChannelsResponse;
use crate::providers::gcp::clients::adsense::ListLinkedAdUnitsResponse;
use crate::providers::gcp::clients::adsense::ListLinkedCustomChannelsResponse;
use crate::providers::gcp::clients::adsense::ListPaymentsResponse;
use crate::providers::gcp::clients::adsense::ListPolicyIssuesResponse;
use crate::providers::gcp::clients::adsense::ListSavedReportsResponse;
use crate::providers::gcp::clients::adsense::ListSitesResponse;
use crate::providers::gcp::clients::adsense::ListUrlChannelsResponse;
use crate::providers::gcp::clients::adsense::PolicyIssue;
use crate::providers::gcp::clients::adsense::ReportResult;
use crate::providers::gcp::clients::adsense::SavedReport;
use crate::providers::gcp::clients::adsense::Site;
use crate::providers::gcp::clients::adsense::UrlChannel;
use crate::providers::gcp::clients::adsense::AdsenseAccountsAdclientsAdunitsCreateArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsAdclientsAdunitsGetAdcodeArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsAdclientsAdunitsGetArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsAdclientsAdunitsListArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsAdclientsAdunitsListLinkedCustomChannelsArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsAdclientsAdunitsPatchArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsAdclientsCustomchannelsCreateArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsAdclientsCustomchannelsDeleteArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsAdclientsCustomchannelsGetArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsAdclientsCustomchannelsListArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsAdclientsCustomchannelsListLinkedAdUnitsArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsAdclientsCustomchannelsPatchArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsAdclientsGetAdcodeArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsAdclientsGetArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsAdclientsListArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsAdclientsUrlchannelsGetArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsAdclientsUrlchannelsListArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsAlertsListArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsGetAdBlockingRecoveryTagArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsGetArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsListArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsListChildAccountsArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsPaymentsListArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsPolicyIssuesGetArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsPolicyIssuesListArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsReportsGenerateArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsReportsGenerateCsvArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsReportsGetSavedArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsReportsSavedGenerateArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsReportsSavedGenerateCsvArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsReportsSavedListArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsSitesGetArgs;
use crate::providers::gcp::clients::adsense::AdsenseAccountsSitesListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// AdsenseProvider with automatic state tracking.
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
/// let provider = AdsenseProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct AdsenseProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> AdsenseProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new AdsenseProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new AdsenseProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Adsense accounts get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Account result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adsense_accounts_get(
        &self,
        args: &AdsenseAccountsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Account, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts get ad blocking recovery tag.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdBlockingRecoveryTag result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adsense_accounts_get_ad_blocking_recovery_tag(
        &self,
        args: &AdsenseAccountsGetAdBlockingRecoveryTagArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdBlockingRecoveryTag, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_get_ad_blocking_recovery_tag_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_get_ad_blocking_recovery_tag_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAccountsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adsense_accounts_list(
        &self,
        args: &AdsenseAccountsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAccountsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_list_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts list child accounts.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListChildAccountsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adsense_accounts_list_child_accounts(
        &self,
        args: &AdsenseAccountsListChildAccountsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListChildAccountsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_list_child_accounts_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_list_child_accounts_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts adclients get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdClient result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adsense_accounts_adclients_get(
        &self,
        args: &AdsenseAccountsAdclientsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdClient, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_adclients_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_adclients_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts adclients get adcode.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdClientAdCode result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adsense_accounts_adclients_get_adcode(
        &self,
        args: &AdsenseAccountsAdclientsGetAdcodeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdClientAdCode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_adclients_get_adcode_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_adclients_get_adcode_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts adclients list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAdClientsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adsense_accounts_adclients_list(
        &self,
        args: &AdsenseAccountsAdclientsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAdClientsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_adclients_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_adclients_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts adclients adunits create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdUnit result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn adsense_accounts_adclients_adunits_create(
        &self,
        args: &AdsenseAccountsAdclientsAdunitsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdUnit, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_adclients_adunits_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_adclients_adunits_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts adclients adunits get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdUnit result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adsense_accounts_adclients_adunits_get(
        &self,
        args: &AdsenseAccountsAdclientsAdunitsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdUnit, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_adclients_adunits_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_adclients_adunits_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts adclients adunits get adcode.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdUnitAdCode result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adsense_accounts_adclients_adunits_get_adcode(
        &self,
        args: &AdsenseAccountsAdclientsAdunitsGetAdcodeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdUnitAdCode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_adclients_adunits_get_adcode_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_adclients_adunits_get_adcode_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts adclients adunits list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAdUnitsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adsense_accounts_adclients_adunits_list(
        &self,
        args: &AdsenseAccountsAdclientsAdunitsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAdUnitsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_adclients_adunits_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_adclients_adunits_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts adclients adunits list linked custom channels.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLinkedCustomChannelsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adsense_accounts_adclients_adunits_list_linked_custom_channels(
        &self,
        args: &AdsenseAccountsAdclientsAdunitsListLinkedCustomChannelsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLinkedCustomChannelsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_adclients_adunits_list_linked_custom_channels_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_adclients_adunits_list_linked_custom_channels_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts adclients adunits patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdUnit result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn adsense_accounts_adclients_adunits_patch(
        &self,
        args: &AdsenseAccountsAdclientsAdunitsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdUnit, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_adclients_adunits_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_adclients_adunits_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts adclients customchannels create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomChannel result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn adsense_accounts_adclients_customchannels_create(
        &self,
        args: &AdsenseAccountsAdclientsCustomchannelsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomChannel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_adclients_customchannels_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_adclients_customchannels_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts adclients customchannels delete.
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
    pub fn adsense_accounts_adclients_customchannels_delete(
        &self,
        args: &AdsenseAccountsAdclientsCustomchannelsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_adclients_customchannels_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_adclients_customchannels_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts adclients customchannels get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomChannel result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adsense_accounts_adclients_customchannels_get(
        &self,
        args: &AdsenseAccountsAdclientsCustomchannelsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomChannel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_adclients_customchannels_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_adclients_customchannels_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts adclients customchannels list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCustomChannelsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adsense_accounts_adclients_customchannels_list(
        &self,
        args: &AdsenseAccountsAdclientsCustomchannelsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCustomChannelsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_adclients_customchannels_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_adclients_customchannels_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts adclients customchannels list linked ad units.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLinkedAdUnitsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adsense_accounts_adclients_customchannels_list_linked_ad_units(
        &self,
        args: &AdsenseAccountsAdclientsCustomchannelsListLinkedAdUnitsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLinkedAdUnitsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_adclients_customchannels_list_linked_ad_units_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_adclients_customchannels_list_linked_ad_units_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts adclients customchannels patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomChannel result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn adsense_accounts_adclients_customchannels_patch(
        &self,
        args: &AdsenseAccountsAdclientsCustomchannelsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomChannel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_adclients_customchannels_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_adclients_customchannels_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts adclients urlchannels get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UrlChannel result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adsense_accounts_adclients_urlchannels_get(
        &self,
        args: &AdsenseAccountsAdclientsUrlchannelsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UrlChannel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_adclients_urlchannels_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_adclients_urlchannels_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts adclients urlchannels list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListUrlChannelsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adsense_accounts_adclients_urlchannels_list(
        &self,
        args: &AdsenseAccountsAdclientsUrlchannelsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListUrlChannelsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_adclients_urlchannels_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_adclients_urlchannels_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts alerts list.
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
    pub fn adsense_accounts_alerts_list(
        &self,
        args: &AdsenseAccountsAlertsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAlertsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_alerts_list_builder(
            &self.http_client,
            &args.parent,
            &args.languageCode,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_alerts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts payments list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPaymentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adsense_accounts_payments_list(
        &self,
        args: &AdsenseAccountsPaymentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPaymentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_payments_list_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_payments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts policy issues get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PolicyIssue result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adsense_accounts_policy_issues_get(
        &self,
        args: &AdsenseAccountsPolicyIssuesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PolicyIssue, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_policy_issues_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_policy_issues_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts policy issues list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPolicyIssuesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adsense_accounts_policy_issues_list(
        &self,
        args: &AdsenseAccountsPolicyIssuesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPolicyIssuesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_policy_issues_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_policy_issues_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts reports generate.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReportResult result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adsense_accounts_reports_generate(
        &self,
        args: &AdsenseAccountsReportsGenerateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReportResult, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_reports_generate_builder(
            &self.http_client,
            &args.account,
            &args.currencyCode,
            &args.dateRange,
            &args.dimensions,
            &args.endDate_day,
            &args.endDate_month,
            &args.endDate_year,
            &args.filters,
            &args.languageCode,
            &args.limit,
            &args.metrics,
            &args.orderBy,
            &args.reportingTimeZone,
            &args.startDate_day,
            &args.startDate_month,
            &args.startDate_year,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_reports_generate_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts reports generate csv.
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
    pub fn adsense_accounts_reports_generate_csv(
        &self,
        args: &AdsenseAccountsReportsGenerateCsvArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_reports_generate_csv_builder(
            &self.http_client,
            &args.account,
            &args.currencyCode,
            &args.dateRange,
            &args.dimensions,
            &args.endDate_day,
            &args.endDate_month,
            &args.endDate_year,
            &args.filters,
            &args.languageCode,
            &args.limit,
            &args.metrics,
            &args.orderBy,
            &args.reportingTimeZone,
            &args.startDate_day,
            &args.startDate_month,
            &args.startDate_year,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_reports_generate_csv_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts reports get saved.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SavedReport result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adsense_accounts_reports_get_saved(
        &self,
        args: &AdsenseAccountsReportsGetSavedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SavedReport, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_reports_get_saved_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_reports_get_saved_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts reports saved generate.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReportResult result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adsense_accounts_reports_saved_generate(
        &self,
        args: &AdsenseAccountsReportsSavedGenerateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReportResult, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_reports_saved_generate_builder(
            &self.http_client,
            &args.name,
            &args.currencyCode,
            &args.dateRange,
            &args.endDate_day,
            &args.endDate_month,
            &args.endDate_year,
            &args.languageCode,
            &args.reportingTimeZone,
            &args.startDate_day,
            &args.startDate_month,
            &args.startDate_year,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_reports_saved_generate_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts reports saved generate csv.
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
    pub fn adsense_accounts_reports_saved_generate_csv(
        &self,
        args: &AdsenseAccountsReportsSavedGenerateCsvArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_reports_saved_generate_csv_builder(
            &self.http_client,
            &args.name,
            &args.currencyCode,
            &args.dateRange,
            &args.endDate_day,
            &args.endDate_month,
            &args.endDate_year,
            &args.languageCode,
            &args.reportingTimeZone,
            &args.startDate_day,
            &args.startDate_month,
            &args.startDate_year,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_reports_saved_generate_csv_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts reports saved list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSavedReportsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adsense_accounts_reports_saved_list(
        &self,
        args: &AdsenseAccountsReportsSavedListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSavedReportsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_reports_saved_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_reports_saved_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts sites get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Site result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adsense_accounts_sites_get(
        &self,
        args: &AdsenseAccountsSitesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Site, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_sites_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_sites_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Adsense accounts sites list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSitesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn adsense_accounts_sites_list(
        &self,
        args: &AdsenseAccountsSitesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSitesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = adsense_accounts_sites_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = adsense_accounts_sites_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
