//! The instance Durable Object (spec 83 feature 05, sections 5.6 and 5.12).
//!
//! WHY: one Durable Object (DO) holds one instance: its session, its history
//! and its memory. The Worker forwards each call with the checked principal.
//! A DO may restart; the instance records and the session identifier live in
//! the DO's SQL storage, so a new object rebuilds the registry before it
//! serves the first call (issue #976).
//!
//! WHAT: [`AgentInstanceObject`] (the logic that the `#[durable_object]` type
//! forwards to) and [`DoStores`] (the session stores over the DO SQL).
//!
//! HOW: on the first request the object builds the side effects (`Env`), the
//! SQL stores, the remote catalog, the local registry and the core service.
//! It then loads the instance row of the forwarded principal and restores the
//! session. `CreateInstance` records the row; `DestroyInstance` archives the
//! history to R2 and then deletes the row.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use foundation_ai::agentic::KvMemoryStore;
use foundation_ai::types::SessionId;
use foundation_auth::client_auth::{AuthMethod, ForwardedPrincipalAuth, Principal};
use foundation_compact::ids::Id;
use foundation_connectrpc::shared::auth::ClientAuthInterceptor;
use foundation_connectrpc::{Interceptor, Router};
use foundation_context::Ctx;
use foundation_db::wasm::bindgen::cf::R2Bucket;
use foundation_db::{
    DataValue, DoSqlStore, QueryStore, R2WasmStorage, SqlDocumentStore, WorkerDoSql,
};
use foundation_errstacks::ErrorTrace;
use foundation_http::shared::app::HttpApp;
use foundation_http::wasm::bridge::cf::CfHttpApp;
use foundation_http::wasm::serve_cf::CfServe;
use foundation_netio::HttpClientBuilder;

use crate::core::archive::HistoryArchive;
use crate::core::model::{Device, ModelCatalog, ModelKind, ModelRequest, RemoteCatalog};
use crate::core::registry::{
    CreateHookFn, DestroyHookFn, InstanceInfo, InstanceRegistry, InstanceSpec,
    LocalInstanceRegistry, ScopePolicy, SessionStores,
};
use crate::core::service::AgentRpcService;
use crate::error::{AgentRpcError, AgentRpcResult};
use crate::host::worker::config::{ServeLimits, WorkerHostConfig};
use crate::host::worker::do_client::FORWARDED_PRINCIPAL_HEADER;
use crate::host::worker::secrets::read_api_keys;
use crate::host::worker::serve::{
    incoming_from_web_request, ConnectService, HealthCheck, ServedAgent,
};
use crate::host::worker::SERVICE_PATHS;
use crate::proto::v1::agent_service::register_agent_service;

/// The Durable Object binding of the history archive.
pub const ARCHIVE_BINDING: &str = "AGENT_RPC_ARCHIVE";

/// The table of the instance records. One DO holds one instance, but the scope
/// key keeps two owners apart.
pub const INSTANCE_TABLE: &str = "agent_instances";

/// The Worker var that enables the test-only reset endpoint.
pub const TEST_RESET_VAR: &str = "AGENT_RPC_TEST_RESET";

/// The test-only header that makes the object drop its in-memory parts.
pub const TEST_RESET_HEADER: &str = "ewe-test-reset";

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
    registry: Arc<LocalInstanceRegistry<DoStores>>,
    sql: DoSqlStore,
    scope_policy: ScopePolicy,
}

/// The instance Durable Object logic.
pub struct AgentInstanceObject {
    state: worker::State,
    env: worker::Env,
    ctx: Ctx,
    inner: Mutex<Option<Arc<AgentInner>>>,
    /// Whether the instance records of this object are already restored.
    restored: AtomicBool,
    /// Whether the test-only reset endpoint is enabled.
    test_reset: bool,
}

impl AgentInstanceObject {
    /// The object for one Durable Object instance.
    pub fn new(state: worker::State, env: worker::Env) -> Self {
        let ctx = Ctx::host();
        // The registry reads the identifier source and the timer from the Env.
        // A missing Env is reported when the first request builds the parts.
        let _ = foundation_core::env::install(&ctx, foundation_core::env::system);
        let test_reset = env
            .var(TEST_RESET_VAR)
            .map(|value| value.to_string() == "1")
            .unwrap_or(false);
        Self {
            state,
            env,
            ctx,
            inner: Mutex::new(None),
            restored: AtomicBool::new(false),
            test_reset,
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

        // Test hook: drop the in-memory parts, as a new object would start.
        if self.test_reset
            && req
                .headers()
                .get(TEST_RESET_HEADER)
                .ok()
                .flatten()
                .as_deref()
                == Some("1")
        {
            if let Ok(mut slot) = self.inner.lock() {
                *slot = None;
            }
            self.restored.store(false, Ordering::Release);
            return worker::Response::ok("reset")
                .map_err(|error| worker::Error::RustError(format!("the reset reply: {error}")));
        }

        // A new object rebuilds its registry from the records before it serves
        // the call, so the instance and its session survive a restart.
        if !self.restored.load(Ordering::Acquire) {
            if let Ok(Some(subject)) = req.headers().get(FORWARDED_PRINCIPAL_HEADER) {
                restore_instances(&inner, &subject)
                    .await
                    .map_err(|error| worker::Error::RustError(format!("restore: {error}")))?;
                self.restored.store(true, Ordering::Release);
            }
        }

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
    sql.execute(
        &format!(
            "CREATE TABLE IF NOT EXISTS {INSTANCE_TABLE} (\
             scope TEXT NOT NULL, \
             name TEXT NOT NULL, \
             model TEXT NOT NULL, \
             session_id TEXT NOT NULL, \
             system_prompt TEXT, \
             max_tokens INTEGER, \
             temperature REAL, \
             created_ms INTEGER NOT NULL, \
             turns INTEGER NOT NULL, \
             PRIMARY KEY (scope, name))"
        ),
        &[],
    )
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
    let scope_policy = registry_config.scope_policy;
    let stores = DoStores { sql: sql.clone() };
    let registry = LocalInstanceRegistry::new(
        Arc::clone(&catalog),
        stores,
        registry_config.clone(),
        config.turns.clone(),
        ctx.clone(),
    )?
    .with_create_hook(create_hook(sql.clone()))
    .with_destroy_hook(destroy_hook(
        sql.clone(),
        archive_target(env, &config, &ctx),
    ));
    let registry = Arc::new(registry);
    let registry_trait: Arc<dyn InstanceRegistry> = registry.clone();

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
    app.route_any_cf::<HealthCheck>("/healthz");
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
        registry,
        sql,
        scope_policy,
    })
}

/// The create hook: record the instance and its session.
fn create_hook(sql: DoSqlStore) -> CreateHookFn {
    Arc::new(move |caller, info, session, spec: InstanceSpec| {
        let sql = sql.clone();
        Box::pin(async move {
            let sql_text = format!(
                "INSERT INTO {INSTANCE_TABLE} \
                 (scope, name, model, session_id, system_prompt, max_tokens, temperature, \
                  created_ms, turns) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
            );
            sql.execute(
                &sql_text,
                &[
                    DataValue::Text(caller.scope.as_key().to_string()),
                    DataValue::Text(info.name),
                    DataValue::Text(info.model),
                    DataValue::Text(session.to_string()),
                    optional_text(spec.system_prompt.as_deref()),
                    optional_integer(spec.max_tokens),
                    optional_real(spec.temperature),
                    DataValue::Integer(i64::try_from(info.created_unix_ms).unwrap_or(0)),
                    DataValue::Integer(i64::try_from(info.turns).unwrap_or(0)),
                ],
            )
            .await
            .map(|_| ())
            .map_err(|error| ErrorTrace::new(AgentRpcError::Storage(error.to_string())))
        })
    })
}

/// The destroy hook: archive the history, then delete the record.
fn destroy_hook(sql: DoSqlStore, archive: Option<Arc<HistoryArchive>>) -> DestroyHookFn {
    Arc::new(move |caller, info, session: SessionId| {
        let sql = sql.clone();
        let archive = archive.clone();
        Box::pin(async move {
            if let Some(archive) = archive {
                let documents = SqlDocumentStore::new(sql.clone());
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
                    .map_err(|error| {
                        ErrorTrace::new(AgentRpcError::Transport(format!(
                            "the history archive failed: {}",
                            error.current_context()
                        )))
                    })?;
            }
            sql.execute(
                &format!("DELETE FROM {INSTANCE_TABLE} WHERE scope = ? AND name = ?"),
                &[
                    DataValue::Text(caller.scope.as_key().to_string()),
                    DataValue::Text(info.name),
                ],
            )
            .await
            .map(|_| ())
            .map_err(|error| ErrorTrace::new(AgentRpcError::Storage(error.to_string())))
        })
    })
}

/// The archive target, when the archive is on and the R2 binding exists.
fn archive_target(
    env: &worker::Env,
    config: &WorkerHostConfig,
    ctx: &Ctx,
) -> Option<Arc<HistoryArchive>> {
    if !config.archive.enabled {
        return None;
    }
    let env_js = wasm_bindgen::JsValue::from(env.clone());
    let bucket = R2Bucket::from_env(&env_js, ARCHIVE_BINDING).ok()?;
    Some(Arc::new(HistoryArchive::new(
        Arc::new(R2WasmStorage::new(bucket, "archives").with_part_size(config.archive.part_size)),
        config.archive.clone(),
        ctx,
    )))
}

/// Rebuild every instance record of `subject`.
async fn restore_instances(inner: &AgentInner, subject: &str) -> AgentRpcResult<()> {
    let principal = Principal {
        subject: subject.to_string(),
        method: AuthMethod::Forwarded,
        roles: Vec::new(),
    };
    let caller = inner.scope_policy.caller(principal);
    let rows = inner
        .sql
        .query(
            &format!(
                "SELECT name, model, session_id, system_prompt, max_tokens, temperature, \
                 created_ms, turns FROM {INSTANCE_TABLE} WHERE scope = ?"
            ),
            &[DataValue::Text(caller.scope.as_key().to_string())],
        )
        .await
        .map_err(|error| AgentRpcError::Storage(error.to_string()))?
        .collect_all()
        .await
        .map_err(|error| AgentRpcError::Storage(error.to_string()))?;
    for row in rows {
        let name = row
            .get_by_name::<String>("name")
            .map_err(|error| AgentRpcError::Storage(error.to_string()))?;
        let model = row
            .get_by_name::<String>("model")
            .map_err(|error| AgentRpcError::Storage(error.to_string()))?;
        let session_text = row
            .get_by_name::<String>("session_id")
            .map_err(|error| AgentRpcError::Storage(error.to_string()))?;
        let session_id = session_text.parse::<Id>().map_err(|error| {
            AgentRpcError::Storage(format!("the recorded session id is bad: {error}"))
        })?;
        let spec = InstanceSpec {
            name: name.clone(),
            model: ModelRequest {
                name: Some(model.clone()),
                kind: None,
                device: None,
            },
            system_prompt: row
                .get_by_name::<Option<String>>("system_prompt")
                .ok()
                .flatten(),
            max_tokens: row
                .get_by_name::<Option<i64>>("max_tokens")
                .ok()
                .flatten()
                .and_then(|tokens| usize::try_from(tokens).ok()),
            temperature: row
                .get_by_name::<Option<f64>>("temperature")
                .ok()
                .flatten()
                .map(|value| value as f32),
        };
        let info = InstanceInfo {
            name,
            model,
            kind: ModelKind::Api,
            device: Device::Remote,
            created_unix_ms: u64::try_from(row.get_by_name::<i64>("created_ms").unwrap_or(0))
                .unwrap_or(0),
            turns: u64::try_from(row.get_by_name::<i64>("turns").unwrap_or(0)).unwrap_or(0),
        };
        inner
            .registry
            .restore(&caller, &info, SessionId::from_id(session_id), &spec)
            .await?;
    }
    Ok(())
}

fn optional_text(value: Option<&str>) -> DataValue {
    value.map_or(DataValue::Null, |text| DataValue::Text(text.to_string()))
}

fn optional_integer(value: Option<usize>) -> DataValue {
    value
        .and_then(|number| i64::try_from(number).ok())
        .map_or(DataValue::Null, DataValue::Integer)
}

fn optional_real(value: Option<f32>) -> DataValue {
    value.map_or(DataValue::Null, |number| DataValue::Real(f64::from(number)))
}
