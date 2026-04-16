//! IdentitytoolkitProvider - State-aware identitytoolkit API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       identitytoolkit API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::identitytoolkit::{
    identitytoolkit_relyingparty_create_auth_uri_builder, identitytoolkit_relyingparty_create_auth_uri_task,
    identitytoolkit_relyingparty_delete_account_builder, identitytoolkit_relyingparty_delete_account_task,
    identitytoolkit_relyingparty_download_account_builder, identitytoolkit_relyingparty_download_account_task,
    identitytoolkit_relyingparty_email_link_signin_builder, identitytoolkit_relyingparty_email_link_signin_task,
    identitytoolkit_relyingparty_get_account_info_builder, identitytoolkit_relyingparty_get_account_info_task,
    identitytoolkit_relyingparty_get_oob_confirmation_code_builder, identitytoolkit_relyingparty_get_oob_confirmation_code_task,
    identitytoolkit_relyingparty_get_project_config_builder, identitytoolkit_relyingparty_get_project_config_task,
    identitytoolkit_relyingparty_get_public_keys_builder, identitytoolkit_relyingparty_get_public_keys_task,
    identitytoolkit_relyingparty_get_recaptcha_param_builder, identitytoolkit_relyingparty_get_recaptcha_param_task,
    identitytoolkit_relyingparty_reset_password_builder, identitytoolkit_relyingparty_reset_password_task,
    identitytoolkit_relyingparty_send_verification_code_builder, identitytoolkit_relyingparty_send_verification_code_task,
    identitytoolkit_relyingparty_set_account_info_builder, identitytoolkit_relyingparty_set_account_info_task,
    identitytoolkit_relyingparty_set_project_config_builder, identitytoolkit_relyingparty_set_project_config_task,
    identitytoolkit_relyingparty_sign_out_user_builder, identitytoolkit_relyingparty_sign_out_user_task,
    identitytoolkit_relyingparty_signup_new_user_builder, identitytoolkit_relyingparty_signup_new_user_task,
    identitytoolkit_relyingparty_upload_account_builder, identitytoolkit_relyingparty_upload_account_task,
    identitytoolkit_relyingparty_verify_assertion_builder, identitytoolkit_relyingparty_verify_assertion_task,
    identitytoolkit_relyingparty_verify_custom_token_builder, identitytoolkit_relyingparty_verify_custom_token_task,
    identitytoolkit_relyingparty_verify_password_builder, identitytoolkit_relyingparty_verify_password_task,
    identitytoolkit_relyingparty_verify_phone_number_builder, identitytoolkit_relyingparty_verify_phone_number_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::identitytoolkit::CreateAuthUriResponse;
use crate::providers::gcp::clients::identitytoolkit::DeleteAccountResponse;
use crate::providers::gcp::clients::identitytoolkit::DownloadAccountResponse;
use crate::providers::gcp::clients::identitytoolkit::EmailLinkSigninResponse;
use crate::providers::gcp::clients::identitytoolkit::GetAccountInfoResponse;
use crate::providers::gcp::clients::identitytoolkit::GetOobConfirmationCodeResponse;
use crate::providers::gcp::clients::identitytoolkit::GetRecaptchaParamResponse;
use crate::providers::gcp::clients::identitytoolkit::IdentitytoolkitRelyingpartyGetProjectConfigResponse;
use crate::providers::gcp::clients::identitytoolkit::IdentitytoolkitRelyingpartyGetPublicKeysResponse;
use crate::providers::gcp::clients::identitytoolkit::IdentitytoolkitRelyingpartySendVerificationCodeResponse;
use crate::providers::gcp::clients::identitytoolkit::IdentitytoolkitRelyingpartySetProjectConfigResponse;
use crate::providers::gcp::clients::identitytoolkit::IdentitytoolkitRelyingpartySignOutUserResponse;
use crate::providers::gcp::clients::identitytoolkit::IdentitytoolkitRelyingpartyVerifyPhoneNumberResponse;
use crate::providers::gcp::clients::identitytoolkit::ResetPasswordResponse;
use crate::providers::gcp::clients::identitytoolkit::SetAccountInfoResponse;
use crate::providers::gcp::clients::identitytoolkit::SignupNewUserResponse;
use crate::providers::gcp::clients::identitytoolkit::UploadAccountResponse;
use crate::providers::gcp::clients::identitytoolkit::VerifyAssertionResponse;
use crate::providers::gcp::clients::identitytoolkit::VerifyCustomTokenResponse;
use crate::providers::gcp::clients::identitytoolkit::VerifyPasswordResponse;
use crate::providers::gcp::clients::identitytoolkit::IdentitytoolkitRelyingpartyGetProjectConfigArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// IdentitytoolkitProvider with automatic state tracking.
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
/// let provider = IdentitytoolkitProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct IdentitytoolkitProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> IdentitytoolkitProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new IdentitytoolkitProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new IdentitytoolkitProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Identitytoolkit relyingparty create auth uri.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreateAuthUriResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn identitytoolkit_relyingparty_create_auth_uri(
        &self,
        args: &IdentitytoolkitRelyingpartyCreateAuthUriArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreateAuthUriResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = identitytoolkit_relyingparty_create_auth_uri_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = identitytoolkit_relyingparty_create_auth_uri_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Identitytoolkit relyingparty delete account.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeleteAccountResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn identitytoolkit_relyingparty_delete_account(
        &self,
        args: &IdentitytoolkitRelyingpartyDeleteAccountArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeleteAccountResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = identitytoolkit_relyingparty_delete_account_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = identitytoolkit_relyingparty_delete_account_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Identitytoolkit relyingparty download account.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DownloadAccountResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn identitytoolkit_relyingparty_download_account(
        &self,
        args: &IdentitytoolkitRelyingpartyDownloadAccountArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DownloadAccountResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = identitytoolkit_relyingparty_download_account_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = identitytoolkit_relyingparty_download_account_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Identitytoolkit relyingparty email link signin.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EmailLinkSigninResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn identitytoolkit_relyingparty_email_link_signin(
        &self,
        args: &IdentitytoolkitRelyingpartyEmailLinkSigninArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EmailLinkSigninResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = identitytoolkit_relyingparty_email_link_signin_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = identitytoolkit_relyingparty_email_link_signin_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Identitytoolkit relyingparty get account info.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetAccountInfoResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn identitytoolkit_relyingparty_get_account_info(
        &self,
        args: &IdentitytoolkitRelyingpartyGetAccountInfoArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetAccountInfoResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = identitytoolkit_relyingparty_get_account_info_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = identitytoolkit_relyingparty_get_account_info_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Identitytoolkit relyingparty get oob confirmation code.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetOobConfirmationCodeResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn identitytoolkit_relyingparty_get_oob_confirmation_code(
        &self,
        args: &IdentitytoolkitRelyingpartyGetOobConfirmationCodeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetOobConfirmationCodeResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = identitytoolkit_relyingparty_get_oob_confirmation_code_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = identitytoolkit_relyingparty_get_oob_confirmation_code_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Identitytoolkit relyingparty get project config.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IdentitytoolkitRelyingpartyGetProjectConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn identitytoolkit_relyingparty_get_project_config(
        &self,
        args: &IdentitytoolkitRelyingpartyGetProjectConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IdentitytoolkitRelyingpartyGetProjectConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = identitytoolkit_relyingparty_get_project_config_builder(
            &self.http_client,
            &args.delegatedProjectNumber,
            &args.projectNumber,
        )
        .map_err(ProviderError::Api)?;

        let task = identitytoolkit_relyingparty_get_project_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Identitytoolkit relyingparty get public keys.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IdentitytoolkitRelyingpartyGetPublicKeysResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn identitytoolkit_relyingparty_get_public_keys(
        &self,
        args: &IdentitytoolkitRelyingpartyGetPublicKeysArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IdentitytoolkitRelyingpartyGetPublicKeysResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = identitytoolkit_relyingparty_get_public_keys_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = identitytoolkit_relyingparty_get_public_keys_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Identitytoolkit relyingparty get recaptcha param.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetRecaptchaParamResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn identitytoolkit_relyingparty_get_recaptcha_param(
        &self,
        args: &IdentitytoolkitRelyingpartyGetRecaptchaParamArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetRecaptchaParamResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = identitytoolkit_relyingparty_get_recaptcha_param_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = identitytoolkit_relyingparty_get_recaptcha_param_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Identitytoolkit relyingparty reset password.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ResetPasswordResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn identitytoolkit_relyingparty_reset_password(
        &self,
        args: &IdentitytoolkitRelyingpartyResetPasswordArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ResetPasswordResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = identitytoolkit_relyingparty_reset_password_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = identitytoolkit_relyingparty_reset_password_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Identitytoolkit relyingparty send verification code.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IdentitytoolkitRelyingpartySendVerificationCodeResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn identitytoolkit_relyingparty_send_verification_code(
        &self,
        args: &IdentitytoolkitRelyingpartySendVerificationCodeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IdentitytoolkitRelyingpartySendVerificationCodeResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = identitytoolkit_relyingparty_send_verification_code_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = identitytoolkit_relyingparty_send_verification_code_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Identitytoolkit relyingparty set account info.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SetAccountInfoResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn identitytoolkit_relyingparty_set_account_info(
        &self,
        args: &IdentitytoolkitRelyingpartySetAccountInfoArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SetAccountInfoResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = identitytoolkit_relyingparty_set_account_info_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = identitytoolkit_relyingparty_set_account_info_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Identitytoolkit relyingparty set project config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IdentitytoolkitRelyingpartySetProjectConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn identitytoolkit_relyingparty_set_project_config(
        &self,
        args: &IdentitytoolkitRelyingpartySetProjectConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IdentitytoolkitRelyingpartySetProjectConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = identitytoolkit_relyingparty_set_project_config_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = identitytoolkit_relyingparty_set_project_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Identitytoolkit relyingparty sign out user.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IdentitytoolkitRelyingpartySignOutUserResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn identitytoolkit_relyingparty_sign_out_user(
        &self,
        args: &IdentitytoolkitRelyingpartySignOutUserArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IdentitytoolkitRelyingpartySignOutUserResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = identitytoolkit_relyingparty_sign_out_user_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = identitytoolkit_relyingparty_sign_out_user_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Identitytoolkit relyingparty signup new user.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SignupNewUserResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn identitytoolkit_relyingparty_signup_new_user(
        &self,
        args: &IdentitytoolkitRelyingpartySignupNewUserArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SignupNewUserResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = identitytoolkit_relyingparty_signup_new_user_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = identitytoolkit_relyingparty_signup_new_user_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Identitytoolkit relyingparty upload account.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UploadAccountResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn identitytoolkit_relyingparty_upload_account(
        &self,
        args: &IdentitytoolkitRelyingpartyUploadAccountArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UploadAccountResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = identitytoolkit_relyingparty_upload_account_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = identitytoolkit_relyingparty_upload_account_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Identitytoolkit relyingparty verify assertion.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VerifyAssertionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn identitytoolkit_relyingparty_verify_assertion(
        &self,
        args: &IdentitytoolkitRelyingpartyVerifyAssertionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VerifyAssertionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = identitytoolkit_relyingparty_verify_assertion_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = identitytoolkit_relyingparty_verify_assertion_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Identitytoolkit relyingparty verify custom token.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VerifyCustomTokenResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn identitytoolkit_relyingparty_verify_custom_token(
        &self,
        args: &IdentitytoolkitRelyingpartyVerifyCustomTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VerifyCustomTokenResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = identitytoolkit_relyingparty_verify_custom_token_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = identitytoolkit_relyingparty_verify_custom_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Identitytoolkit relyingparty verify password.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VerifyPasswordResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn identitytoolkit_relyingparty_verify_password(
        &self,
        args: &IdentitytoolkitRelyingpartyVerifyPasswordArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VerifyPasswordResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = identitytoolkit_relyingparty_verify_password_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = identitytoolkit_relyingparty_verify_password_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Identitytoolkit relyingparty verify phone number.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IdentitytoolkitRelyingpartyVerifyPhoneNumberResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn identitytoolkit_relyingparty_verify_phone_number(
        &self,
        args: &IdentitytoolkitRelyingpartyVerifyPhoneNumberArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IdentitytoolkitRelyingpartyVerifyPhoneNumberResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = identitytoolkit_relyingparty_verify_phone_number_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = identitytoolkit_relyingparty_verify_phone_number_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
