//! Deployable trait and Deploying enum for trait-based deployments.
//!
//! WHY: Users define infrastructure as Rust code, not YAML or TOML configs.
//!      The `Deployable` trait provides a unified interface for deploying and
//!      destroying resources across all providers.
//!
//! WHAT: A trait with associated types for output, error, state store, and DNS resolver.
//!       Implementors provide `deploy()` and `destroy()` methods that return
//!       valtron `TaskIterator` types. Users decide how to orchestrate execution.
//!
//! HOW: Users implement `Deployable` on their structs. The trait methods receive
//!      `ProviderClient<Store, Resolver>` which provides access to state persistence
//!      and HTTP client for API calls.

use foundation_core::valtron::{BoxedSendExecutionAction, TaskIterator, TaskStatus};
use foundation_core::wire::simple_http::client::DnsResolver;
use foundation_db::state::namespaced::NamespacedStore;
use foundation_db::state::traits::StateStore;
use serde::Serialize;

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
///       state store, and DNS resolver. Provides `deploy()` and `destroy()` methods
///       that return `TaskIterator` — users orchestrate execution themselves.
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
/// use foundation_core::valtron::{TaskIterator, TaskIteratorExt, BoxedSendExecutionAction};
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
///     ) -> Result<
///         impl TaskIterator<Ready = Result<Self::DeployOutput, Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static,
///         Self::Error,
///     > {
///         // Return a TaskIterator — users call execute() or compose it
///         todo!()
///     }
///
///     fn destroy(
///         &self,
///         instance_id: usize,
///         client: ProviderClient<Self::Store, Self::Resolver>,
///     ) -> Result<
///         impl TaskIterator<Ready = Result<Self::DestroyOutput, Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction> + Send + 'static,
///         Self::Error,
///     > {
///         todo!()
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

    /// Deploy a specific instance, returning a TaskIterator for composition.
    ///
    /// Users decide how to execute it — valtron combinators, sequential iteration,
    /// parallel spawning, etc.
    ///
    /// # Arguments
    ///
    /// * `instance_id` - Which instance of this resource to deploy (supports multiple instances)
    /// * `client` - ProviderClient with access to state store and HTTP client
    fn deploy(
        &self,
        instance_id: usize,
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

    /// Destroy a specific instance, returning a TaskIterator for composition.
    ///
    /// Users decide how to execute it — valtron combinators, sequential iteration,
    /// parallel spawning, etc.
    ///
    /// # Arguments
    ///
    /// * `instance_id` - Which instance of this resource to destroy
    /// * `client` - ProviderClient with access to state store and HTTP client
    fn destroy(
        &self,
        instance_id: usize,
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

    // -- Default methods --

    /// Returns a `NamespacedStore` scoped to `Self::NAMESPACE`.
    ///
    /// All `get()`, `list()`, `delete()` operations on the returned wrapper
    /// are automatically prefixed with `NAMESPACE`.
    fn store(
        &self,
        client: &ProviderClient<Self::Store, Self::Resolver>,
    ) -> NamespacedStore<Self::Store> {
        NamespacedStore::new(client.state_store.clone(), Self::NAMESPACE)
    }

    /// Wraps a valtron task to persist the result under `"{NAMESPACE}/{instance_id}"`.
    ///
    /// On task success, stores the result in the state store. The deploy
    /// path calls this to record what was deployed; the destroy path reads it back
    /// via `self.store(client).get_typed(instance_id)` to know what to tear down.
    fn update<T, V, E, I>(
        &self,
        client: &ProviderClient<Self::Store, Self::Resolver>,
        instance_id: usize,
        input: I,
        task: T,
    ) -> UpdateTask<T, Self::Store>
    where
        T: TaskIterator<Ready = Result<V, E>> + Send + 'static,
        V: Serialize + Clone + Send + 'static,
        E: Send + 'static,
        T::Pending: Send + 'static,
        T::Spawner: Send + 'static,
        I: Serialize,
    {
        UpdateTask {
            store: NamespacedStore::new(client.state_store.clone(), Self::NAMESPACE),
            instance_id,
            input: serde_json::to_value(&input).unwrap_or(serde_json::json!(null)),
            task,
        }
    }
}

/// Internal task wrapper that persists results after the inner task completes.
pub struct UpdateTask<T, S>
where
    S: StateStore,
    T: TaskIterator,
{
    pub(crate) store: NamespacedStore<S>,
    pub(crate) instance_id: usize,
    pub(crate) input: serde_json::Value,
    pub(crate) task: T,
}

impl<T, S, V, E> TaskIterator for UpdateTask<T, S>
where
    S: StateStore + Send + Sync + 'static,
    T: TaskIterator<Ready = Result<V, E>>,
    T::Pending: Send + 'static,
    T::Spawner: Send + 'static,
    V: Serialize + Send + 'static,
    E: Send + 'static,
{
    type Ready = Result<V, E>;
    type Pending = T::Pending;
    type Spawner = T::Spawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.task.next_status()? {
            TaskStatus::Ready(Ok(value)) => {
                let key = self.instance_id.to_string();
                let stored = (
                    &self.input,
                    serde_json::to_value(&value).unwrap_or(serde_json::json!(null)),
                );
                if let Err(e) = self.store.store_typed(&key, &stored) {
                    tracing::warn!(
                        "Failed to store deployment state for {}/{}: {e:?}",
                        self.store.prefix(),
                        key
                    );
                }
                Some(TaskStatus::Ready(Ok(value)))
            }
            TaskStatus::Ready(Err(e)) => Some(TaskStatus::Ready(Err(e))),
            other => Some(other),
        }
    }
}
