//! The deterministic simulation harness for the agent RPC proof of concept.
//!
//! WHY: one seed must replay a whole client-to-instance flow, with no socket,
//! no real clock and no global random source.
//!
//! WHAT: [`ScriptedCatalog`] (the only fake: a model catalog whose handle serves
//! scripted model replies) and [`TestWorld`] (the server and client of one run),
//! plus [`TestClient`] (sync calls). The world runs the real
//! `LocalInstanceRegistry` and `AgentSession`.
//!
//! HOW: the core service runs behind `MemNetwork`; the bearer-secret provider
//! authenticates each call; enrollment runs in memory with the client-key
//! credential. Every effect comes from the simulated `Ctx`.

use std::sync::Arc;

use ed25519_compact::{KeyPair, Seed};
use foundation_agent_rpc::core::config::{EnrollConfig, RegistryConfig, TurnLimits};
use foundation_agent_rpc::core::enroll::EnrollmentService;
use foundation_agent_rpc::core::model::{
    select, AgentRpcFuture, CatalogEntry, Device, KindSupport, LoadState, ModelCapabilities,
    ModelCatalog, ModelDescriptor, ModelHandle, ModelKind, ModelRequest,
};
use foundation_agent_rpc::core::registry::{
    InMemoryStores, InstanceInfo, InstanceRegistry, LocalInstanceRegistry, SessionStores,
};
use foundation_agent_rpc::core::service::AgentRpcService;
use foundation_agent_rpc::proto::v1::__buffa::oneof::turn_event::Event;
use foundation_agent_rpc::proto::v1::agent_service::{register_agent_service, AgentServiceClient};
use foundation_agent_rpc::proto::v1::{
    ChatRequest, CreateInstanceRequest, DestroyInstanceRequest, EnrollRequest, EnrollResponse,
    HealthRequest, HealthResponse, InstanceInfo as WireInstanceInfo, ListInstancesRequest,
    ListInstancesResponse, TurnEvent,
};
use foundation_agent_rpc::{AgentRpcError, AgentRpcResult};
use foundation_ai::agentic::testing::{mock_text, MockModelProvider};
use foundation_ai::agentic::KvMemoryStore;
use foundation_ai::types::SessionId;
use foundation_auth::client_auth::{AccessPolicy, ClientAuth, SignedSecretAuth};
use foundation_auth::secret::{
    SecretDomain, SecretIssuer, SecretVerifier, SigningKey, VerifyPolicy,
};
use foundation_auth::trust::{MemoryTrustStore, TrustConfig, TrustStore, TrustedClients};
use foundation_compact::Duration;
use foundation_connectrpc_auth::ClientAuthInterceptor;
use foundation_connectrpc::testing::{MemNetwork, MemTransport};
use foundation_connectrpc::{
    ClientOptions, ConnectResult, Interceptor, Request, Router, RpcCtx, Transport,
};
use foundation_context::Ctx;
use foundation_core::valtron::sim::SimRuntime;
use foundation_db::testing::FaultStep;
use foundation_db::{MemoryDocumentStore, MemoryStorage};
use foundation_errstacks::ErrorTrace;
use foundation_netio::shared::http::SimpleHeader;

/// The words that the scripted model says.
pub const WORDS: [&str; 2] = ["Hi", " there"];

/// The fake model catalog.
///
/// It reports one model and hands out a real [`ModelHandle`] over a
/// [`MockModelProvider`] scripted with [`WORDS`]. It is the only fake in the
/// harness: the registry, the session and the turn are the production code.
pub struct ScriptedCatalog {
    kinds: Vec<KindSupport>,
    models: Vec<ModelDescriptor>,
    handle: ModelHandle,
}

impl ScriptedCatalog {
    /// A catalog with one model named `scripted`, scripted to say [`WORDS`].
    #[must_use]
    pub fn new() -> Self {
        let mut mock = MockModelProvider::new();
        mock.on_any(WORDS.iter().map(|word| mock_text(word)).collect());
        let handle = ModelHandle::from_router(
            "scripted",
            ModelKind::Api,
            Device::Remote,
            "scripted",
            mock.into_router(),
        );
        Self {
            kinds: vec![KindSupport {
                kind: ModelKind::Api,
                devices: vec![Device::Remote],
            }],
            models: vec![ModelDescriptor {
                name: "scripted".into(),
                kind: ModelKind::Api,
                device: Device::Remote,
                state: LoadState::Remote,
            }],
            handle,
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

    fn resolve<'a>(&'a self, request: &'a ModelRequest) -> AgentRpcFuture<'a, ModelHandle> {
        Box::pin(async move {
            let entries = [CatalogEntry {
                name: "scripted".into(),
                kind: ModelKind::Api,
                device: Device::Remote,
            }];
            select(&entries, &self.kinds, request).map_err(|code| {
                ErrorTrace::new(AgentRpcError::ModelSelection {
                    code,
                    detail: code.message().to_string(),
                })
            })?;
            Ok(self.handle.clone())
        })
    }
}

/// The stores of the injected-fault world.
///
/// `open` is the only step of the local registry that reports a storage fault
/// as [`AgentRpcError::Storage`], so the injected fault fails there. The text
/// comes from `foundation_db::testing::injected_error`, the same fault wording
/// as the other storage doubles of the test tree.
#[derive(Debug, Clone, Copy)]
pub struct FaultyStores {
    step: FaultStep,
}

impl SessionStores for FaultyStores {
    type Doc = MemoryDocumentStore;
    type Mem = KvMemoryStore<MemoryStorage>;

    fn open(&self, _session: &SessionId) -> AgentRpcResult<(Self::Doc, Self::Mem)> {
        Err(ErrorTrace::new(AgentRpcError::Storage(
            foundation_db::testing::injected_error(self.step, 1).to_string(),
        )))
    }
}

/// The client and server of one simulated run.
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
    /// A world with in-memory state and the real registry.
    ///
    /// # Panics
    /// Panics when the simulated stores or the test server cannot be built.
    #[must_use]
    pub fn in_memory(sim: &'a SimRuntime) -> Self {
        Self::build(sim, InMemoryStores)
    }

    /// A world whose session stores fail to open with the injected fault.
    ///
    /// # Panics
    /// As [`in_memory`](Self::in_memory).
    #[must_use]
    pub fn with_faulty_storage(sim: &'a SimRuntime, step: FaultStep) -> Self {
        Self::build(sim, FaultyStores { step })
    }

    fn build<S: SessionStores>(sim: &'a SimRuntime, stores: S) -> Self {
        let ctx = sim.ctx().clone();
        let url = "https://dst.local".to_string();
        let registry_config = RegistryConfig::default();
        let turns = TurnLimits::default();

        let catalog: Arc<dyn ModelCatalog> = Arc::new(ScriptedCatalog::new());
        let registry: Arc<dyn InstanceRegistry> = Arc::new(
            LocalInstanceRegistry::new(
                Arc::clone(&catalog),
                stores,
                registry_config.clone(),
                turns.clone(),
                ctx.clone(),
            )
            .expect("the in-memory registry"),
        );
        let service = Arc::new(AgentRpcService::new(
            Arc::clone(&registry),
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
            drive(sim, async move {
                TrustedClients::open(store, &ctx, TrustConfig::default()).await
            })
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
        drive(sim, async move { enrollment.enroll_client(&request).await })
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
        drive(
            self.sim,
            async move { enrollment.enroll_client(&request).await },
        )
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

/// A waker that unparks one thread.
struct ThreadWake(std::thread::Thread);

impl std::task::Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

/// Drive `future` on this thread and run the simulated engine between polls.
///
/// The client stays outside the engine. It pumps the engine only while it
/// waits, so a call has no task of its own. A streaming call still spawns its
/// reader task inside the engine, and the turn runs and drives the engine from
/// that task (a nested drive, issue #1128).
fn drive<F: std::future::Future>(sim: &SimRuntime, future: F) -> F::Output {
    let waker = std::task::Waker::from(Arc::new(ThreadWake(std::thread::current())));
    let mut cx = std::task::Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        match future.as_mut().poll(&mut cx) {
            std::task::Poll::Ready(value) => return value,
            std::task::Poll::Pending => {
                sim.run_until_complete();
                if !foundation_core::valtron::single::has_runnable_work() {
                    std::thread::park_timeout(Duration::from_millis(1));
                }
            }
        }
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
        drive(self.sim, async move {
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
        drive(self.sim, async move {
            client.create_instance(rpc(), Request::new(request)).await
        })
        .map(|reply| reply.msg)
    }

    /// `ListInstances`.
    ///
    /// # Errors
    /// The call error.
    pub fn list(&self) -> ConnectResult<ListInstancesResponse> {
        let client = self.build();
        drive(self.sim, async move {
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
        drive(self.sim, async move {
            client.destroy_instance(rpc(), Request::new(request)).await
        })
        .map(|_| ())
    }

    /// `ChatStream` through the real `ConnectRPC` client, collected into a
    /// vector of events.
    ///
    /// The call goes through the client, the in-memory network and the agent
    /// service, so the client reader task, the turn guard and the session are
    /// the production code. The reader task drives the turn (a nested drive);
    /// `foundation_core` stops the drive when only the reader task is left
    /// (issue #1128).
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
        drive(self.sim, async move {
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
