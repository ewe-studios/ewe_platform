//! The instance Durable Object (spec 83 feature 05, sections 5.6 and 5.12).
//!
//! WHY: one Durable Object (DO) holds one instance: its session, its history
//! and its memory. The Worker forwards each call with the checked principal.
//!
//! WHAT: [`AgentInstanceObject`] (the logic that the `#[durable_object]` type
//! forwards to) and [`DoStores`] (the session stores over the DO SQL).
//!
//! HOW: on the first request the object builds the side effects (`Env`), the
//! SQL stores, the remote catalog, the local registry and the core service.
//! `DestroyInstance` archives the history to R2 through the registry's destroy
//! hook before the session ends.

use std::sync::{Arc, Mutex};

use foundation_ai::agentic::KvMemoryStore;
use foundation_ai::types::SessionId;
use foundation_auth::client_auth::ForwardedPrincipalAuth;
use foundation_connectrpc::shared::auth::ClientAuthInterceptor;
use foundation_connectrpc::{Interceptor, Router};
use foundation_context::Ctx;
use foundation_db::wasm::bindgen::cf::R2Bucket;
use foundation_db::{DoSqlStore, R2WasmStorage, SqlDocumentStore, WorkerDoSql};
use foundation_errstacks::ErrorTrace;
use foundation_http::shared::app::HttpApp;
use foundation_http::wasm::bridge::cf::CfHttpApp;
use foundation_http::wasm::serve_cf::CfServe;
use foundation_netio::HttpClientBuilder;

use crate::core::archive::HistoryArchive;
use crate::core::model::{ModelCatalog, RemoteCatalog};
use crate::core::registry::{
    DestroyHookFn, InstanceRegistry, LocalInstanceRegistry, SessionStores,
};
use crate::core::service::AgentRpcService;
use crate::error::{AgentRpcError, AgentRpcResult};
use crate::host::worker::config::{ServeLimits, WorkerHostConfig};
use crate::host::worker::secrets::read_api_keys;
use crate::host::worker::serve::{incoming_from_web_request, ConnectService, ServedAgent};
use crate::host::worker::SERVICE_PATHS;
use crate::proto::v1::agent_service::register_agent_service;

/// The Durable Object binding of the history archive.
pub const ARCHIVE_BINDING: &str = "AGENT_RPC_ARCHIVE";

/// The two stores of one instance session, over the DO SQL database.
#[derive(Clone)]
pub struct DoStores {
    sql: DoSqlStore,
}

impl SessionStores for DoStores {
    type Doc = SqlDocumentStore<DoSqlStore>;
    type Mem = KvMemoryStore<DoSqlStore>;

    fn open(&self, _session: &SessionId) -> AgentRpcResult<(Self::Doc, Self::Mem)> {
        Ok((
            SqlDocumentStore::new(self.sql.clone()),
            KvMemoryStore::new(self.sql.clone()),
        ))
    }
}

/// The built parts of the object, once per DO isolate.
struct AgentInner {
    app: CfHttpApp,
    limits: ServeLimits,
}

/// The instance Durable Object logic.
pub struct AgentInstanceObject {
    state: worker::State,
    env: worker::Env,
    ctx: Ctx,
    inner: Mutex<Option<Arc<AgentInner>>>,
}

impl AgentInstanceObject {
    /// The object for one Durable Object instance.
    pub fn new(state: worker::State, env: worker::Env) -> Self {
        let ctx = Ctx::host();
        // The registry reads the identifier source and the timer from the Env.
        // A missing Env is reported when the first request builds the parts.
        let _ = foundation_core::env::install(&ctx, foundation_core::env::system);
        Self {
            state,
            env,
            ctx,
            inner: Mutex::new(None),
        }
    }

    /// Build the parts once, then reuse them.
    async fn inner(&self) -> AgentRpcResult<Arc<AgentInner>> {
        if let Some(inner) = self
            .inner
            .lock()
            .ok()
            .and_then(|slot| slot.as_ref().cloned())
        {
            return Ok(inner);
        }
        let built = Arc::new(build(&self.state, &self.env, self.ctx.clone()).await?);
        let mut slot = self
            .inner
            .lock()
            .map_err(|_| AgentRpcError::Config("the object lock is poisoned".to_string()))?;
        Ok(slot.get_or_insert_with(|| Arc::clone(&built)).clone())
    }

    /// Serve one request from the Worker's stub.
    ///
    /// # Errors
    /// A JavaScript error while reading the request or an internal error from
    /// dispatch.
    pub async fn fetch(&self, req: worker::Request) -> worker::Result<worker::Response> {
        let inner = self
            .inner()
            .await
            .map_err(|error| worker::Error::RustError(error.to_string()))?;
        // The forwarded principal header stays: the Worker wrote it after its
        // own authentication, and `ForwardedPrincipalAuth` reads it.
        let incoming = incoming_from_web_request(req.inner(), &inner.limits, false)
            .map_err(|error| worker::Error::RustError(format!("the request: {error:?}")))?;
        let response = inner
            .app
            .dispatch(self.ctx.fork(), incoming)
            .await
            .map_err(|error| worker::Error::RustError(format!("dispatch failed: {error:?}")))?;
        Ok(worker::Response::from(response))
    }

    /// The alarm entry.
    ///
    /// The instance idle time to live (`instance_idle_ttl`) is off by default,
    /// so no alarm is armed. This answers the runtime and keeps the door open
    /// for the idle sweep.
    pub async fn alarm(&self) -> worker::Result<worker::Response> {
        Ok(worker::Response::empty().map_err(|error| {
            worker::Error::RustError(format!("the empty alarm response: {error}"))
        })?)
    }
}

/// Build the parts of one Durable Object.
async fn build(state: &worker::State, env: &worker::Env, ctx: Ctx) -> AgentRpcResult<AgentInner> {
    let config = WorkerHostConfig::from_bindings(env)?;
    let sql = DoSqlStore::new(
        Arc::new(WorkerDoSql::new(state.storage().sql())),
        "agent_rpc",
    );
    sql.init_schema()
        .await
        .map_err(|error| AgentRpcError::Storage(error.to_string()))?;

    let keys = read_api_keys(env, &config.models).await?;
    let http = HttpClientBuilder::new().build();
    let catalog: Arc<dyn ModelCatalog> = Arc::new(RemoteCatalog::from_config(
        &config.models,
        &|name| keys.get(name).map(|key| key.to_string()),
        &http,
        &ctx,
    )?);

    let mut registry_config = config.registry.clone();
    registry_config.max_instances = 1;
    let stores = DoStores { sql: sql.clone() };
    let mut registry = LocalInstanceRegistry::new(
        Arc::clone(&catalog),
        stores,
        registry_config.clone(),
        config.turns.clone(),
        ctx.clone(),
    )?;
    if let Some(hook) = archive_hook(env, &config, &ctx, sql) {
        registry = registry.with_destroy_hook(hook);
    }
    let registry_trait: Arc<dyn InstanceRegistry> = Arc::new(registry);

    let service = Arc::new(AgentRpcService::new(
        registry_trait,
        Arc::clone(&catalog),
        registry_config,
        config.turns.clone(),
        vec!["connect".into(), "grpc-web".into()],
        vec!["forwarded-principal".into()],
        "worker-do",
    ));
    let interceptor: Arc<dyn Interceptor> = Arc::new(ClientAuthInterceptor::new(Arc::new(
        ForwardedPrincipalAuth::new(),
    )));
    let mut router = Router::new().with_global_interceptors(vec![interceptor]);
    register_agent_service(&mut router, service);

    let limits = ServeLimits::from(&config);
    let mut app = HttpApp::<Arc<dyn CfServe>>::with_ctx(ctx.clone());
    for path in SERVICE_PATHS {
        app.route_any_cf::<ConnectService>(path);
    }
    ctx.store(ServedAgent {
        handler: Arc::new(router.into_handler()),
        limits: limits.clone(),
    });
    Ok(AgentInner {
        app: CfHttpApp::from_app(app),
        limits,
    })
}

/// The destroy hook that archives the session history to R2.
///
/// `None` when the archive is off or the R2 binding is absent. A failed upload
/// returns `UNAVAILABLE` and the registry keeps the instance (invariant I5).
fn archive_hook(
    env: &worker::Env,
    config: &WorkerHostConfig,
    ctx: &Ctx,
    sql: DoSqlStore,
) -> Option<DestroyHookFn> {
    if !config.archive.enabled {
        return None;
    }
    let env_js = wasm_bindgen::JsValue::from(env.clone());
    let bucket = R2Bucket::from_env(&env_js, ARCHIVE_BINDING).ok()?;
    let archive = Arc::new(HistoryArchive::new(
        Arc::new(R2WasmStorage::new(bucket, "archives")),
        config.archive.clone(),
        ctx,
    ));
    Some(Arc::new(move |caller, info, session: SessionId| {
        let archive = Arc::clone(&archive);
        let sql = sql.clone();
        Box::pin(async move {
            let documents = SqlDocumentStore::new(sql);
            let session_id = session.to_string();
            archive
                .archive(
                    Arc::new(documents),
                    &session_id,
                    &caller.scope,
                    &info.name,
                    &session_id,
                    &info.model,
                )
                .await
                .map(|_| ())
                .map_err(|error| {
                    ErrorTrace::new(AgentRpcError::Transport(format!(
                        "the history archive failed: {}",
                        error.current_context()
                    )))
                })
        })
    }))
}
