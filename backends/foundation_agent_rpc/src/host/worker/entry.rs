//! The Worker entry: one isolate-wide host over `CfHttpApp` (spec 83 feature
//! 05, section 5.3).
//!
//! WHY: the Worker is the public endpoint. It checks the caller, keeps the
//! instance index in D1 and forwards each call to the instance Durable Object.
//! It holds no instance state between requests (invariant I1).
//!
//! WHAT: [`WorkerHost`].
//!
//! HOW: build the trust store on D1, the bearer-secret verifier and the auth
//! providers once per isolate. The Connect handler and its limits travel in the
//! `Ctx`. `fetch` answers a preflight, removes every incoming
//! `ewe-forwarded-*` header, copies the Cloudflare client-certificate facts and
//! dispatches. The response body stays a `ReadableStream`.

use std::sync::Arc;

use foundation_auth::client_auth::{
    AccessPolicy, AnyOf, CfMtlsAuth, CfTlsClientAuth, ClientAuth, SignedSecretAuth,
};
use foundation_auth::secret::{
    SecretDomain, SecretVerifier, SigningAlgorithm, VerifyPolicy, VerifyingKey,
};
use foundation_auth::trust::{SqlTrustStore, TrustedClients};
use foundation_connectrpc::{Interceptor, Router};
use foundation_connectrpc_auth::ClientAuthInterceptor;
use foundation_context::Ctx;
use foundation_db::wasm::bindgen::cf::D1Database;
use foundation_db::D1WasmStorage;
use foundation_http::shared::app::HttpApp;
use foundation_http::wasm::bridge::cf::CfHttpApp;
use foundation_http::wasm::serve_cf::CfServe;
use foundation_netio::HttpClientBuilder;
use wasm_bindgen::JsValue;

use crate::core::cors::CorsConfig;
use crate::core::model::{ModelCatalog, RemoteCatalog};
use crate::core::registry::{InMemoryStores, InstanceRegistry, LocalInstanceRegistry};
use crate::core::service::AgentRpcService;
use crate::error::{AgentRpcError, AgentRpcResult};
use crate::host::worker::config::{ServeLimits, WorkerHostConfig};
use crate::host::worker::do_client::InstanceStub;
use crate::host::worker::object::TEST_RESET_VAR;
use crate::host::worker::registry::DurableObjectRegistry;
use crate::host::worker::secrets::read_api_keys;
use crate::host::worker::serve::{
    incoming_from_web_request, ConnectService, HealthCheck, ServedAgent,
};
use crate::host::worker::SERVICE_PATHS;
use crate::proto::v1::agent_service::register_agent_service;

/// The vars and bindings that the Worker reads.
pub const PUBLIC_KEYS_VAR: &str = "AGENT_RPC_SECRET_PUBLIC_KEYS";
/// The D1 binding of the trust store and the instance index.
pub const DATABASE_BINDING: &str = "AGENT_RPC_DB";

/// The Worker host: the isolate-wide parts of one Worker.
pub struct WorkerHost {
    env: worker::Env,
    ctx: Ctx,
    config: WorkerHostConfig,
    app: CfHttpApp,
    limits: ServeLimits,
    /// The D1 index of the full host. A thin orchestrator has no index.
    registry: Option<Arc<DurableObjectRegistry>>,
    /// Whether the test-only reset endpoint is enabled.
    test_reset: bool,
}

impl std::fmt::Debug for WorkerHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerHost")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl WorkerHost {
    /// Build the full host once per isolate.
    ///
    /// The host keeps the instance index in D1 and each session in the
    /// instance Durable Object. Use
    /// [`new_orchestrator`](Self::new_orchestrator) for the thin orchestrator
    /// on a Worker.
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] for a bad var, [`AgentRpcError::Storage`] for
    /// a D1 failure and [`AgentRpcError::Auth`] for bad public keys.
    pub async fn new(env: worker::Env, ctx: Ctx) -> AgentRpcResult<Self> {
        let config = WorkerHostConfig::from_bindings(&env)?;
        reject_enrollment(&config)?;
        let env_js = JsValue::from(env.clone());
        let d1 = D1Database::from_env(&env_js, DATABASE_BINDING).map_err(|error| {
            AgentRpcError::Config(format!(
                "the D1 binding {DATABASE_BINDING} is missing: {error:?}"
            ))
        })?;
        let storage = Arc::new(D1WasmStorage::new(Arc::new(d1.clone()), "agent_rpc"));
        let trust_store = SqlTrustStore::new(Arc::clone(&storage), "agent_rpc")
            .map_err(|error| AgentRpcError::Storage(error.to_string()))?;
        let trust = TrustedClients::open(Arc::new(trust_store), &ctx, config.trust.clone())
            .await
            .map_err(|error| AgentRpcError::Storage(error.to_string()))?;
        let catalog = build_catalog(&env, &config, &ctx).await?;
        let limits = ServeLimits::from(&config);
        let registry = Arc::new(
            DurableObjectRegistry::new(
                env.clone(),
                d1,
                config.registry.clone(),
                limits.clone(),
                ctx.clone(),
            )
            .await?,
        );
        let registry_trait: Arc<dyn InstanceRegistry> = registry.clone();
        Self::assemble(
            env,
            ctx,
            config,
            trust,
            catalog,
            registry_trait,
            Some(registry),
        )
    }

    /// Build the thin orchestrator once per isolate.
    ///
    /// The thin orchestrator keeps its instances in this isolate, not in a
    /// Durable Object, and it reads its trust store from memory. It serves the
    /// agentic layer and the remote model providers over the same Connect
    /// router as the full host.
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] for a bad var, and
    /// [`AgentRpcError::Auth`] for bad public keys. The caller installs the
    /// `Env` first, because the local registry reads its identifiers and its
    /// timer from it.
    pub async fn new_orchestrator(env: worker::Env, ctx: Ctx) -> AgentRpcResult<Self> {
        let config = WorkerHostConfig::from_bindings(&env)?;
        reject_enrollment(&config)?;
        let trust = TrustedClients::open(
            Arc::new(foundation_auth::trust::MemoryTrustStore::new()),
            &ctx,
            config.trust.clone(),
        )
        .await
        .map_err(|error| AgentRpcError::Storage(error.to_string()))?;
        let catalog = build_catalog(&env, &config, &ctx).await?;
        let registry = Arc::new(LocalInstanceRegistry::new(
            Arc::clone(&catalog),
            InMemoryStores,
            config.registry.clone(),
            config.turns.clone(),
            ctx.clone(),
        )?);
        let registry_trait: Arc<dyn InstanceRegistry> = registry;
        Self::assemble(env, ctx, config, trust, catalog, registry_trait, None)
    }

    /// Build the shared parts over one registry and one trust store.
    fn assemble(
        env: worker::Env,
        ctx: Ctx,
        config: WorkerHostConfig,
        trust: Arc<TrustedClients>,
        catalog: Arc<dyn ModelCatalog>,
        registry: Arc<dyn InstanceRegistry>,
        index: Option<Arc<DurableObjectRegistry>>,
    ) -> AgentRpcResult<Self> {
        let verifier = access_verifier(&env, &config, &ctx)?;
        let mut providers: Vec<Arc<dyn ClientAuth>> = Vec::new();
        if config.cloudflare_mtls {
            providers.push(Arc::new(CfMtlsAuth::new(Arc::clone(&trust), &ctx)));
        }
        providers.push(Arc::new(SignedSecretAuth::new(
            verifier,
            trust,
            AccessPolicy::default(),
        )));

        let limits = ServeLimits::from(&config);
        let service = Arc::new(AgentRpcService::new(
            registry,
            catalog,
            config.registry.clone(),
            config.turns.clone(),
            vec!["connect".into(), "grpc-web".into()],
            providers
                .iter()
                .map(|provider| provider.mode().to_string())
                .collect(),
            "worker",
        ));
        let interceptor: Arc<dyn Interceptor> =
            Arc::new(ClientAuthInterceptor::new(Arc::new(AnyOf::new(providers))));
        let mut router = Router::new().with_global_interceptors(vec![interceptor]);
        register_agent_service(&mut router, service);

        let mut app = HttpApp::<Arc<dyn CfServe>>::with_ctx(ctx.clone());
        app.route_any_cf::<HealthCheck>("/healthz");
        for path in SERVICE_PATHS {
            app.route_any_cf::<ConnectService>(path);
        }
        ctx.store(ServedAgent {
            handler: Arc::new(router.into_handler()),
            limits: limits.clone(),
        });
        let test_reset = env
            .var(TEST_RESET_VAR)
            .is_ok_and(|value| value.to_string() == "1");
        Ok(Self {
            env,
            ctx,
            config,
            app: CfHttpApp::from_app(app),
            limits,
            registry: index,
            test_reset,
        })
    }

    /// Serve one request from the internet.
    ///
    /// # Errors
    /// A JavaScript error while building the response.
    pub async fn fetch(&self, req: worker::Request) -> worker::Result<worker::Response> {
        // Test hook: drop the in-memory parts of an instance object, as a new
        // object starts. It is enabled only by the `AGENT_RPC_TEST_RESET` var.
        if self.test_reset && req.path() == "/__test/reset" {
            let name = req
                .headers()
                .get("x-ewe-instance")
                .ok()
                .flatten()
                .unwrap_or_default();
            if name.is_empty() {
                return worker::Response::error("missing x-ewe-instance", 400)
                    .map_err(|error| worker::Error::RustError(format!("the reply: {error}")));
            }
            let Some(registry) = self.registry.as_ref() else {
                return worker::Response::error("this host has no instance index", 404)
                    .map_err(|error| worker::Error::RustError(format!("the reply: {error}")));
            };
            let object_name = registry
                .object_key_for_name(&name)
                .await
                .map_err(|error| worker::Error::RustError(error.to_string()))?;
            InstanceStub::new(&self.env, &object_name, "reset")
                .map_err(|error| worker::Error::RustError(error.to_string()))?
                .reset()
                .await
                .map_err(|error| worker::Error::RustError(error.to_string()))?;
            return worker::Response::ok("reset")
                .map_err(|error| worker::Error::RustError(format!("the reply: {error}")));
        }
        let origin = req
            .headers()
            .get("origin")
            .ok()
            .flatten()
            .unwrap_or_default();
        if req.method() == worker::Method::Options {
            return preflight(&self.config.cors, &origin)
                .map_err(|error| worker::Error::RustError(format!("the preflight: {error}")));
        }

        let mut incoming = incoming_from_web_request(req.inner(), &self.limits, true)
            .map_err(|error| worker::Error::RustError(format!("the request: {error:?}")))?;
        if let Some(tls) = req.cf().and_then(worker::Cf::tls_client_auth) {
            let facts = tls_client_auth(&tls);
            if let Some(extensions) = incoming.extensions.as_mut() {
                extensions.insert(facts);
            }
        }
        let response = self
            .app
            .dispatch(self.ctx.fork(), incoming)
            .await
            .map_err(|error| worker::Error::RustError(format!("dispatch failed: {error:?}")))?;
        decorate(&response, &self.config.cors, &origin);
        Ok(worker::Response::from(response))
    }
}

/// Refuse a Worker that asks to serve enrollment.
///
/// The browser enrollment flow needs a host that signs server proofs; that is
/// the native host or the separate enrollment feature, not this Worker entry.
fn reject_enrollment(config: &WorkerHostConfig) -> AgentRpcResult<()> {
    if config.enrollment.is_some() {
        return Err(AgentRpcError::Unsupported(
            "enrollment on a Worker is not implemented; serve BootstrapService from the \
             native host or leave `enrollment` off"
                .to_string(),
        )
        .into());
    }
    Ok(())
}

/// The remote model catalog of this Worker.
///
/// The keys are Worker secrets or Secrets Store secrets. A missing secret
/// stops the start with an error that names the secret, never a value.
async fn build_catalog(
    env: &worker::Env,
    config: &WorkerHostConfig,
    ctx: &Ctx,
) -> AgentRpcResult<Arc<dyn ModelCatalog>> {
    let keys = read_api_keys(env, &config.models).await?;
    let http = HttpClientBuilder::new().build();
    Ok(Arc::new(RemoteCatalog::from_config(
        &config.models,
        &|name| keys.get(name).map(|key| key.as_str().to_owned()),
        &http,
        ctx,
    )?))
}

/// Read the public keys of the bearer-secret verifier.
fn access_verifier(
    env: &worker::Env,
    config: &WorkerHostConfig,
    ctx: &Ctx,
) -> AgentRpcResult<SecretVerifier> {
    let text = env
        .var(PUBLIC_KEYS_VAR)
        .map(|value| value.to_string())
        .map_err(|_| {
            AgentRpcError::Config(format!("the Worker var {PUBLIC_KEYS_VAR} is missing"))
        })?;
    let entries: Vec<KeyEntry> = serde_json::from_str(&text).map_err(|error| {
        AgentRpcError::Config(format!("{PUBLIC_KEYS_VAR} is not a JSON key list: {error}"))
    })?;
    let mut keys = Vec::with_capacity(entries.len());
    for entry in entries {
        let algorithm = match entry.alg.as_str() {
            "ed25519" => SigningAlgorithm::Ed25519,
            "rsa-pss-sha256" => SigningAlgorithm::RsaPssSha256,
            other => {
                return Err(AgentRpcError::Config(format!(
                    "{PUBLIC_KEYS_VAR} names the unknown algorithm {other:?}"
                ))
                .into())
            }
        };
        let key = VerifyingKey::from_base64url(algorithm, &entry.public_key, entry.kid)
            .map_err(|error| AgentRpcError::Auth(error.to_string()))?;
        keys.push(key);
    }
    if keys.is_empty() {
        return Err(AgentRpcError::Config(format!("{PUBLIC_KEYS_VAR} holds no key")).into());
    }
    Ok(SecretVerifier::new(
        SecretDomain::AGENT_RPC,
        keys,
        ctx.clone(),
        VerifyPolicy::access(config.public_url.clone()),
    ))
}

#[derive(serde::Deserialize)]
struct KeyEntry {
    #[serde(default)]
    kid: Option<String>,
    alg: String,
    public_key: String,
}

/// The CORS preflight answer.
fn preflight(config: &CorsConfig, origin: &str) -> Result<worker::Response, String> {
    let headers = config.preflight_headers(origin);
    let init = web_sys::ResponseInit::new();
    init.set_status(204);
    let web_headers = web_sys::Headers::new().map_err(|error| format!("{error:?}"))?;
    for (name, value) in &headers {
        let _ = web_headers.append(name, value);
    }
    init.set_headers(&web_headers);
    let response = web_sys::Response::new_with_opt_str_and_init(None, &init)
        .map_err(|error| format!("{error:?}"))?;
    Ok(worker::Response::from(response))
}

/// Add the CORS headers of `origin` to a response.
fn decorate(response: &web_sys::Response, config: &CorsConfig, origin: &str) {
    let headers = response.headers();
    for (name, value) in config.response_headers(origin) {
        let _ = headers.append(&name, &value);
    }
}

/// Copy the Cloudflare client-certificate facts for `CfMtlsAuth`.
fn tls_client_auth(tls: &worker::TlsClientAuth) -> CfTlsClientAuth {
    let presented = matches!(tls.cert_presented().as_str(), "1" | "true");
    CfTlsClientAuth {
        cert_presented: presented,
        cert_verified: tls.cert_verified(),
        cert_fingerprint_sha256: tls.cert_fingerprint_sha256(),
        cert_subject_dn: tls.cert_subject_dn(),
        cert_not_after: tls.cert_not_after(),
        cert_serial: tls.cert_serial(),
    }
}
