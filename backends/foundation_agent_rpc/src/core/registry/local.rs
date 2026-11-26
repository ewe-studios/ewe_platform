//! The local instance registry: in-process instances over one catalog.
//!
//! WHY: the native host and each Durable Object hold their instances in one
//! process. The logic is the same in both places; only the two session stores
//! differ.
//!
//! WHAT: [`SessionStores`] (the seam for the stores), [`InMemoryStores`] (the
//! native default) and [`LocalInstanceRegistry`].
//!
//! HOW: the table is keyed by `(Scope, name)`. `create` builds the session
//! outside the lock, because building is the only real work. One turn at a
//! time is a compare-and-swap on an atomic flag that a [`TurnGuard`] clears on
//! drop.

use std::collections::BTreeMap;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::{Duration, Instant};

use foundation_ai::agentic::{AgentConfig, AgentSession, KvMemoryStore, MemoryStore};
use foundation_ai::types::SessionId;
use foundation_compact::time::unix_millis;
use foundation_context::env::Env;
use foundation_context::Ctx;
use foundation_db::{DocumentStore, MemoryDocumentStore, MemoryStorage};
use foundation_errstacks::ErrorTrace;

use crate::core::config::{RegistryConfig, TurnLimits};
use crate::core::model::{AgentRpcFuture, ModelCatalog, ModelHandle};
use crate::core::registry::{
    decode_page_token, encode_page_token, Caller, InstanceInfo, InstanceRegistry, InstanceSpec,
    Page, Scope, TurnEventStream,
};
use crate::core::turn::{guarded_turn, TurnShutdown};
use crate::error::{AgentRpcError, AgentRpcResult};

/// Makes the two stores of a new session.
pub trait SessionStores: Send + Sync + 'static {
    /// The document store (the session history).
    type Doc: DocumentStore + 'static;
    /// The memory store.
    type Mem: MemoryStore + 'static;

    /// Open the two stores of one session.
    ///
    /// # Errors
    /// `AgentRpcError::Storage` when the stores cannot open.
    fn open(&self, session: &SessionId) -> AgentRpcResult<(Self::Doc, Self::Mem)>;
}

/// The native default: history and memory in process memory.
#[derive(Debug, Clone, Copy, Default)]
pub struct InMemoryStores;

impl SessionStores for InMemoryStores {
    type Doc = MemoryDocumentStore;
    type Mem = KvMemoryStore<MemoryStorage>;

    fn open(&self, _session: &SessionId) -> AgentRpcResult<(Self::Doc, Self::Mem)> {
        Ok((
            MemoryDocumentStore::new(),
            KvMemoryStore::new(MemoryStorage::new()),
        ))
    }
}

/// One named instance: an agent session on a shared model.
pub struct InstanceHandle<D, M>
where
    D: DocumentStore + 'static,
    M: MemoryStore + 'static,
{
    inner: Arc<InstanceInner<D, M>>,
}

struct InstanceInner<D, M>
where
    D: DocumentStore + 'static,
    M: MemoryStore + 'static,
{
    name: String,
    model: ModelHandle,
    session: AgentSession<D, M>,
    busy: AtomicBool,
    turns: AtomicU64,
    created_unix_ms: u64,
}

impl<D, M> Clone for InstanceHandle<D, M>
where
    D: DocumentStore + 'static,
    M: MemoryStore + 'static,
{
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<D, M> std::fmt::Debug for InstanceHandle<D, M>
where
    D: DocumentStore + 'static,
    M: MemoryStore + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstanceHandle")
            .field("name", &self.inner.name)
            .field("model", &self.inner.model.name())
            .finish_non_exhaustive()
    }
}

impl<D, M> InstanceHandle<D, M>
where
    D: DocumentStore + 'static,
    M: MemoryStore + 'static,
{
    /// The instance name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.inner.name
    }

    /// The shared model that this instance runs on.
    #[must_use]
    pub fn model(&self) -> &ModelHandle {
        &self.inner.model
    }

    /// The session of this instance.
    #[must_use]
    pub fn session(&self) -> &AgentSession<D, M> {
        &self.inner.session
    }

    /// A snapshot for the wire.
    #[must_use]
    pub fn info(&self) -> InstanceInfo {
        InstanceInfo {
            name: self.inner.name.clone(),
            model: self.inner.model.name().to_string(),
            kind: self.inner.model.kind(),
            device: self.inner.model.device(),
            created_unix_ms: self.inner.created_unix_ms,
            turns: self.inner.turns.load(Ordering::Relaxed),
        }
    }

    /// Claim the instance for one turn.
    ///
    /// # Errors
    /// [`AgentRpcError::InstanceBusy`] while another turn holds the claim.
    pub fn begin_turn(&self) -> AgentRpcResult<TurnGuard<D, M>> {
        if self
            .inner
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(AgentRpcError::InstanceBusy(self.inner.name.clone()).into());
        }
        self.inner.turns.fetch_add(1, Ordering::Relaxed);
        Ok(TurnGuard {
            inner: Arc::clone(&self.inner),
        })
    }
}

/// Holds an instance's one-turn claim; releases it on drop.
pub struct TurnGuard<D, M>
where
    D: DocumentStore + 'static,
    M: MemoryStore + 'static,
{
    inner: Arc<InstanceInner<D, M>>,
}

impl<D, M> std::fmt::Debug for TurnGuard<D, M>
where
    D: DocumentStore + 'static,
    M: MemoryStore + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TurnGuard")
            .field("instance", &self.inner.name)
            .finish()
    }
}

impl<D, M> Drop for TurnGuard<D, M>
where
    D: DocumentStore + 'static,
    M: MemoryStore + 'static,
{
    fn drop(&mut self) {
        self.inner.busy.store(false, Ordering::Release);
    }
}

/// The count of running turns, for a graceful shutdown.
#[derive(Default)]
struct TurnActivity {
    count: Mutex<usize>,
    idle: Condvar,
}

impl TurnActivity {
    fn enter(self: &Arc<Self>) -> TurnActive {
        let mut count = self
            .count
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *count += 1;
        TurnActive {
            activity: Arc::clone(self),
        }
    }

    /// Wait until no turn runs, or until `grace` real seconds pass.
    ///
    /// The wait reads real time: shutdown grace is the time that a process
    /// gets after `SIGTERM` (Kubernetes `terminationGracePeriodSeconds`), not
    /// simulated time. It runs only at the host shutdown boundary.
    fn wait_idle(&self, grace: Duration) -> bool {
        let deadline = Instant::now() + grace;
        let mut count = self
            .count
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while *count > 0 {
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            let remaining = deadline.saturating_duration_since(now);
            let (guard, _) = self
                .idle
                .wait_timeout(count, remaining)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            count = guard;
        }
        true
    }
}

/// One running turn's count. It decrements on drop.
struct TurnActive {
    activity: Arc<TurnActivity>,
}

impl Drop for TurnActive {
    fn drop(&mut self) {
        let mut count = self
            .activity
            .count
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *count = count.saturating_sub(1);
        if *count == 0 {
            self.activity.idle.notify_all();
        }
    }
}

/// The table of live instances, keyed by scope and name.
type InstanceTable<D, M> = BTreeMap<(Scope, String), InstanceHandle<D, M>>;

/// The table of live instances over one catalog.
pub struct LocalInstanceRegistry<S: SessionStores = InMemoryStores> {
    ctx: Ctx,
    env: Env,
    catalog: Arc<dyn ModelCatalog>,
    stores: S,
    instances: RwLock<InstanceTable<S::Doc, S::Mem>>,
    config: RegistryConfig,
    turns: TurnLimits,
    shutdown: TurnShutdown,
    activity: Arc<TurnActivity>,
    /// Runs before `destroy` removes an instance. The Durable Object uploads
    /// the history here; an error stops the destroy and keeps the storage
    /// (invariant I5).
    destroy_hook: Option<DestroyHookFn>,
    /// Runs after `create` inserts an instance. The Durable Object records the
    /// instance and its session here, so a new object can restore them.
    create_hook: Option<CreateHookFn>,
}

/// The future of one [`DestroyHook`].
pub type DestroyHookFuture =
    Pin<Box<dyn std::future::Future<Output = AgentRpcResult<()>> + Send + 'static>>;

/// Runs before an instance is destroyed.
///
/// The hook receives the checked caller, the instance facts and the session
/// identifier. It reads the session's history from the same storage and
/// uploads it. When it returns an error, the registry keeps the instance and
/// its storage, so no history is lost.
pub type DestroyHookFn =
    Arc<dyn Fn(Caller, InstanceInfo, SessionId) -> DestroyHookFuture + Send + Sync + 'static>;

/// Runs after an instance is created.
///
/// It receives the checked caller, the instance facts, the session identifier
/// and the spec. A Durable Object uses it to record the instance in its SQL
/// storage. An error removes the new instance and fails the create.
pub type CreateHookFn = Arc<
    dyn Fn(Caller, InstanceInfo, SessionId, InstanceSpec) -> DestroyHookFuture
        + Send
        + Sync
        + 'static,
>;

impl<S: SessionStores> std::fmt::Debug for LocalInstanceRegistry<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalInstanceRegistry")
            .field("config", &self.config)
            .field("turns", &self.turns)
            .finish_non_exhaustive()
    }
}

impl<S: SessionStores> LocalInstanceRegistry<S> {
    /// A registry over `catalog`, with the stores of `stores`.
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] when the context holds no [`Env`].
    pub fn new(
        catalog: Arc<dyn ModelCatalog>,
        stores: S,
        config: RegistryConfig,
        turns: TurnLimits,
        ctx: Ctx,
    ) -> AgentRpcResult<Self> {
        let env = ctx
            .require_env()
            .map_err(|error| AgentRpcError::Config(format!("the context holds no Env: {error}")))?;
        Ok(Self {
            ctx,
            env: env.as_ref().clone(),
            catalog,
            stores,
            instances: RwLock::new(BTreeMap::new()),
            config,
            turns,
            shutdown: TurnShutdown::default(),
            activity: Arc::new(TurnActivity::default()),
            destroy_hook: None,
            create_hook: None,
        })
    }

    /// Run `hook` before an instance is destroyed.
    ///
    /// The Durable Object uses it to upload the session history to R2. The
    /// hook runs before the instance leaves the table; an error keeps the
    /// instance and its storage (invariant I5).
    #[must_use]
    pub fn with_destroy_hook(mut self, hook: DestroyHookFn) -> Self {
        self.destroy_hook = Some(hook);
        self
    }

    /// Run `hook` after an instance is created.
    ///
    /// The Durable Object uses it to record the instance and its session in
    /// SQL, so a new object can restore the instance. An error removes the new
    /// instance and fails the create.
    #[must_use]
    pub fn with_create_hook(mut self, hook: CreateHookFn) -> Self {
        self.create_hook = Some(hook);
        self
    }

    /// Rebuild one instance from its recorded facts.
    ///
    /// A Durable Object calls this after a restart for every record that its
    /// SQL storage holds. The session keeps its identifier, so its history
    /// stays. An instance that is already in the table is left as it is.
    ///
    /// # Errors
    /// [`AgentRpcError::ModelSelection`] when the model is gone, and
    /// [`AgentRpcError::Storage`] when a store cannot open.
    pub async fn restore(
        &self,
        caller: &Caller,
        info: &InstanceInfo,
        session_id: SessionId,
        spec: &InstanceSpec,
    ) -> AgentRpcResult<()> {
        let key = (caller.scope.clone(), info.name.clone());
        if self.table().contains_key(&key) {
            return Ok(());
        }
        let model = self.catalog.resolve(&spec.model).await?;
        let (doc, memory) = self.stores.open(&session_id)?;
        let session =
            build_session(&model, spec, session_id, doc, memory, self.ctx.clone()).await?;
        let handle = InstanceHandle {
            inner: Arc::new(InstanceInner {
                name: info.name.clone(),
                model,
                session,
                busy: AtomicBool::new(false),
                turns: AtomicU64::new(info.turns),
                created_unix_ms: info.created_unix_ms,
            }),
        };
        self.table_mut().insert(key, handle);
        Ok(())
    }

    /// The configuration.
    #[must_use]
    pub fn config(&self) -> &RegistryConfig {
        &self.config
    }

    /// The turn limits.
    #[must_use]
    pub fn turn_limits(&self) -> &TurnLimits {
        &self.turns
    }

    /// How many instances the table holds.
    #[must_use]
    pub fn instance_count(&self) -> usize {
        self.table().len()
    }

    /// Stop the running turns and abort their sessions.
    ///
    /// The method waits up to `grace` for the turns to end on their own. The
    /// turns that still run then get [`TurnShutdown::fire`], so each one emits
    /// its terminal `TurnFailed("server shutting down")` and drops. A second
    /// bounded wait gives them time to flush.
    pub fn shutdown(&self, grace: Duration) {
        if self.activity.wait_idle(grace) {
            return;
        }
        self.shutdown.fire();
        let _ = self.activity.wait_idle(grace.min(Duration::from_secs(2)));
        let table = self.table();
        for handle in table.values() {
            handle.session().abort();
        }
    }

    /// End every session and empty the table.
    ///
    /// # Errors
    /// [`AgentRpcError::Inference`] when a session cannot end. The table is
    /// emptied either way.
    pub async fn end_all(&self) -> AgentRpcResult<()> {
        let handles: Vec<InstanceHandle<S::Doc, S::Mem>> = self.table().values().cloned().collect();
        let mut result = Ok(());
        for handle in &handles {
            handle.session().abort();
            if let Err(error) = handle.session().end().await {
                result = Err(ErrorTrace::new(AgentRpcError::Inference(format!(
                    "ending instance {}: {error}",
                    handle.name()
                ))));
            }
        }
        self.table_mut().clear();
        result
    }

    fn table(&self) -> std::sync::RwLockReadGuard<'_, InstanceTable<S::Doc, S::Mem>> {
        self.instances
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn table_mut(&self) -> std::sync::RwLockWriteGuard<'_, InstanceTable<S::Doc, S::Mem>> {
        self.instances
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Names are 1..=`max_name_len` bytes of `[a-z0-9._-]` (ASCII, lowercase),
    /// starting with a letter or digit.
    fn validate_name(&self, name: &str) -> AgentRpcResult<()> {
        let invalid = |why: &str| -> AgentRpcResult<()> {
            Err(AgentRpcError::InvalidName(format!("{name:?}: {why}")).into())
        };
        if name.is_empty() {
            return invalid("empty");
        }
        if name.len() > self.config.max_name_len {
            return invalid(&format!("longer than {} bytes", self.config.max_name_len));
        }
        if !name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
        }) {
            return invalid("allowed characters are a-z 0-9 . _ -");
        }
        if !name
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
        {
            return invalid("must start with a letter or digit");
        }
        Ok(())
    }
}

impl<S: SessionStores> InstanceRegistry for LocalInstanceRegistry<S> {
    fn create<'a>(
        &'a self,
        caller: &'a Caller,
        spec: InstanceSpec,
    ) -> AgentRpcFuture<'a, InstanceInfo> {
        Box::pin(async move {
            self.validate_name(&spec.name)?;
            let model = self.catalog.resolve(&spec.model).await?;
            let session_id = SessionId::from_id(self.env.ids().next_id());
            let (doc, memory) = self.stores.open(&session_id)?;
            let session =
                build_session(&model, &spec, session_id, doc, memory, self.ctx.clone()).await?;
            let key = (caller.scope.clone(), spec.name.clone());
            let handle = InstanceHandle {
                inner: Arc::new(InstanceInner {
                    name: spec.name.clone(),
                    model,
                    session,
                    busy: AtomicBool::new(false),
                    turns: AtomicU64::new(0),
                    created_unix_ms: u64::try_from(unix_millis(self.ctx.clock().wall_now()))
                        .unwrap_or(0),
                }),
            };
            let info = handle.info();
            let session_id = handle.session().session_id().clone();
            {
                let mut table = self.table_mut();
                if table.contains_key(&key) {
                    return Err(AgentRpcError::InstanceExists(spec.name).into());
                }
                if table
                    .keys()
                    .filter(|(scope, _)| scope == &caller.scope)
                    .count()
                    >= self.config.max_instances
                {
                    return Err(AgentRpcError::InstanceLimit(self.config.max_instances).into());
                }
                table.insert(key.clone(), handle.clone());
            }
            if let Some(hook) = &self.create_hook {
                if let Err(error) =
                    hook(caller.clone(), info.clone(), session_id, spec.clone()).await
                {
                    let _ = self.table_mut().remove(&key);
                    return Err(error);
                }
            }
            tracing::info!(instance = %spec.name, model = %handle.model().name(), "instance created");
            Ok(info)
        })
    }

    fn get<'a>(&'a self, caller: &'a Caller, name: &'a str) -> AgentRpcFuture<'a, InstanceInfo> {
        Box::pin(async move {
            let table = self.table();
            table
                .get(&(caller.scope.clone(), name.to_string()))
                .map(InstanceHandle::info)
                .ok_or_else(|| AgentRpcError::InstanceNotFound(name.to_string()).into())
        })
    }

    fn list<'a>(
        &'a self,
        caller: &'a Caller,
        page_size: u32,
        page_token: Option<&'a str>,
    ) -> AgentRpcFuture<'a, Page<InstanceInfo>> {
        Box::pin(async move {
            let after = decode_page_token(page_token.unwrap_or(""))?;
            let size = if page_size == 0 {
                self.config.default_page_size
            } else {
                page_size.min(self.config.max_page_size)
            }
            .max(1);
            let size = usize::try_from(size).unwrap_or(usize::MAX);
            let table = self.table();
            let names: Vec<String> = table
                .keys()
                .filter(|(scope, _)| scope == &caller.scope)
                .map(|(_, name)| name.clone())
                .filter(|name| after.as_ref().is_none_or(|after| name > after))
                .collect();
            let page_len = names.len().min(size);
            let items: Vec<InstanceInfo> = names[..page_len]
                .iter()
                .filter_map(|name| {
                    table
                        .get(&(caller.scope.clone(), name.clone()))
                        .map(InstanceHandle::info)
                })
                .collect();
            let next_token =
                (names.len() > page_len).then(|| encode_page_token(&names[page_len - 1]));
            Ok(Page { items, next_token })
        })
    }

    fn destroy<'a>(&'a self, caller: &'a Caller, name: &'a str) -> AgentRpcFuture<'a, ()> {
        Box::pin(async move {
            let key = (caller.scope.clone(), name.to_string());
            let handle = {
                let table = self.table();
                table.get(&key).cloned()
            };
            let Some(handle) = handle else {
                return Err(AgentRpcError::InstanceNotFound(name.to_string()).into());
            };
            // The history archive runs before the instance leaves the table.
            // Its error keeps the instance and its storage (invariant I5).
            if let Some(hook) = &self.destroy_hook {
                let info = handle.info();
                let session = handle.session().session_id().clone();
                hook(caller.clone(), info, session).await?;
            }
            let removed = self.table_mut().remove(&key);
            let Some(handle) = removed else {
                return Err(AgentRpcError::InstanceNotFound(name.to_string()).into());
            };
            handle.session().abort();
            handle.session().end().await.map_err(|error| {
                ErrorTrace::new(AgentRpcError::Inference(format!(
                    "ending instance {name}: {error}"
                )))
            })?;
            tracing::info!(instance = %name, "instance destroyed");
            Ok(())
        })
    }

    fn start_turn<'a>(
        &'a self,
        caller: &'a Caller,
        name: &'a str,
        prompt: String,
    ) -> AgentRpcFuture<'a, TurnEventStream> {
        Box::pin(async move {
            if prompt.len() > self.turns.max_prompt_bytes {
                return Err(AgentRpcError::InvalidArgument(format!(
                    "the prompt is {} bytes; the limit is {}",
                    prompt.len(),
                    self.turns.max_prompt_bytes
                ))
                .into());
            }
            let instance = {
                let table = self.table();
                table
                    .get(&(caller.scope.clone(), name.to_string()))
                    .cloned()
                    .ok_or_else(|| AgentRpcError::InstanceNotFound(name.to_string()))?
            };
            let guard = instance.begin_turn()?;
            let active = self.activity.enter();
            let turn = instance
                .session()
                .run_turn_stream(prompt)
                .map_err(|error| AgentRpcError::Inference(error.to_string()))?;
            let session = instance.session().clone();
            let abort: Arc<dyn Fn() + Send + Sync> = Arc::new(move || session.abort());
            Ok(guarded_turn(
                turn.events(),
                (guard, active),
                abort,
                self.shutdown.clone(),
                self.ctx.clone(),
                self.env.clone(),
                &self.turns,
            ))
        })
    }

    fn count<'a>(&'a self, caller: &'a Caller) -> AgentRpcFuture<'a, u64> {
        Box::pin(async move {
            let count = self
                .table()
                .keys()
                .filter(|(scope, _)| scope == &caller.scope)
                .count();
            Ok(u64::try_from(count).unwrap_or(u64::MAX))
        })
    }
}

async fn build_session<D, M>(
    model: &ModelHandle,
    spec: &InstanceSpec,
    session_id: SessionId,
    doc: D,
    memory: M,
    ctx: Ctx,
) -> AgentRpcResult<AgentSession<D, M>>
where
    D: DocumentStore + 'static,
    M: MemoryStore + 'static,
{
    let mut config = AgentConfig::default();
    if let Some(max_tokens) = spec.max_tokens {
        config.model_params.max_tokens = max_tokens;
    }
    if let Some(temperature) = spec.temperature {
        config.model_params.temperature = temperature;
    }
    let mut builder = AgentSession::builder(model.router(), ctx)
        .with_session_id(session_id)
        .with_doc_store(doc)
        .with_memory_store(memory)
        .with_model(model.model_id().clone())
        .with_config(config);
    if let Some(prompt) = &spec.system_prompt {
        builder = builder.with_system_prompt(prompt.clone());
    }
    builder
        .build()
        .await
        .map_err(|error| AgentRpcError::Inference(format!("building session: {error}")).into())
}
