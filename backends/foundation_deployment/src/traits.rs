//! Deployable trait and Deploying enum for trait-based deployments.
//!
//! WHY: Users define infrastructure as Rust code, not YAML or TOML configs.
//!      The `Deployable` trait provides a unified interface for deploying and
//!      destroying resources across all providers.
//!
//! WHAT: A trait with associated types for output, error, state store, and DNS
//!       resolver. `deploy()` and `destroy()` are **async**: they return a
//!       [`BoxFuture`] (a `Pin<Box<dyn Future<Output = Result<..>> + Send>>`).
//!       Implementors write ordinary async logic and `Box::pin(async move { … })`
//!       it — no `async_trait` macro, and the trait stays object-safe.
//!
//! HOW: Users implement `Deployable` on their structs. The methods receive a
//!      [`ProviderClient<Store, Resolver>`] for state persistence and (for
//!      HTTP-first providers) an API client. A provider whose transport does
//!      not fit `ProviderClient`'s HTTP client — e.g. Docker over a Unix socket
//!      — may build its own client inside the future and use `ProviderClient`
//!      only for state. See `foundation_deployment_docker`'s `ContainerDeployment`.

use std::future::Future;
use std::pin::Pin;

use foundation_netio::shared::client::DnsResolver;
use foundation_db::core::state::namespaced::NamespacedStore;
use foundation_db::core::state::traits::StateStore;

use crate::provider_client::ProviderClient;

/// A boxed, `Send` future — the return type of [`Deployable::deploy`] and
/// [`Deployable::destroy`].
///
/// Boxing keeps the trait object-safe and lets implementors write plain
/// `Box::pin(async move { … })` without `async_trait` or unstable
/// return-position-`impl`-Trait-in-trait Send plumbing. Any valtron task can
/// also be adapted into one of these futures, so this is the single async
/// interface deployment code speaks.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Generic progress states for deployment and destroy execution.
///
/// WHY: All deployments share the same progress states - no need for custom enums.
///
/// WHAT: Simple enum with `Init`, `Processing`, `Done`, and `Failed` variants.
///
/// HOW: A general-purpose progress signal a caller may report while awaiting a
/// [`Deployable`] future (the trait itself is plain async and does not require
/// it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Deploying {
    /// Initial state before execution begins.
    #[default]
    Init,
    /// Currently processing/executing.
    Processing,
    /// Execution completed successfully.
    Done,
    /// Execution failed.
    Failed,
}

/// Trait for deployable and destroyable infrastructure.
///
/// WHY: Users define infrastructure as Rust types implementing this trait.
///      No YAML, TOML, or custom configuration formats needed.
///
/// WHAT: Trait with associated types for deploy output, destroy output, error,
///       state store, and DNS resolver. `deploy()` and `destroy()` are async —
///       they return a [`BoxFuture`] the caller `.await`s.
///
/// HOW: Implement on user structs. Methods receive `ProviderClient<Store, Resolver>`
///      providing state persistence and HTTP client access. Write the real work
///      as an `async move` block and `Box::pin` it.
///
/// # Associated Types
///
/// * `DeployOutput` - Deployment output type containing URLs, IDs, and artifacts
/// * `DestroyOutput` - Destruction output type (often `()` but can contain metadata)
/// * `Error` - Error type implementing `std::error::Error + Send + Sync + Debug`
/// * `Store` - State store implementation for persistence
/// * `Resolver` - DNS resolver for HTTP calls (e.g., `SystemDnsResolver` or `StaticSocketAddr`)
///
/// # Examples
///
/// ```rust,no_run
/// use foundation_deployment::traits::{BoxFuture, Deployable};
/// use foundation_deployment::provider_client::ProviderClient;
/// use foundation_db::core::state::FileStateStore;
/// use foundation_netio::shared::client::dns::SystemDnsResolver;
///
/// struct MyWorker {
///     name: String,
/// }
///
/// impl Deployable for MyWorker {
///     const NAMESPACE: &'static str = "cloudflare/workers/script";
///
///     type DeployOutput = String;
///     type DestroyOutput = ();
///     type Error = std::io::Error;
///     type Store = FileStateStore;
///     type Resolver = SystemDnsResolver;
///
///     fn deploy(
///         &self,
///         instance_id: usize,
///         client: ProviderClient<Self::Store, Self::Resolver>,
///     ) -> BoxFuture<'static, Result<Self::DeployOutput, Self::Error>> {
///         let name = self.name.clone();
///         Box::pin(async move {
///             // ... real async deploy logic ...
///             Ok(name)
///         })
///     }
///
///     fn destroy(
///         &self,
///         instance_id: usize,
///         client: ProviderClient<Self::Store, Self::Resolver>,
///     ) -> BoxFuture<'static, Result<Self::DestroyOutput, Self::Error>> {
///         Box::pin(async move { Ok(()) })
///     }
/// }
/// ```
pub trait Deployable {
    /// Namespace for this deployable's state store keys.
    ///
    /// Convention: `"provider/group/resource"` — e.g., `"cloudflare/workers/script"`.
    /// All state store operations via `self.store(client)` are automatically
    /// prefixed with this value, isolating resources per-deployable.
    /// Must be a compile-time constant so it cannot drift between deploys.
    const NAMESPACE: &'static str;

    /// Deployment output type — contains URLs, IDs, and other artifacts.
    type DeployOutput: Send + Sync;

    /// Destroy output type — contains metadata about the destruction (often `()`).
    type DestroyOutput: Send + Sync;

    /// Error type for this deployment.
    type Error: std::error::Error + Send + Sync + std::fmt::Debug;

    /// State store type for persistence.
    type Store: StateStore + Send + Sync + 'static;

    /// DNS resolver type for HTTP calls.
    type Resolver: DnsResolver + Clone + 'static;

    /// Deploy a specific instance (async).
    ///
    /// Returns a [`BoxFuture`] the caller `.await`s (or drives on valtron via
    /// `from_future`). Implement it as `Box::pin(async move { … })`.
    ///
    /// # Arguments
    ///
    /// * `instance_id` - Which instance of this resource to deploy (supports multiple instances)
    /// * `client` - ProviderClient with access to state store and HTTP client
    fn deploy(
        &self,
        instance_id: usize,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> BoxFuture<'static, Result<Self::DeployOutput, Self::Error>>;

    /// Destroy a specific instance (async).
    ///
    /// Returns a [`BoxFuture`] the caller `.await`s. Implement it as
    /// `Box::pin(async move { … })`.
    ///
    /// # Arguments
    ///
    /// * `instance_id` - Which instance of this resource to destroy
    /// * `client` - ProviderClient with access to state store and HTTP client
    fn destroy(
        &self,
        instance_id: usize,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> BoxFuture<'static, Result<Self::DestroyOutput, Self::Error>>;

    // -- Default methods --

    /// Returns a `NamespacedStore` scoped to `Self::NAMESPACE`.
    ///
    /// All `get()`, `list()`, `delete()` operations on the returned wrapper
    /// are automatically prefixed with `NAMESPACE`. Call it in `deploy`/`destroy`
    /// to persist and read back what was deployed (the id to tear down, etc.).
    fn store(
        &self,
        client: &ProviderClient<Self::Store, Self::Resolver>,
    ) -> NamespacedStore<Self::Store> {
        NamespacedStore::new(client.state_store.clone(), Self::NAMESPACE)
    }
}
