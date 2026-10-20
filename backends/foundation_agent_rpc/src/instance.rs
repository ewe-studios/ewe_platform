//! Named agent instances and the registry that owns them.
//!
//! WHY: an "instance" is what a client talks to — a conversation with its own
//! memory and settings. It must be cheap to create and destroy, because the
//! model behind it is loaded once for the whole server and shared.
//!
//! WHAT: [`InstanceRegistry`] maps client-chosen names to [`InstanceHandle`]s
//! (an `AgentSession` over a shared [`ModelHandle`] plus a one-turn-at-a-time
//! guard). [`InstanceConfig`] picks the model and per-instance settings;
//! [`RegistryConfig`] holds the limits.
//!
//! HOW: `create` validates the name, looks the model up by name and builds a
//! session on the model's router — a registry operation, never a model load.
//! The table sits behind an `RwLock`; handles are `Arc`s, so a turn keeps
//! running if its instance is destroyed meanwhile.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use foundation_ai::agentic::{AgentConfig, AgentSession};
use foundation_errstacks::ErrorTrace;

use crate::error::{AgentRpcError, AgentRpcResult};
use crate::model::ModelHandle;

/// Registry limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryConfig {
    max_instances: usize,
    max_name_len: usize,
}

impl RegistryConfig {
    /// Cap the number of live instances.
    #[must_use]
    pub fn with_max_instances(mut self, max: usize) -> Self {
        self.max_instances = max;
        self
    }

    /// Cap the instance name length.
    #[must_use]
    pub fn with_max_name_len(mut self, max: usize) -> Self {
        self.max_name_len = max;
        self
    }

    /// The live-instance cap.
    #[must_use]
    pub fn max_instances(&self) -> usize {
        self.max_instances
    }

    /// The instance-name length cap.
    #[must_use]
    pub fn max_name_len(&self) -> usize {
        self.max_name_len
    }
}

impl Default for RegistryConfig {
    /// - `max_instances`: 1024 — nginx's default `worker_connections` in the
    ///   shipped `nginx.conf`, the same order as the concurrent clients one
    ///   host is expected to serve.
    /// - `max_name_len`: 63 — an RFC 1123 DNS label (the Kubernetes object-name
    ///   rule), so names are safe as path segments and hostnames.
    fn default() -> Self {
        Self {
            max_instances: 1024,
            max_name_len: 63,
        }
    }
}

/// Per-instance settings chosen at creation.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InstanceConfig {
    /// Name of a model the server hosts. Empty = the server's only model.
    pub model: String,
    /// System prompt for the session.
    pub system_prompt: Option<String>,
    /// Max tokens per model call (the session default when `None`).
    pub max_tokens: Option<usize>,
    /// Sampling temperature (the session default when `None`).
    pub temperature: Option<f32>,
}

impl InstanceConfig {
    /// An instance on `model` with session defaults.
    #[must_use]
    pub fn for_model(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            ..Self::default()
        }
    }
}

/// What `list` reports about an instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceMeta {
    /// The instance name.
    pub name: String,
    /// The model it runs on.
    pub model: String,
    /// Creation time, milliseconds since the Unix epoch.
    pub created_unix_ms: u64,
    /// Turns completed or started so far.
    pub turns: u64,
}

/// One named instance: an agent session on a shared model.
#[derive(Clone)]
pub struct InstanceHandle {
    inner: Arc<InstanceInner>,
}

struct InstanceInner {
    name: String,
    model: ModelHandle,
    session: AgentSession,
    busy: AtomicBool,
    turns: AtomicU64,
    created_unix_ms: u64,
}

impl std::fmt::Debug for InstanceHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstanceHandle")
            .field("name", &self.inner.name)
            .field("model", &self.inner.model.name())
            .finish_non_exhaustive()
    }
}

impl std::fmt::Display for InstanceHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "instance {} ({})", self.inner.name, self.inner.model)
    }
}

impl InstanceHandle {
    /// The instance name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.inner.name
    }

    /// The shared model this instance runs on.
    #[must_use]
    pub fn model(&self) -> &ModelHandle {
        &self.inner.model
    }

    /// The instance's agent session.
    #[must_use]
    pub fn session(&self) -> &AgentSession {
        &self.inner.session
    }

    /// A snapshot for listings.
    #[must_use]
    pub fn meta(&self) -> InstanceMeta {
        InstanceMeta {
            name: self.inner.name.clone(),
            model: self.inner.model.name().to_string(),
            created_unix_ms: self.inner.created_unix_ms,
            turns: self.inner.turns.load(Ordering::Relaxed),
        }
    }

    /// Claim the instance for one turn. A session runs one turn at a time;
    /// the claim is released when the returned guard drops.
    ///
    /// # Errors
    /// [`AgentRpcError::InstanceBusy`] while another turn holds the claim.
    pub fn begin_turn(&self) -> AgentRpcResult<TurnGuard> {
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
pub struct TurnGuard {
    inner: Arc<InstanceInner>,
}

impl std::fmt::Debug for TurnGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TurnGuard")
            .field("instance", &self.inner.name)
            .finish()
    }
}

impl Drop for TurnGuard {
    fn drop(&mut self) {
        self.inner.busy.store(false, Ordering::Release);
    }
}

/// The table of live instances over the server's models.
pub struct InstanceRegistry {
    models: BTreeMap<String, ModelHandle>,
    instances: RwLock<BTreeMap<String, InstanceHandle>>,
    config: RegistryConfig,
}

impl std::fmt::Debug for InstanceRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstanceRegistry")
            .field("models", &self.models.keys().collect::<Vec<_>>())
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl InstanceRegistry {
    /// A registry over `models` (each served under its own name).
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] for an empty model list or two models with
    /// the same name.
    pub fn new(
        models: impl IntoIterator<Item = ModelHandle>,
        config: RegistryConfig,
    ) -> AgentRpcResult<Self> {
        let mut table = BTreeMap::new();
        for model in models {
            let name = model.name().to_string();
            if table.insert(name.clone(), model).is_some() {
                return Err(AgentRpcError::Config(format!("model {name} registered twice")).into());
            }
        }
        if table.is_empty() {
            return Err(AgentRpcError::Config("no models to serve".into()).into());
        }
        Ok(Self {
            models: table,
            instances: RwLock::new(BTreeMap::new()),
            config,
        })
    }

    /// The hosted models, by name.
    pub fn models(&self) -> impl Iterator<Item = &ModelHandle> {
        self.models.values()
    }

    /// Create the instance `name`.
    ///
    /// # Errors
    /// [`AgentRpcError::InvalidName`], [`AgentRpcError::ModelNotFound`],
    /// [`AgentRpcError::InstanceExists`], [`AgentRpcError::InstanceLimit`], or
    /// [`AgentRpcError::Inference`] when the session cannot be built.
    pub fn create(&self, name: &str, cfg: &InstanceConfig) -> AgentRpcResult<InstanceHandle> {
        self.validate_name(name)?;
        let model = self.resolve_model(&cfg.model)?.clone();

        // Build the session before taking the write lock: it is the only part
        // that does real work, and it never loads the model.
        let session = build_session(&model, cfg)?;

        let mut instances = self.write()?;
        if instances.contains_key(name) {
            return Err(AgentRpcError::InstanceExists(name.to_string()).into());
        }
        if instances.len() >= self.config.max_instances {
            return Err(AgentRpcError::InstanceLimit(self.config.max_instances).into());
        }
        let handle = InstanceHandle {
            inner: Arc::new(InstanceInner {
                name: name.to_string(),
                model,
                session,
                busy: AtomicBool::new(false),
                turns: AtomicU64::new(0),
                created_unix_ms: unix_ms_now(),
            }),
        };
        instances.insert(name.to_string(), handle.clone());
        tracing::info!(instance = %name, model = %handle.model().name(), "instance created");
        Ok(handle)
    }

    /// The instance `name`, if it exists.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<InstanceHandle> {
        self.instances
            .read()
            .ok()
            .and_then(|table| table.get(name).cloned())
    }

    /// Every live instance, ordered by name.
    #[must_use]
    pub fn list(&self) -> Vec<InstanceMeta> {
        self.instances
            .read()
            .map(|table| table.values().map(InstanceHandle::meta).collect())
            .unwrap_or_default()
    }

    /// Destroy the instance `name`: abort any running turn, flush and end its
    /// session, and drop it from the table.
    ///
    /// # Errors
    /// [`AgentRpcError::InstanceNotFound`]; [`AgentRpcError::Inference`] when
    /// ending the session fails (the instance is removed either way).
    pub fn destroy(&self, name: &str) -> AgentRpcResult<()> {
        let removed = self.write()?.remove(name);
        let Some(handle) = removed else {
            return Err(AgentRpcError::InstanceNotFound(name.to_string()).into());
        };
        handle.session().abort();
        handle.session().end().map_err(|e| {
            ErrorTrace::new(AgentRpcError::Inference(format!(
                "ending instance {name}: {e}"
            )))
        })?;
        tracing::info!(instance = %name, "instance destroyed");
        Ok(())
    }

    fn write(
        &self,
    ) -> AgentRpcResult<std::sync::RwLockWriteGuard<'_, BTreeMap<String, InstanceHandle>>> {
        self.instances
            .write()
            .map_err(|_| AgentRpcError::Inference("instance table lock poisoned".into()).into())
    }

    fn resolve_model(&self, name: &str) -> AgentRpcResult<&ModelHandle> {
        if name.is_empty() && self.models.len() == 1 {
            if let Some(only) = self.models.values().next() {
                return Ok(only);
            }
        }
        self.models
            .get(name)
            .ok_or_else(|| AgentRpcError::ModelNotFound(name.to_string()).into())
    }

    /// Names are 1..=`max_name_len` of `[a-z0-9._-]` (ASCII, lowercase),
    /// starting with a letter or digit: no separators, traversal or control
    /// characters can reach a path, log line or header.
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
        if !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
        {
            return invalid("allowed characters are a-z 0-9 . _ -");
        }
        if !name
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphanumeric())
        {
            return invalid("must start with a letter or digit");
        }
        Ok(())
    }
}

fn build_session(model: &ModelHandle, cfg: &InstanceConfig) -> AgentRpcResult<AgentSession> {
    let mut agent = AgentConfig::default();
    if let Some(max_tokens) = cfg.max_tokens {
        agent.model_params.max_tokens = max_tokens;
    }
    if let Some(temperature) = cfg.temperature {
        agent.model_params.temperature = temperature;
    }
    let mut builder = AgentSession::builder(model.router())
        .with_model(model.model_id().clone())
        .with_config(agent);
    if let Some(prompt) = &cfg.system_prompt {
        builder = builder.with_system_prompt(prompt.clone());
    }
    builder
        .build()
        .map_err(|e| AgentRpcError::Inference(format!("building session: {e}")).into())
}

fn unix_ms_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}
