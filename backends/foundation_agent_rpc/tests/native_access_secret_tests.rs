//! An access secret (bearer) on the agent listener, with no client certificate.

#![cfg(unix)]
#![recursion_limit = "256"]

use std::sync::Arc;
use std::time::Duration;

use ed25519_compact::{KeyPair, Seed};
use foundation_agent_rpc::core::model::{
    select, AgentRpcFuture, CatalogEntry, Device, KindSupport, LoadState, ModelCapabilities,
    ModelCatalog, ModelDescriptor, ModelHandle, ModelKind, ModelRequest,
};
use foundation_agent_rpc::host::native::{
    NativeHost, NativeHostConfig, NativeSecurity, SIGNING_KEY_FILE,
};
use foundation_agent_rpc::proto::v1::agent_service::AgentServiceClient;
use foundation_agent_rpc::proto::v1::bootstrap_service::BootstrapServiceClient;
use foundation_agent_rpc::proto::v1::{ClientKey, EnrollRequest, HealthRequest};
use foundation_agent_rpc::AgentRpcError;
use foundation_ai::agentic::testing::{mock_text, MockModelProvider};
use foundation_auth::pki::{CaKey, KeyAlgorithm};
use foundation_auth::secret::SigningKey;
use foundation_connectrpc::{ClientOptions, H1Transport, Request, RpcCtx, Transport};
use foundation_context::{Ctx, MonotonicClock};
use foundation_core::valtron::valtron_test;
use foundation_netio::netcap::ssl::SSLConnector;
use foundation_netio::shared::http::SimpleHeader;
use foundation_netio::HttpClientBuilder;

struct OneModel {
    handle: ModelHandle,
    entry: CatalogEntry,
}

impl OneModel {
    fn new() -> Self {
        let mut mock = MockModelProvider::new();
        mock.on_any(vec![mock_text("hi")]);
        Self {
            handle: ModelHandle::from_router(
                "mock",
                ModelKind::Api,
                Device::Remote,
                "mock",
                mock.into_router(),
            ),
            entry: CatalogEntry {
                name: "mock".into(),
                kind: ModelKind::Api,
                device: Device::Remote,
            },
        }
    }
}

impl ModelCatalog for OneModel {
    fn capabilities(&self) -> ModelCapabilities {
        ModelCapabilities {
            kinds: vec![KindSupport {
                kind: ModelKind::Api,
                devices: vec![Device::Remote],
            }],
            models: vec![ModelDescriptor {
                name: "mock".into(),
                kind: ModelKind::Api,
                device: Device::Remote,
                state: LoadState::Loaded,
            }],
        }
    }

    fn resolve<'a>(&'a self, request: &'a ModelRequest) -> AgentRpcFuture<'a, ModelHandle> {
        Box::pin(async move {
            select(
                std::slice::from_ref(&self.entry),
                &self.capabilities().kinds,
                request,
            )
            .map_err(|code| {
                foundation_errstacks::ErrorTrace::new(AgentRpcError::ModelSelection {
                    code,
                    detail: "no match".into(),
                })
            })?;
            Ok(self.handle.clone())
        })
    }
}

fn ctx() -> Ctx {
    let ctx = Ctx::fixed(Arc::new(MonotonicClock::new()));
    foundation_core::env::install(&ctx, foundation_core::env::system).expect("Env");
    ctx
}

fn write_operator_keys(dir: &std::path::Path, ctx: &Ctx) {
    let signing = SigningKey::generate_ed25519(ctx, None).expect("signing key");
    std::fs::write(
        dir.join(SIGNING_KEY_FILE),
        signing.to_pkcs8_pem().as_bytes(),
    )
    .expect("signing key file");
    let ca = CaKey::generate(KeyAlgorithm::Ed25519, ctx).expect("ca key");
    std::fs::write(dir.join("ca-key.pem"), ca.to_pem().as_bytes()).expect("ca key file");
    let server = CaKey::generate(KeyAlgorithm::Ed25519, ctx).expect("server key");
    std::fs::write(dir.join("server-key.pem"), server.to_pem().as_bytes())
        .expect("server key file");
}

fn pinned_transport(fingerprint: &str) -> Arc<dyn Transport> {
    let config = foundation_auth::mtls::client_config_pinned_default(fingerprint)
        .expect("the pinned client configuration");
    let connector = SSLConnector::with_config(config);
    Arc::new(H1Transport::new(
        HttpClientBuilder::new()
            .with_tls_connector(connector)
            .build(),
    ))
}

fn rpc() -> RpcCtx {
    RpcCtx::background().with_deadline(Duration::from_secs(30))
}

#[valtron_test(threads = 8)]
async fn native_access_secret_is_accepted_without_a_client_certificate() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = ctx();
    write_operator_keys(dir.path(), &ctx);
    let mut config = NativeHostConfig::default()
        .with_state_dir(dir.path())
        .with_bind("127.0.0.1:0")
        .with_catalog(Arc::new(OneModel::new()))
        .with_preload(false)
        .with_security(NativeSecurity::Mtls {
            accept_access_secrets: true,
        });
    config.http.shutdown_grace = Duration::from_millis(250);
    let server = NativeHost::start(config, ctx.clone())
        .await
        .expect("host starts");

    let fingerprint = server
        .tls_fingerprint()
        .expect("the fingerprint")
        .to_string();
    let bootstrap_url = server.bootstrap_url().expect("the enroll URL").to_string();
    let token = server
        .issue_enroll_secret(Duration::from_secs(600))
        .expect("an enrollment secret");

    let key = KeyPair::from_seed(Seed::new([7_u8; 32]));
    let bootstrap = BootstrapServiceClient::new(
        pinned_transport(&fingerprint),
        &bootstrap_url,
        ClientOptions::new(),
    )
    .expect("the bootstrap client");
    let reply = bootstrap
        .enroll(
            rpc(),
            Request::new(EnrollRequest {
                token: token.encode().as_str().to_string(),
                client_name: "browser".into(),
                credential: Some(
                    foundation_agent_rpc::proto::v1::__buffa::oneof::enroll_request::Credential::ClientKey(
                        Box::new(ClientKey {
                            alg: "ed25519".into(),
                            public_key: key.pk[..].to_vec(),
                            ..Default::default()
                        }),
                    ),
                ),
                nonce: [9_u8; 32].to_vec(),
                ..Default::default()
            }),
        )
        .await
        .expect("the client enrolls with a key")
        .msg;
    assert!(
        !reply.access_secret.is_empty(),
        "enrollment returns an access secret"
    );

    // The agent listener: TLS server authentication only, plus the bearer
    // secret. No client certificate is presented.
    let options = ClientOptions::new().with_header(
        SimpleHeader::AUTHORIZATION,
        format!("Bearer {}", reply.access_secret),
    );
    let client =
        AgentServiceClient::new(pinned_transport(&fingerprint), server.base_url(), options)
            .expect("the bearer client");
    let health = client
        .health(rpc(), Request::new(HealthRequest::default()))
        .await
        .expect("the bearer secret is accepted");
    assert_eq!(health.msg.status, "serving");

    drop(client);
    drop(bootstrap);
    server.graceful_shutdown().expect("clean shutdown");
}
