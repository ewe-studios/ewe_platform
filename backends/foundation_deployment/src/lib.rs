//! Foundation Deployment — trait-based, multi-provider infrastructure deploys.
//!
//! WHY: Infrastructure is declared as ordinary Rust types (no YAML/TOML). A type
//! that implements [`Deployable`](traits::Deployable) can be deployed and
//! destroyed uniformly, whatever the underlying provider or transport.
//!
//! WHAT: The [`Deployable`](traits::Deployable) trait — `deploy`/`destroy` are
//! **async**, returning a [`BoxFuture`](traits::BoxFuture)
//! (`Pin<Box<dyn Future<Output = Result<..>> + Send>>`) the caller `.await`s —
//! plus [`ProviderClient`](provider_client::ProviderClient) (state store +
//! HTTP client) and the namespaced state persistence a deploy records and a
//! destroy reads back.
//!
//! HOW: Implementors write plain `Box::pin(async move { … })` — no `async_trait`
//! macro, and the trait stays object-safe (`dyn Deployable` works). A provider
//! whose transport does not fit `ProviderClient`'s HTTP client (e.g. Docker over
//! a Unix socket) may build its own client inside the future and use
//! `ProviderClient` only for state. See `README.md` for the full pattern and the
//! "unique underlying mechanics" case.

pub mod config;
pub mod core;
pub mod error;
pub mod json_schema;
pub mod provider_client;
pub mod providers;
pub mod resource_info;
pub mod traits;
pub mod types;

pub use traits::{BoxFuture, Deployable, Deploying};
