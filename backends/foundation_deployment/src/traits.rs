//! Deployable trait and Deploying enum for trait-based deployments.
//!
//! WHY: Users define infrastructure as Rust code, not YAML or TOML configs.
//!      The `Deployable` trait provides a unified interface for deploying and
//!      destroying resources across all providers.
//!
//! WHAT: A trait with associated types for output, error, state store, and DNS resolver.
//!       Implementors provide `deploy()` and `destroy()` methods that return
//!       valtron `StreamIterator`/`TaskIterator` types.
//!
//! HOW: Users implement `Deployable` on their structs. The trait methods receive
//!      `ProviderClient<Store, Resolver>` which provides access to state persistence
//!      and HTTP client for API calls.

use foundation_core::valtron::{BoxedSendExecutionAction, StreamIterator, TaskIterator};
use foundation_core::wire::simple_http::client::DnsResolver;
use foundation_db::state::traits::StateStore;

use crate::provider_client::ProviderClient;

/// Generic progress states for deployment and destroy execution.
///
/// WHY: All deployments share the same progress states - no need for custom enums.
///
/// WHAT: Simple enum with `Init`, `Processing`, `Done`, and `Failed` variants.
///
/// HOW: Used as the `Pending` associated type in `TaskIterator` return types.
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
///       state store, and DNS resolver. Provides `deploy()`/`deploy_task()` and
///       `destroy()`/`destroy_task()` methods.
///
/// HOW: Implement on user structs. Methods receive `ProviderClient<Store, Resolver>`
///      providing state persistence and HTTP client access.
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
/// use foundation_deployment::traits::{Deployable, Deploying};
/// use foundation_deployment::provider_client::ProviderClient;
/// use foundation_db::state::FileStateStore;
/// use foundation_core::wire::simple_http::client::SystemDnsResolver;
///
/// struct MyWorker {
///     name: String,
///     script: String,
/// }
///
/// impl Deployable for MyWorker {
///     type DeployOutput = WorkerDeployment;
///     type DestroyOutput = ();
///     type Error = DeploymentError;
///     type Store = FileStateStore;
///     type Resolver = SystemDnsResolver;
///
///     fn deploy(
///         &self,
///         client: ProviderClient<Self::Store, Self::Resolver>,
///     ) -> Result<
///         impl StreamIterator<D = Result<Self::DeployOutput, Self::Error>, P = Deploying> + Send + 'static,
///         Self::Error,
///     > {
///         self.deploy_task(client)
///             .and_then(|task| execute(task, None))
///             .map_err(|e| DeploymentError::ExecutorFailed(e.to_string()))
///     }
///
///     fn deploy_task(
///         &self,
///         client: ProviderClient<Self::Store, Self::Resolver>,
///     ) -> Result<
///         impl TaskIterator<Ready = Result<Self::DeployOutput, Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static,
///         Self::Error,
///     > {
///         // Implementation here
///         todo!()
///     }
///
///     fn destroy(
///         &self,
///         client: ProviderClient<Self::Store, Self::Resolver>,
///     ) -> Result<
///         impl StreamIterator<D = Result<Self::DestroyOutput, Self::Error>, P = Deploying> + Send + 'static,
///         Self::Error,
///     > {
///         self.destroy_task(client)
///             .and_then(|task| execute(task, None))
///             .map_err(|e| DeploymentError::ExecutorFailed(e.to_string()))
///     }
///
///     fn destroy_task(
///         &self,
///         client: ProviderClient<Self::Store, Self::Resolver>,
///     ) -> Result<
///         impl TaskIterator<Ready = Result<Self::DestroyOutput, Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static,
///         Self::Error,
///     > {
///         // Implementation here
///         todo!()
///     }
/// }
/// ```
pub trait Deployable {
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

    /// Deploy the resource and return a StreamIterator with the result.
    ///
    /// WHY: Convenience method for immediate execution via valtron.
    ///
    /// WHAT: Calls `deploy_task()` and executes it via `execute()`.
    ///
    /// HOW: Users call this for simple deployments, or `deploy_task()` for composition.
    ///
    /// # Arguments
    ///
    /// * `client` - ProviderClient with access to state store and HTTP client
    ///
    /// # Returns
    ///
    /// Returns `Ok(StreamIterator)` which yields `Result<DeployOutput, Error>` when iterated.
    fn deploy(
        &self,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<
        impl StreamIterator<D = Result<Self::DeployOutput, Self::Error>, P = Deploying> + Send + 'static,
        Self::Error,
    >;

    /// Deploy the resource and return a TaskIterator for customization.
    ///
    /// WHY: Users may want to compose tasks with valtron combinators before execution.
    ///
    /// WHAT: Core method containing actual deployment logic.
    ///
    /// HOW: Returns `TaskIterator` that can be executed via `execute()` or composed.
    ///
    /// # Arguments
    ///
    /// * `client` - ProviderClient with access to state store and HTTP client
    ///
    /// # Returns
    ///
    /// Returns `Ok(TaskIterator)` which can be executed via `execute()`.
    fn deploy_task(
        &self,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<
        impl TaskIterator<
                Ready = Result<Self::DeployOutput, Self::Error>,
                Pending = Deploying,
                Spawner = BoxedSendExecutionAction,
            > + Send
            + 'static,
        Self::Error,
    >;

    /// Destroy the resource and return a StreamIterator with the result.
    ///
    /// WHY: Convenience method for immediate execution via valtron.
    ///
    /// WHAT: Calls `destroy_task()` and executes it via `execute()`.
    ///
    /// HOW: Uses state store to read deployment output for resource identification.
    ///
    /// # Arguments
    ///
    /// * `client` - ProviderClient with access to state store and HTTP client
    ///
    /// # Returns
    ///
    /// Returns `Ok(StreamIterator)` which yields `Result<DestroyOutput, Error>` when iterated.
    fn destroy(
        &self,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<
        impl StreamIterator<D = Result<Self::DestroyOutput, Self::Error>, P = Deploying> + Send + 'static,
        Self::Error,
    >;

    /// Destroy the resource and return a TaskIterator for customization.
    ///
    /// WHY: Users may want to compose tasks with valtron combinators before execution.
    ///
    /// WHAT: Core method containing actual destroy logic.
    ///
    /// HOW: Reads state store to get deployment output, then calls provider delete API.
    ///      Returns idempotent success if no state exists (resource never deployed).
    ///
    /// # Arguments
    ///
    /// * `client` - ProviderClient with access to state store and HTTP client
    ///
    /// # Returns
    ///
    /// Returns `Ok(TaskIterator)` which can be executed via `execute()`.
    fn destroy_task(
        &self,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<
        impl TaskIterator<
                Ready = Result<Self::DestroyOutput, Self::Error>,
                Pending = Deploying,
                Spawner = BoxedSendExecutionAction,
            > + Send
            + 'static,
        Self::Error,
    >;
}
