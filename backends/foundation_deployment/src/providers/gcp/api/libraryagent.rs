//! LibraryagentProvider - State-aware libraryagent API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       libraryagent API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::libraryagent::{
    libraryagent_shelves_get_builder, libraryagent_shelves_get_task,
    libraryagent_shelves_list_builder, libraryagent_shelves_list_task,
    libraryagent_shelves_books_borrow_builder, libraryagent_shelves_books_borrow_task,
    libraryagent_shelves_books_get_builder, libraryagent_shelves_books_get_task,
    libraryagent_shelves_books_list_builder, libraryagent_shelves_books_list_task,
    libraryagent_shelves_books_return_builder, libraryagent_shelves_books_return_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::libraryagent::GoogleExampleLibraryagentV1Book;
use crate::providers::gcp::clients::libraryagent::GoogleExampleLibraryagentV1ListBooksResponse;
use crate::providers::gcp::clients::libraryagent::GoogleExampleLibraryagentV1ListShelvesResponse;
use crate::providers::gcp::clients::libraryagent::GoogleExampleLibraryagentV1Shelf;
use crate::providers::gcp::clients::libraryagent::LibraryagentShelvesBooksBorrowArgs;
use crate::providers::gcp::clients::libraryagent::LibraryagentShelvesBooksGetArgs;
use crate::providers::gcp::clients::libraryagent::LibraryagentShelvesBooksListArgs;
use crate::providers::gcp::clients::libraryagent::LibraryagentShelvesBooksReturnArgs;
use crate::providers::gcp::clients::libraryagent::LibraryagentShelvesGetArgs;
use crate::providers::gcp::clients::libraryagent::LibraryagentShelvesListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// LibraryagentProvider with automatic state tracking.
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
/// let provider = LibraryagentProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct LibraryagentProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> LibraryagentProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new LibraryagentProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new LibraryagentProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Libraryagent shelves get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleExampleLibraryagentV1Shelf result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn libraryagent_shelves_get(
        &self,
        args: &LibraryagentShelvesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleExampleLibraryagentV1Shelf, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = libraryagent_shelves_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = libraryagent_shelves_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Libraryagent shelves list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleExampleLibraryagentV1ListShelvesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn libraryagent_shelves_list(
        &self,
        args: &LibraryagentShelvesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleExampleLibraryagentV1ListShelvesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = libraryagent_shelves_list_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = libraryagent_shelves_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Libraryagent shelves books borrow.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleExampleLibraryagentV1Book result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn libraryagent_shelves_books_borrow(
        &self,
        args: &LibraryagentShelvesBooksBorrowArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleExampleLibraryagentV1Book, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = libraryagent_shelves_books_borrow_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = libraryagent_shelves_books_borrow_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Libraryagent shelves books get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleExampleLibraryagentV1Book result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn libraryagent_shelves_books_get(
        &self,
        args: &LibraryagentShelvesBooksGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleExampleLibraryagentV1Book, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = libraryagent_shelves_books_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = libraryagent_shelves_books_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Libraryagent shelves books list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleExampleLibraryagentV1ListBooksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn libraryagent_shelves_books_list(
        &self,
        args: &LibraryagentShelvesBooksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleExampleLibraryagentV1ListBooksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = libraryagent_shelves_books_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = libraryagent_shelves_books_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Libraryagent shelves books return.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleExampleLibraryagentV1Book result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn libraryagent_shelves_books_return(
        &self,
        args: &LibraryagentShelvesBooksReturnArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleExampleLibraryagentV1Book, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = libraryagent_shelves_books_return_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = libraryagent_shelves_books_return_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
