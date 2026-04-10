//! FirebaseappcheckProvider - State-aware firebaseappcheck API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       firebaseappcheck API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::firebaseappcheck::{
    firebaseappcheck_oauth_clients_exchange_app_attest_assertion_builder, firebaseappcheck_oauth_clients_exchange_app_attest_assertion_task,
    firebaseappcheck_oauth_clients_exchange_app_attest_attestation_builder, firebaseappcheck_oauth_clients_exchange_app_attest_attestation_task,
    firebaseappcheck_oauth_clients_exchange_debug_token_builder, firebaseappcheck_oauth_clients_exchange_debug_token_task,
    firebaseappcheck_oauth_clients_generate_app_attest_challenge_builder, firebaseappcheck_oauth_clients_generate_app_attest_challenge_task,
    firebaseappcheck_projects_apps_exchange_app_attest_assertion_builder, firebaseappcheck_projects_apps_exchange_app_attest_assertion_task,
    firebaseappcheck_projects_apps_exchange_app_attest_attestation_builder, firebaseappcheck_projects_apps_exchange_app_attest_attestation_task,
    firebaseappcheck_projects_apps_exchange_custom_token_builder, firebaseappcheck_projects_apps_exchange_custom_token_task,
    firebaseappcheck_projects_apps_exchange_debug_token_builder, firebaseappcheck_projects_apps_exchange_debug_token_task,
    firebaseappcheck_projects_apps_exchange_device_check_token_builder, firebaseappcheck_projects_apps_exchange_device_check_token_task,
    firebaseappcheck_projects_apps_exchange_play_integrity_token_builder, firebaseappcheck_projects_apps_exchange_play_integrity_token_task,
    firebaseappcheck_projects_apps_exchange_recaptcha_enterprise_token_builder, firebaseappcheck_projects_apps_exchange_recaptcha_enterprise_token_task,
    firebaseappcheck_projects_apps_exchange_recaptcha_v3_token_builder, firebaseappcheck_projects_apps_exchange_recaptcha_v3_token_task,
    firebaseappcheck_projects_apps_exchange_safety_net_token_builder, firebaseappcheck_projects_apps_exchange_safety_net_token_task,
    firebaseappcheck_projects_apps_generate_app_attest_challenge_builder, firebaseappcheck_projects_apps_generate_app_attest_challenge_task,
    firebaseappcheck_projects_apps_generate_play_integrity_challenge_builder, firebaseappcheck_projects_apps_generate_play_integrity_challenge_task,
    firebaseappcheck_projects_apps_app_attest_config_patch_builder, firebaseappcheck_projects_apps_app_attest_config_patch_task,
    firebaseappcheck_projects_apps_debug_tokens_create_builder, firebaseappcheck_projects_apps_debug_tokens_create_task,
    firebaseappcheck_projects_apps_debug_tokens_delete_builder, firebaseappcheck_projects_apps_debug_tokens_delete_task,
    firebaseappcheck_projects_apps_debug_tokens_patch_builder, firebaseappcheck_projects_apps_debug_tokens_patch_task,
    firebaseappcheck_projects_apps_device_check_config_patch_builder, firebaseappcheck_projects_apps_device_check_config_patch_task,
    firebaseappcheck_projects_apps_play_integrity_config_patch_builder, firebaseappcheck_projects_apps_play_integrity_config_patch_task,
    firebaseappcheck_projects_apps_recaptcha_enterprise_config_patch_builder, firebaseappcheck_projects_apps_recaptcha_enterprise_config_patch_task,
    firebaseappcheck_projects_apps_recaptcha_v3_config_patch_builder, firebaseappcheck_projects_apps_recaptcha_v3_config_patch_task,
    firebaseappcheck_projects_apps_safety_net_config_patch_builder, firebaseappcheck_projects_apps_safety_net_config_patch_task,
    firebaseappcheck_projects_services_batch_update_builder, firebaseappcheck_projects_services_batch_update_task,
    firebaseappcheck_projects_services_patch_builder, firebaseappcheck_projects_services_patch_task,
    firebaseappcheck_projects_services_resource_policies_batch_update_builder, firebaseappcheck_projects_services_resource_policies_batch_update_task,
    firebaseappcheck_projects_services_resource_policies_create_builder, firebaseappcheck_projects_services_resource_policies_create_task,
    firebaseappcheck_projects_services_resource_policies_delete_builder, firebaseappcheck_projects_services_resource_policies_delete_task,
    firebaseappcheck_projects_services_resource_policies_patch_builder, firebaseappcheck_projects_services_resource_policies_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::firebaseappcheck::GoogleFirebaseAppcheckV1AppAttestConfig;
use crate::providers::gcp::clients::firebaseappcheck::GoogleFirebaseAppcheckV1AppCheckToken;
use crate::providers::gcp::clients::firebaseappcheck::GoogleFirebaseAppcheckV1BatchUpdateResourcePoliciesResponse;
use crate::providers::gcp::clients::firebaseappcheck::GoogleFirebaseAppcheckV1BatchUpdateServicesResponse;
use crate::providers::gcp::clients::firebaseappcheck::GoogleFirebaseAppcheckV1DebugToken;
use crate::providers::gcp::clients::firebaseappcheck::GoogleFirebaseAppcheckV1DeviceCheckConfig;
use crate::providers::gcp::clients::firebaseappcheck::GoogleFirebaseAppcheckV1ExchangeAppAttestAttestationResponse;
use crate::providers::gcp::clients::firebaseappcheck::GoogleFirebaseAppcheckV1GenerateAppAttestChallengeResponse;
use crate::providers::gcp::clients::firebaseappcheck::GoogleFirebaseAppcheckV1GeneratePlayIntegrityChallengeResponse;
use crate::providers::gcp::clients::firebaseappcheck::GoogleFirebaseAppcheckV1PlayIntegrityConfig;
use crate::providers::gcp::clients::firebaseappcheck::GoogleFirebaseAppcheckV1RecaptchaEnterpriseConfig;
use crate::providers::gcp::clients::firebaseappcheck::GoogleFirebaseAppcheckV1RecaptchaV3Config;
use crate::providers::gcp::clients::firebaseappcheck::GoogleFirebaseAppcheckV1ResourcePolicy;
use crate::providers::gcp::clients::firebaseappcheck::GoogleFirebaseAppcheckV1SafetyNetConfig;
use crate::providers::gcp::clients::firebaseappcheck::GoogleFirebaseAppcheckV1Service;
use crate::providers::gcp::clients::firebaseappcheck::GoogleProtobufEmpty;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckOauthClientsExchangeAppAttestAssertionArgs;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckOauthClientsExchangeAppAttestAttestationArgs;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckOauthClientsExchangeDebugTokenArgs;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckOauthClientsGenerateAppAttestChallengeArgs;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckProjectsAppsAppAttestConfigPatchArgs;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckProjectsAppsDebugTokensCreateArgs;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckProjectsAppsDebugTokensDeleteArgs;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckProjectsAppsDebugTokensPatchArgs;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckProjectsAppsDeviceCheckConfigPatchArgs;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckProjectsAppsExchangeAppAttestAssertionArgs;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckProjectsAppsExchangeAppAttestAttestationArgs;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckProjectsAppsExchangeCustomTokenArgs;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckProjectsAppsExchangeDebugTokenArgs;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckProjectsAppsExchangeDeviceCheckTokenArgs;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckProjectsAppsExchangePlayIntegrityTokenArgs;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckProjectsAppsExchangeRecaptchaEnterpriseTokenArgs;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckProjectsAppsExchangeRecaptchaV3TokenArgs;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckProjectsAppsExchangeSafetyNetTokenArgs;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckProjectsAppsGenerateAppAttestChallengeArgs;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckProjectsAppsGeneratePlayIntegrityChallengeArgs;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckProjectsAppsPlayIntegrityConfigPatchArgs;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckProjectsAppsRecaptchaEnterpriseConfigPatchArgs;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckProjectsAppsRecaptchaV3ConfigPatchArgs;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckProjectsAppsSafetyNetConfigPatchArgs;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckProjectsServicesBatchUpdateArgs;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckProjectsServicesPatchArgs;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckProjectsServicesResourcePoliciesBatchUpdateArgs;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckProjectsServicesResourcePoliciesCreateArgs;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckProjectsServicesResourcePoliciesDeleteArgs;
use crate::providers::gcp::clients::firebaseappcheck::FirebaseappcheckProjectsServicesResourcePoliciesPatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// FirebaseappcheckProvider with automatic state tracking.
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
/// let provider = FirebaseappcheckProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct FirebaseappcheckProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> FirebaseappcheckProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new FirebaseappcheckProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Firebaseappcheck oauth clients exchange app attest assertion.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppcheckV1AppCheckToken result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_oauth_clients_exchange_app_attest_assertion(
        &self,
        args: &FirebaseappcheckOauthClientsExchangeAppAttestAssertionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppcheckV1AppCheckToken, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_oauth_clients_exchange_app_attest_assertion_builder(
            &self.http_client,
            &args.app,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_oauth_clients_exchange_app_attest_assertion_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappcheck oauth clients exchange app attest attestation.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppcheckV1ExchangeAppAttestAttestationResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_oauth_clients_exchange_app_attest_attestation(
        &self,
        args: &FirebaseappcheckOauthClientsExchangeAppAttestAttestationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppcheckV1ExchangeAppAttestAttestationResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_oauth_clients_exchange_app_attest_attestation_builder(
            &self.http_client,
            &args.app,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_oauth_clients_exchange_app_attest_attestation_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappcheck oauth clients exchange debug token.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppcheckV1AppCheckToken result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_oauth_clients_exchange_debug_token(
        &self,
        args: &FirebaseappcheckOauthClientsExchangeDebugTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppcheckV1AppCheckToken, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_oauth_clients_exchange_debug_token_builder(
            &self.http_client,
            &args.app,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_oauth_clients_exchange_debug_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappcheck oauth clients generate app attest challenge.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppcheckV1GenerateAppAttestChallengeResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_oauth_clients_generate_app_attest_challenge(
        &self,
        args: &FirebaseappcheckOauthClientsGenerateAppAttestChallengeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppcheckV1GenerateAppAttestChallengeResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_oauth_clients_generate_app_attest_challenge_builder(
            &self.http_client,
            &args.app,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_oauth_clients_generate_app_attest_challenge_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappcheck projects apps exchange app attest assertion.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppcheckV1AppCheckToken result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_projects_apps_exchange_app_attest_assertion(
        &self,
        args: &FirebaseappcheckProjectsAppsExchangeAppAttestAssertionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppcheckV1AppCheckToken, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_projects_apps_exchange_app_attest_assertion_builder(
            &self.http_client,
            &args.app,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_projects_apps_exchange_app_attest_assertion_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappcheck projects apps exchange app attest attestation.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppcheckV1ExchangeAppAttestAttestationResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_projects_apps_exchange_app_attest_attestation(
        &self,
        args: &FirebaseappcheckProjectsAppsExchangeAppAttestAttestationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppcheckV1ExchangeAppAttestAttestationResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_projects_apps_exchange_app_attest_attestation_builder(
            &self.http_client,
            &args.app,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_projects_apps_exchange_app_attest_attestation_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappcheck projects apps exchange custom token.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppcheckV1AppCheckToken result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_projects_apps_exchange_custom_token(
        &self,
        args: &FirebaseappcheckProjectsAppsExchangeCustomTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppcheckV1AppCheckToken, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_projects_apps_exchange_custom_token_builder(
            &self.http_client,
            &args.app,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_projects_apps_exchange_custom_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappcheck projects apps exchange debug token.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppcheckV1AppCheckToken result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_projects_apps_exchange_debug_token(
        &self,
        args: &FirebaseappcheckProjectsAppsExchangeDebugTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppcheckV1AppCheckToken, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_projects_apps_exchange_debug_token_builder(
            &self.http_client,
            &args.app,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_projects_apps_exchange_debug_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappcheck projects apps exchange device check token.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppcheckV1AppCheckToken result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_projects_apps_exchange_device_check_token(
        &self,
        args: &FirebaseappcheckProjectsAppsExchangeDeviceCheckTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppcheckV1AppCheckToken, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_projects_apps_exchange_device_check_token_builder(
            &self.http_client,
            &args.app,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_projects_apps_exchange_device_check_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappcheck projects apps exchange play integrity token.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppcheckV1AppCheckToken result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_projects_apps_exchange_play_integrity_token(
        &self,
        args: &FirebaseappcheckProjectsAppsExchangePlayIntegrityTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppcheckV1AppCheckToken, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_projects_apps_exchange_play_integrity_token_builder(
            &self.http_client,
            &args.app,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_projects_apps_exchange_play_integrity_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappcheck projects apps exchange recaptcha enterprise token.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppcheckV1AppCheckToken result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_projects_apps_exchange_recaptcha_enterprise_token(
        &self,
        args: &FirebaseappcheckProjectsAppsExchangeRecaptchaEnterpriseTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppcheckV1AppCheckToken, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_projects_apps_exchange_recaptcha_enterprise_token_builder(
            &self.http_client,
            &args.app,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_projects_apps_exchange_recaptcha_enterprise_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappcheck projects apps exchange recaptcha v3 token.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppcheckV1AppCheckToken result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_projects_apps_exchange_recaptcha_v3_token(
        &self,
        args: &FirebaseappcheckProjectsAppsExchangeRecaptchaV3TokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppcheckV1AppCheckToken, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_projects_apps_exchange_recaptcha_v3_token_builder(
            &self.http_client,
            &args.app,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_projects_apps_exchange_recaptcha_v3_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappcheck projects apps exchange safety net token.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppcheckV1AppCheckToken result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_projects_apps_exchange_safety_net_token(
        &self,
        args: &FirebaseappcheckProjectsAppsExchangeSafetyNetTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppcheckV1AppCheckToken, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_projects_apps_exchange_safety_net_token_builder(
            &self.http_client,
            &args.app,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_projects_apps_exchange_safety_net_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappcheck projects apps generate app attest challenge.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppcheckV1GenerateAppAttestChallengeResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_projects_apps_generate_app_attest_challenge(
        &self,
        args: &FirebaseappcheckProjectsAppsGenerateAppAttestChallengeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppcheckV1GenerateAppAttestChallengeResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_projects_apps_generate_app_attest_challenge_builder(
            &self.http_client,
            &args.app,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_projects_apps_generate_app_attest_challenge_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappcheck projects apps generate play integrity challenge.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppcheckV1GeneratePlayIntegrityChallengeResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_projects_apps_generate_play_integrity_challenge(
        &self,
        args: &FirebaseappcheckProjectsAppsGeneratePlayIntegrityChallengeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppcheckV1GeneratePlayIntegrityChallengeResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_projects_apps_generate_play_integrity_challenge_builder(
            &self.http_client,
            &args.app,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_projects_apps_generate_play_integrity_challenge_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappcheck projects apps app attest config patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppcheckV1AppAttestConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_projects_apps_app_attest_config_patch(
        &self,
        args: &FirebaseappcheckProjectsAppsAppAttestConfigPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppcheckV1AppAttestConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_projects_apps_app_attest_config_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_projects_apps_app_attest_config_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappcheck projects apps debug tokens create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppcheckV1DebugToken result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_projects_apps_debug_tokens_create(
        &self,
        args: &FirebaseappcheckProjectsAppsDebugTokensCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppcheckV1DebugToken, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_projects_apps_debug_tokens_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_projects_apps_debug_tokens_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappcheck projects apps debug tokens delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_projects_apps_debug_tokens_delete(
        &self,
        args: &FirebaseappcheckProjectsAppsDebugTokensDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_projects_apps_debug_tokens_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_projects_apps_debug_tokens_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappcheck projects apps debug tokens patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppcheckV1DebugToken result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_projects_apps_debug_tokens_patch(
        &self,
        args: &FirebaseappcheckProjectsAppsDebugTokensPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppcheckV1DebugToken, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_projects_apps_debug_tokens_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_projects_apps_debug_tokens_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappcheck projects apps device check config patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppcheckV1DeviceCheckConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_projects_apps_device_check_config_patch(
        &self,
        args: &FirebaseappcheckProjectsAppsDeviceCheckConfigPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppcheckV1DeviceCheckConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_projects_apps_device_check_config_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_projects_apps_device_check_config_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappcheck projects apps play integrity config patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppcheckV1PlayIntegrityConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_projects_apps_play_integrity_config_patch(
        &self,
        args: &FirebaseappcheckProjectsAppsPlayIntegrityConfigPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppcheckV1PlayIntegrityConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_projects_apps_play_integrity_config_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_projects_apps_play_integrity_config_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappcheck projects apps recaptcha enterprise config patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppcheckV1RecaptchaEnterpriseConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_projects_apps_recaptcha_enterprise_config_patch(
        &self,
        args: &FirebaseappcheckProjectsAppsRecaptchaEnterpriseConfigPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppcheckV1RecaptchaEnterpriseConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_projects_apps_recaptcha_enterprise_config_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_projects_apps_recaptcha_enterprise_config_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappcheck projects apps recaptcha v3 config patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppcheckV1RecaptchaV3Config result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_projects_apps_recaptcha_v3_config_patch(
        &self,
        args: &FirebaseappcheckProjectsAppsRecaptchaV3ConfigPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppcheckV1RecaptchaV3Config, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_projects_apps_recaptcha_v3_config_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_projects_apps_recaptcha_v3_config_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappcheck projects apps safety net config patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppcheckV1SafetyNetConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_projects_apps_safety_net_config_patch(
        &self,
        args: &FirebaseappcheckProjectsAppsSafetyNetConfigPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppcheckV1SafetyNetConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_projects_apps_safety_net_config_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_projects_apps_safety_net_config_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappcheck projects services batch update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppcheckV1BatchUpdateServicesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_projects_services_batch_update(
        &self,
        args: &FirebaseappcheckProjectsServicesBatchUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppcheckV1BatchUpdateServicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_projects_services_batch_update_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_projects_services_batch_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappcheck projects services patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppcheckV1Service result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_projects_services_patch(
        &self,
        args: &FirebaseappcheckProjectsServicesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppcheckV1Service, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_projects_services_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_projects_services_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappcheck projects services resource policies batch update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppcheckV1BatchUpdateResourcePoliciesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_projects_services_resource_policies_batch_update(
        &self,
        args: &FirebaseappcheckProjectsServicesResourcePoliciesBatchUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppcheckV1BatchUpdateResourcePoliciesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_projects_services_resource_policies_batch_update_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_projects_services_resource_policies_batch_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappcheck projects services resource policies create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppcheckV1ResourcePolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_projects_services_resource_policies_create(
        &self,
        args: &FirebaseappcheckProjectsServicesResourcePoliciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppcheckV1ResourcePolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_projects_services_resource_policies_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_projects_services_resource_policies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappcheck projects services resource policies delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_projects_services_resource_policies_delete(
        &self,
        args: &FirebaseappcheckProjectsServicesResourcePoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_projects_services_resource_policies_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_projects_services_resource_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseappcheck projects services resource policies patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirebaseAppcheckV1ResourcePolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaseappcheck_projects_services_resource_policies_patch(
        &self,
        args: &FirebaseappcheckProjectsServicesResourcePoliciesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirebaseAppcheckV1ResourcePolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseappcheck_projects_services_resource_policies_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseappcheck_projects_services_resource_policies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
