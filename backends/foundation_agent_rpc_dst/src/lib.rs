//! The deterministic simulation harness for the agent RPC proof of concept.
//!
//! WHY: one seed must replay a whole client-to-instance flow, with no socket,
//! no real clock and no global random source.
//!
//! WHAT: [`ScriptedCatalog`] (a fake model catalog), [`ScriptedRegistry`] (an
//! in-memory instance registry with a scripted turn) and [`TestWorld`] (the
//! in-memory server and client of one run), plus [`TestClient`] (sync calls).
//!
//! HOW: the core service runs behind `MemNetwork`; the bearer-secret provider
//! authenticates each call; enrollment runs in memory with the client-key
//! credential. Every effect comes from the simulated `Ctx`.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use ed25519_compact::{KeyPair, Seed};
use foundation_agent_rpc::core::config::{EnrollConfig, RegistryConfig, TurnLimits};
use foundation_agent_rpc::core::enroll::EnrollmentService;
use foundation_agent_rpc::core::model::{
    AgentRpcFuture, Device, KindSupport, LoadState, ModelCapabilities, ModelCatalog,
    ModelDescriptor, ModelHandle, ModelKind, ModelRequest,
};
use foundation_agent_rpc::core::registry::{
    Caller, InstanceInfo, InstanceRegistry, InstanceSpec, Page, TurnEventStream,
};
use foundation_agent_rpc::core::service::AgentRpcService;
use foundation_agent_rpc::proto::v1::__buffa::oneof::turn_event::Event;
use foundation_agent_rpc::proto::v1::agent_service::{register_agent_service, AgentServiceClient};
use foundation_agent_rpc::proto::v1::{
    ChatRequest, CreateInstanceRequest, DestroyInstanceRequest, EnrollRequest, EnrollResponse,
    HealthRequest, HealthResponse, InstanceInfo as WireInstanceInfo, ListInstancesRequest,
    ListInstancesResponse, TurnDone, TurnEvent,
};
use foundation_agent_rpc::{AgentRpcError, AgentRpcResult};
use foundation_auth::client_auth::{AccessPolicy, ClientAuth, SignedSecretAuth};
use foundation_auth::secret::{
    SecretDomain, SecretIssuer, SecretVerifier, SigningKey, VerifyPolicy,
};
use foundation_auth::trust::{MemoryTrustStore, TrustConfig, TrustStore, TrustedClients};
use foundation_compact::Duration;
use foundation_connectrpc::shared::auth::ClientAuthInterceptor;
use foundation_connectrpc::testing::{MemNetwork, MemTransport};
use foundation_connectrpc::{
    ClientOptions, ConnectResult, Interceptor, Request, Router, RpcCtx, Transport,
};
use foundation_context::Ctx;
use foundation_core::valtron::sim::SimRuntime;
use foundation_db::testing::FaultStep;
use foundation_errstacks::ErrorTrace;
use foundation_netio::shared::http::SimpleHeader;

/// The words that the scripted turn says.
pub const WORDS: [&str; 2] = ["Hi", " there"];

/// The fake model catalog.
///
/// It reports one model so `Health` and `GetCapabilities` have an answer. The
/// turn itself comes from [`ScriptedRegistry`], so `resolve` is never called.
pub struct ScriptedCatalog {
    kinds: Vec<KindSupport>,
    models: Vec<ModelDescriptor>,
}

impl ScriptedCatalog {
    /// A catalog with one model named `scripted`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            kinds: vec![KindSupport {
                kind: ModelKind::Api,
                devices: vec![Device::Remote],
            }],
            models: vec![ModelDescriptor {
                name: "scripted".into(),
                kind: ModelKind::Api,
                device: Device::Remote,
                state: LoadState::Loaded,
            }],
        }
    }
}

impl Default for ScriptedCatalog {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelCatalog for ScriptedCatalog {
    fn capabilities(&self) -> ModelCapabilities {
        ModelCapabilities {
            kinds: self.kinds.clone(),
            models: self.models.clone(),
        }
    }

    fn resolve<'a>(&'a self, _request: &'a ModelRequest) -> AgentRpcFuture<'a, ModelHandle> {
        Box::pin(async move {
            Err(ErrorTrace::new(AgentRpcError::Unsupported(
                "the scripted registry runs the turn; no model loads".to_string(),
            )))
        })
    }
}

/// An in-memory instance registry with a scripted turn.
pub struct ScriptedRegistry {
    ctx: Ctx,
    words: Vec<String>,
    fail: Option<FaultStep>,
    instances: Mutex<BTreeMap<String, InstanceInfo>>,
}

impl ScriptedRegistry {
    /// A registry that says `words` on each turn.
    #[must_use]
    pub fn new(ctx: Ctx, words: &[&str]) -> Self {
        Self {
            ctx,
            words: words.iter().map(|word| (*word).to_string()).collect(),
            fail: None,
            instances: Mutex::new(BTreeMap::new()),
        }
    }

    /// A registry whose next storage step fails at `step`.
    #[must_use]
    pub fn with_failure(mut self, step: FaultStep) -> Self {
        self.fail = Some(step);
        self
    }

    fn info(&self, name: &str) -> InstanceInfo {
        InstanceInfo {
            name: name.to_string(),
            model: "scripted".into(),
            kind: ModelKind::Api,
            device: Device::Remote,
            created_unix_ms: u64::try_from(foundation_compact::time::unix_millis(
                self.ctx.clock().wall_now(),
            ))
            .unwrap_or(0),
            turns: 0,
        }
    }

    fn fail_if_injected(&self) -> AgentRpcResult<()> {
        match self.fail {
            Some(FaultStep::Append) => Err(ErrorTrace::new(AgentRpcError::Storage(
                "injected storage failure at append call 1".to_string(),
            ))),
            Some(other) => Err(ErrorTrace::new(AgentRpcError::Storage(format!(
                "injected storage failure at {}",
                other.name()
            )))),
            None => Ok(()),
        }
    }
}

impl InstanceRegistry for ScriptedRegistry {
    fn create<'a>(
        &'a self,
        _caller: &'a Caller,
        spec: InstanceSpec,
    ) -> AgentRpcFuture<'a, InstanceInfo> {
        Box::pin(async move {
            let mut instances = self
                .instances
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if instances.contains_key(&spec.name) {
                return Err(ErrorTrace::new(AgentRpcError::InstanceExists(spec.name)));
            }
            let info = self.info(&spec.name);
            instances.insert(spec.name, info.clone());
            Ok(info)
        })
    }

    fn get<'a>(&'a self, _caller: &'a Caller, name: &'a str) -> AgentRpcFuture<'a, InstanceInfo> {
        Box::pin(async move {
            self.instances
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(name)
                .cloned()
                .ok_or_else(|| ErrorTrace::new(AgentRpcError::InstanceNotFound(name.to_string())))
        })
    }

    fn list<'a>(
        &'a self,
        _caller: &'a Caller,
        _page_size: u32,
        _page_token: Option<&'a str>,
    ) -> AgentRpcFuture<'a, Page<InstanceInfo>> {
        Box::pin(async move {
            let items = self
                .instances
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .values()
                .cloned()
                .collect();
            Ok(Page {
                items,
                next_token: None,
            })
        })
    }

    fn destroy<'a>(&'a self, _caller: &'a Caller, name: &'a str) -> AgentRpcFuture<'a, ()> {
        Box::pin(async move {
            let removed = self
                .instances
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(name);
            if removed.is_none() {
                return Err(ErrorTrace::new(AgentRpcError::InstanceNotFound(
                    name.to_string(),
                )));
            }
            Ok(())
        })
    }

    fn start_turn<'a>(
        &'a self,
        _caller: &'a Caller,
        name: &'a str,
        _prompt: String,
    ) -> AgentRpcFuture<'a, TurnEventStream> {
        Box::pin(async move {
            if !self
                .instances
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .contains_key(name)
            {
                return Err(ErrorTrace::new(AgentRpcError::InstanceNotFound(
                    name.to_string(),
                )));
            }
            self.fail_if_injected()?;
            let mut events: Vec<AgentRpcResult<TurnEvent>> = self
                .words
                .iter()
                .map(|word| {
                    Ok(TurnEvent {
                        event: Some(Event::Text(word.clone())),
                        ..Default::default()
                    })
                })
                .collect();
            events.push(Ok(TurnEvent {
                event: Some(Event::Done(Box::new(TurnDone {
                    message_count: 1,
                    ..Default::default()
                }))),
                ..Default::default()
            }));
            Ok(Box::pin(futures::stream::iter(events)) as TurnEventStream)
        })
    }

    fn count<'a>(&'a self, _caller: &'a Caller) -> AgentRpcFuture<'a, u64> {
        Box::pin(async move {
            Ok(u64::try_from(
                self.instances
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .len(),
            )
            .unwrap_or(u64::MAX))
        })
    }
}

/// The server and the client of one simulated run.
pub struct TestWorld<'a> {
    sim: &'a SimRuntime,
    enrollment: Arc<EnrollmentService>,
    transport: Arc<dyn Transport>,
    url: String,
    default_access: String,
    trust: Arc<TrustedClients>,
    service: Arc<AgentRpcService>,
}

impl std::fmt::Debug for TestWorld<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestWorld")
            .field("url", &self.url)
            .finish_non_exhaustive()
    }
}

impl<'a> TestWorld<'a> {
    /// A world with in-memory state.
    ///
    /// # Panics
    /// Panics when the simulated stores or the test server cannot be built.
    #[must_use]
    pub fn in_memory(sim: &'a SimRuntime) -> Self {
        Self::build(sim, ScriptedRegistry::new(sim.ctx().clone(), &WORDS))
    }

    /// A world whose next storage step fails at `step`.
    ///
    /// # Panics
    /// As [`in_memory`](Self::in_memory).
    #[must_use]
    pub fn with_faulty_storage(sim: &'a SimRuntime, step: FaultStep) -> Self {
        Self::build(
            sim,
            ScriptedRegistry::new(sim.ctx().clone(), &WORDS).with_failure(step),
        )
    }

    fn build(sim: &'a SimRuntime, registry: ScriptedRegistry) -> Self {
        let ctx = sim.ctx().clone();
        let url = "https://dst.local".to_string();
        let registry_config = RegistryConfig::default();
        let turns = TurnLimits::default();

        let catalog = Arc::new(ScriptedCatalog::new());
        let registry: Arc<dyn InstanceRegistry> = Arc::new(registry);
        let service = Arc::new(AgentRpcService::new(
            registry,
            catalog,
            registry_config,
            turns,
            vec!["connect".into(), "grpc".into()],
            vec!["bearer".into()],
            "dst",
        ));

        let (enrollment, client_auth, trust) = Self::auth(sim, &ctx, &url);

        let interceptor: Arc<dyn Interceptor> =
            Arc::new(ClientAuthInterceptor::new(Arc::clone(&client_auth)));
        let mut router = Router::new().with_global_interceptors(vec![interceptor]);
        register_agent_service(&mut router, Arc::clone(&service));
        let network = Arc::new(MemNetwork::new(router, ctx));
        let transport: Arc<dyn Transport> = Arc::new(MemTransport::new(network));

        let default = Self::default_enrollment(sim, &enrollment);

        Self {
            sim,
            enrollment,
            transport,
            url,
            default_access: default.access_secret,
            trust,
            service,
        }
    }

    /// The enrollment service, the bearer-secret provider and the trust store
    /// of one run.
    fn auth(
        sim: &SimRuntime,
        ctx: &Ctx,
        url: &str,
    ) -> (
        Arc<EnrollmentService>,
        Arc<dyn ClientAuth>,
        Arc<TrustedClients>,
    ) {
        let signing = SigningKey::generate_ed25519(ctx, None).expect("the simulated signing key");
        let verifying = signing.verifying_key().clone();
        let store: Arc<dyn TrustStore> = Arc::new(MemoryTrustStore::new());
        let trust = {
            let ctx = ctx.clone();
            sim.block_on(
                async move { TrustedClients::open(store, &ctx, TrustConfig::default()).await },
            )
            .expect("the in-memory trust store")
        };

        let access_verifier = SecretVerifier::new(
            SecretDomain::AGENT_RPC,
            vec![verifying.clone()],
            ctx.clone(),
            VerifyPolicy::access(url.to_string()),
        );
        let client_auth: Arc<dyn ClientAuth> = Arc::new(SignedSecretAuth::new(
            access_verifier,
            Arc::clone(&trust),
            AccessPolicy::default(),
        ));

        let mut enroll_policy = VerifyPolicy::enroll();
        enroll_policy.audience = Some(url.to_string());
        let enroll_verifier = SecretVerifier::new(
            SecretDomain::AGENT_RPC,
            vec![verifying],
            ctx.clone(),
            enroll_policy,
        );
        let signer = SecretIssuer::new(
            SecretDomain::AGENT_RPC,
            signing,
            ctx.clone(),
            Duration::from_hours(24),
        );
        let enrollment = Arc::new(
            EnrollmentService::new(
                enroll_verifier,
                signer,
                None,
                Arc::clone(&trust),
                ctx.clone(),
                EnrollConfig::default(),
            )
            .expect("the simulated enrollment service"),
        );
        (enrollment, client_auth, trust)
    }

    /// The client that the default calls use, enrolled in memory.
    fn default_enrollment(sim: &SimRuntime, enrollment: &Arc<EnrollmentService>) -> EnrollResponse {
        let enrollment = Arc::clone(enrollment);
        let env = sim.ctx().require_env().expect("the simulated Env");
        let mut seed = [0_u8; 32];
        env.entropy().fill(&mut seed).expect("seeded entropy");
        let key = KeyPair::from_seed(Seed::new(seed));
        let token = enrollment
            .issue_enroll_secret(Duration::from_secs(600))
            .expect("the default enrollment secret");
        let request = EnrollRequest {
            token: token.encode().as_str().to_string(),
            client_name: "dst-default".into(),
            credential: Some(
                foundation_agent_rpc::proto::v1::__buffa::oneof::enroll_request::Credential::ClientKey(
                    Box::new(foundation_agent_rpc::proto::v1::ClientKey {
                        alg: "ed25519".into(),
                        public_key: key.pk[..].to_vec(),
                        ..Default::default()
                    }),
                ),
            ),
            nonce: seed.to_vec(),
            ..Default::default()
        };
        sim.block_on(async move { enrollment.enroll_client(&request).await })
            .expect("the default client enrolls")
    }

    /// The seed of the run.
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.sim.seed()
    }

    /// The agent service (for a shutdown test).
    #[must_use]
    pub fn service(&self) -> &Arc<AgentRpcService> {
        &self.service
    }

    /// The trusted clients (for a revocation test).
    #[must_use]
    pub fn trusted_clients(&self) -> &Arc<TrustedClients> {
        &self.trust
    }

    /// Issue one enrollment secret, valid for `validity`.
    ///
    /// # Errors
    /// The enrollment service error.
    pub fn issue_secret(
        &self,
        validity: Duration,
    ) -> AgentRpcResult<foundation_auth::secret::SignedSecret> {
        self.enrollment.issue_enroll_secret(validity)
    }

    /// Enroll one client with a client key and return its access secret.
    ///
    /// # Errors
    /// The enrollment error (a spent secret, a bad key, a store failure).
    ///
    /// # Panics
    /// Panics when the simulated context holds no effect bundle, or when its
    /// seeded entropy source fails.
    pub fn enroll(&self, name: &str) -> AgentRpcResult<EnrollResponse> {
        let env = self.sim.ctx().require_env().expect("the simulated Env");
        let mut seed = [0_u8; 32];
        env.entropy().fill(&mut seed).expect("seeded entropy");
        let key = KeyPair::from_seed(Seed::new(seed));
        let mut nonce = [0_u8; 32];
        env.entropy().fill(&mut nonce).expect("seeded entropy");
        let token = self
            .enrollment
            .issue_enroll_secret(Duration::from_secs(600))?;
        let request = EnrollRequest {
            token: token.encode().as_str().to_string(),
            client_name: name.to_string(),
            credential: Some(
                foundation_agent_rpc::proto::v1::__buffa::oneof::enroll_request::Credential::ClientKey(
                    Box::new(foundation_agent_rpc::proto::v1::ClientKey {
                        alg: "ed25519".into(),
                        public_key: key.pk[..].to_vec(),
                        ..Default::default()
                    }),
                ),
            ),
            nonce: nonce.to_vec(),
            ..Default::default()
        };
        let enrollment = Arc::clone(&self.enrollment);
        self.sim
            .block_on(async move { enrollment.enroll_client(&request).await })
    }

    /// A client with the access secret that the world enrolled at build time.
    #[must_use]
    pub fn client(&self) -> TestClient<'a> {
        self.client_with(&self.default_access)
    }

    /// A client with `access` as its bearer secret.
    #[must_use]
    pub fn client_with(&self, access: &str) -> TestClient<'a> {
        let options = ClientOptions::new()
            .with_header(SimpleHeader::AUTHORIZATION, format!("Bearer {access}"));
        TestClient {
            sim: self.sim,
            transport: Arc::clone(&self.transport),
            url: self.url.clone(),
            options,
        }
    }

    /// The default access secret, for a test that builds its own client.
    #[must_use]
    pub fn default_access(&self) -> &str {
        &self.default_access
    }
}

/// Sync calls over one simulated client.
pub struct TestClient<'a> {
    sim: &'a SimRuntime,
    transport: Arc<dyn Transport>,
    url: String,
    options: ClientOptions,
}

impl std::fmt::Debug for TestClient<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestClient").finish_non_exhaustive()
    }
}

impl TestClient<'_> {
    fn build(&self) -> AgentServiceClient {
        AgentServiceClient::new(Arc::clone(&self.transport), &self.url, self.options.clone())
            .expect("the simulated client builds")
    }

    /// `Health`.
    ///
    /// # Errors
    /// The call error.
    pub fn health(&self) -> ConnectResult<HealthResponse> {
        let client = self.build();
        self.sim
            .block_on(async move {
                client
                    .health(rpc(), Request::new(HealthRequest::default()))
                    .await
            })
            .map(|reply| reply.msg)
    }

    /// `CreateInstance`.
    ///
    /// # Errors
    /// The call error.
    pub fn create(&self, name: &str) -> ConnectResult<WireInstanceInfo> {
        let client = self.build();
        let request = CreateInstanceRequest {
            name: name.into(),
            ..Default::default()
        };
        self.sim
            .block_on(async move { client.create_instance(rpc(), Request::new(request)).await })
            .map(|reply| reply.msg)
    }

    /// `ListInstances`.
    ///
    /// # Errors
    /// The call error.
    pub fn list(&self) -> ConnectResult<ListInstancesResponse> {
        let client = self.build();
        self.sim
            .block_on(async move {
                client
                    .list_instances(rpc(), Request::new(ListInstancesRequest::default()))
                    .await
            })
            .map(|reply| reply.msg)
    }

    /// `DestroyInstance`.
    ///
    /// # Errors
    /// The call error.
    pub fn destroy(&self, name: &str) -> ConnectResult<()> {
        let client = self.build();
        let request = DestroyInstanceRequest {
            name: name.into(),
            ..Default::default()
        };
        self.sim
            .block_on(async move { client.destroy_instance(rpc(), Request::new(request)).await })
            .map(|_| ())
    }

    /// `ChatStream`, collected into a vector of events.
    ///
    /// # Errors
    /// The call error.
    pub fn chat_stream(&self, instance: &str, prompt: &str) -> ConnectResult<Vec<TurnEvent>> {
        let client = self.build();
        let request = ChatRequest {
            instance: instance.into(),
            prompt: prompt.into(),
            ..Default::default()
        };
        self.sim.block_on(async move {
            let mut stream = client.chat_stream(rpc(), Request::new(request)).await?;
            let mut events = Vec::new();
            while let Some(event) = stream.receive().await? {
                events.push(event);
            }
            Ok(events)
        })
    }
}

/// The text that a collected turn stream carried.
#[must_use]
pub fn text_of(events: &[TurnEvent]) -> String {
    events
        .iter()
        .filter_map(|event| match &event.event {
            Some(Event::Text(text) | Event::Thinking(text)) => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

/// A deadline for one simulated call. The in-memory dispatcher runs no timer,
/// so the value only documents the intent.
fn rpc() -> RpcCtx {
    RpcCtx::background().with_deadline(Duration::from_secs(60))
}

/// The wire instance info of an `InstanceInfo`, for a trace.
#[must_use]
pub fn wire_info(info: &InstanceInfo) -> WireInstanceInfo {
    WireInstanceInfo {
        name: info.name.clone(),
        model: info.model.clone(),
        created_unix_ms: info.created_unix_ms,
        turns: info.turns,
        ..Default::default()
    }
}
