//! Social login end to end against a local upstream provider simulator.
//!
//! WHY a simulator: no local form of `Google`, `GitHub`, `Facebook` or
//! `Twitter` exists. `ory/hydra` is a different `OAuth2` server. It cannot serve
//! GitHub's list of emails or the provider-specific JavaScript Object Notation
//! (JSON), so it cannot stand in for the four providers. The simulator is a
//! test double for the provider only. The `IdP`, its upstream client, the
//! redirect chain and the token exchange are the real code over real TCP.
//!
//! WHAT: [`Simulator`] is a real HTTP server on `127.0.0.1:0`, built with
//! `foundation_http`. It serves the routes of one provider. It records every
//! request (method, path, query, headers, form fields). It holds the
//! authorization code, the Proof Key for Code Exchange (PKCE) challenge and
//! the OIDC nonce for one flow. It signs an ID token with a test Ed25519 key
//! and publishes the matching JSON Web Key Set (JWKS).
//!
//! HOW: each test starts the simulator and a real [`IdpServer`] whose issuer is
//! the test server origin. It stores an upstream provider and an app client,
//! then walks the redirect chain with raw HTTP requests so every `302` is
//! visible. It ends by exchanging the `IdP` code at `POST /idp/token`.
//!
//! The tests use raw HTTP because the standard client follows redirects. A
//! small response here is read whole and parsed, which the streaming rule
//! allows for a test.

#![cfg(feature = "social-integration-tests")]

use std::fmt::Write as _;
use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use foundation_auth::server::models::{OAuthClient, User};
use foundation_auth::server::services::{ProviderCrypto, ProviderService};
use foundation_auth::server::storage::{self, HandlerStorage};
use foundation_auth::server::{IdpConfig, IdpServer};
use foundation_auth::shared::auth_session::memory::MemoryAuthSessionStore;
use foundation_auth::{JwtSigningKey, ProviderMapping, ProviderType, UpstreamProvider};
use foundation_context::{Ctx, MonotonicClock};
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::valtron::{valtron_test, BlockingFutureExt};
use foundation_db::{MemoryStorage, QueryStore};
use foundation_http::native::serve::{Serve, ServeFactory};
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::serve::ConnectionResult;
use foundation_http::ShutdownSignal;
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::client::body_reader::try_collect_bytes;
use foundation_netio::shared::http::{
    Http11, RenderHttp, SendSafeBody, SimpleHeader, SimpleIncomingRequest, SimpleOutgoingResponse,
    Status,
};
use jwt_simple::prelude::Ed25519KeyPair;
use serde_json::{json, Value};

// ── Constants ───────────────────────────────────────────────────────────

const APP_CLIENT_ID: &str = "test-app";
const APP_CLIENT_SECRET: &str = "test-secret";
const APP_REDIRECT_URI: &str = "https://app.example/callback";
const PROVIDER_CLIENT_ID: &str = "provider-client-id";
const PROVIDER_CLIENT_SECRET: &str = "provider-client-secret";
const KEY_ID: &str = "sim-key";

const DISCOVERY_PATH: &str = "/.well-known/openid-configuration";
const JWKS_PATH: &str = "/jwks";
const AUTHORIZE_PATH: &str = "/authorize";
const TOKEN_PATH: &str = "/token";
const USERINFO_PATH: &str = "/userinfo";
const GITHUB_USER_PATH: &str = "/user";
const GITHUB_EMAILS_PATH: &str = "/user/emails";

// ── Provider scenarios ──────────────────────────────────────────────────

/// The provider wire shape the simulator speaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Google,
    GitHub,
    Facebook,
    Twitter,
}

impl Kind {
    fn id(self) -> &'static str {
        match self {
            Self::Google => "google",
            Self::GitHub => "github",
            Self::Facebook => "facebook",
            Self::Twitter => "twitter",
        }
    }

    fn provider_type(self) -> ProviderType {
        match self {
            Self::Google => ProviderType::Oidc,
            Self::GitHub | Self::Facebook | Self::Twitter => ProviderType::Oauth2,
        }
    }

    fn discovery_path(self) -> Option<&'static str> {
        match self {
            Self::Google => Some(DISCOVERY_PATH),
            Self::GitHub | Self::Facebook | Self::Twitter => None,
        }
    }

    fn userinfo_path(self) -> &'static str {
        match self {
            Self::GitHub => GITHUB_USER_PATH,
            Self::Google | Self::Facebook | Self::Twitter => USERINFO_PATH,
        }
    }

    fn mapping(self) -> ProviderMapping {
        match self {
            Self::GitHub => ProviderMapping {
                email_field: "email".into(),
                email_verified_field: "email_verified".into(),
                name_field: "name".into(),
                // GitHub's numeric `id` is not a string; `login` is stable too.
                subject_field: "login".into(),
                username_field: Some("login".into()),
            },
            Self::Twitter => ProviderMapping {
                // Twitter/X wraps its user object in `data`.
                email_field: "data.email".into(),
                email_verified_field: "data.email_verified".into(),
                name_field: "data.name".into(),
                subject_field: "data.id".into(),
                username_field: Some("data.username".into()),
            },
            Self::Facebook => ProviderMapping {
                // The Graph API uses `id`, not the OIDC `sub`.
                email_field: "email".into(),
                email_verified_field: "email_verified".into(),
                name_field: "name".into(),
                subject_field: "id".into(),
                username_field: None,
            },
            Self::Google => ProviderMapping::default(),
        }
    }

    fn scopes(self) -> Vec<String> {
        match self {
            Self::Google => vec!["openid".into(), "email".into(), "profile".into()],
            Self::GitHub => vec!["read:user".into(), "user:email".into()],
            Self::Facebook => vec!["email".into(), "public_profile".into()],
            Self::Twitter => vec!["users.read".into(), "tweet.read".into()],
        }
    }

    fn access_token(self) -> String {
        format!("sim-access-{}", self.id())
    }

    fn userinfo(self) -> Value {
        match self {
            Self::Google => json!({
                "sub": "google-sub-1",
                "email": "alice@example.com",
                "email_verified": true,
                "name": "Alice Example",
                "preferred_username": "alice",
            }),
            Self::GitHub => json!({
                "id": 12_345,
                "login": "octocat",
                "name": "The Octocat",
                "email": null,
            }),
            Self::Facebook => json!({
                "id": "fb-123",
                "name": "Fiona Book",
                "email": "fiona@example.com",
            }),
            Self::Twitter => json!({
                "data": {
                    "id": "tw-99",
                    "name": "Tina Tweet",
                    "username": "tinatweet",
                }
            }),
        }
    }
}

// ── Recorded requests ───────────────────────────────────────────────────

/// One request the simulator received.
#[derive(Clone, Debug)]
struct Recorded {
    method: String,
    path: String,
    query: Vec<(String, String)>,
    headers: Vec<(String, String)>,
    form: Vec<(String, String)>,
}

impl Recorded {
    fn query_param(&self, name: &str) -> Option<&str> {
        pair(&self.query, name)
    }

    fn form_param(&self, name: &str) -> Option<&str> {
        pair(&self.form, name)
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

// ── Simulator state ─────────────────────────────────────────────────────

#[derive(Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "the test toggles independent failure modes of one flow"
)]
struct Flow {
    code: Option<String>,
    pkce_challenge: Option<String>,
    nonce: Option<String>,
    redirect_uri: Option<String>,
    bad_signature: bool,
    wrong_nonce: bool,
    wrong_audience: bool,
    wrong_issuer: bool,
    omit_id_token: bool,
}

struct SimState {
    kind: Kind,
    origin: String,
    ctx: Ctx,
    signing: JwtSigningKey,
    flow: Mutex<Flow>,
    requests: Mutex<Vec<Recorded>>,
    last_token_response: Mutex<Option<Value>>,
}

impl SimState {
    fn public_key_bytes(&self) -> Vec<u8> {
        match &self.signing {
            JwtSigningKey::Ed25519(kp) => kp.public_key().to_bytes(),
            _ => unreachable!("the simulator signs with Ed25519"),
        }
    }

    fn jwks(&self) -> Value {
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
        json!({
            "keys": [{
                "kty": "OKP",
                "crv": "Ed25519",
                "use": "sig",
                "kid": KEY_ID,
                "alg": "EdDSA",
                "x": URL_SAFE_NO_PAD.encode(self.public_key_bytes()),
            }]
        })
    }

    fn discovery(&self) -> Value {
        json!({
            "issuer": self.origin,
            "authorization_endpoint": format!("{}{}", self.origin, AUTHORIZE_PATH),
            "token_endpoint": format!("{}{}", self.origin, TOKEN_PATH),
            "userinfo_endpoint": format!("{}{}", self.origin, self.kind.userinfo_path()),
            "jwks_uri": format!("{}{}", self.origin, JWKS_PATH),
            "response_types_supported": ["code"],
            "subject_types_supported": ["public"],
            "id_token_signing_alg_values_supported": ["EdDSA"],
            "code_challenge_methods_supported": ["S256"],
            "scopes_supported": ["openid", "profile", "email"],
        })
    }

    fn sign_id_token(&self, claims: &Value) -> String {
        let bad = self.flow.lock().unwrap().bad_signature;
        if bad {
            let key = JwtSigningKey::Ed25519(Ed25519KeyPair::generate().with_key_id(KEY_ID));
            key.sign_claims(claims, &self.ctx)
                .expect("sign bad id_token")
        } else {
            self.signing
                .sign_claims(claims, &self.ctx)
                .expect("sign id_token")
        }
    }
}

// ── Simulator HTTP handler ──────────────────────────────────────────────

struct SimHandler {
    state: Arc<SimState>,
}

impl ServeFactory for SimHandler {
    fn create(ctx: &Ctx) -> Self {
        let state = ctx
            .get::<Arc<SimState>>()
            .expect("Arc<SimState> must be in the app Ctx");
        Self {
            state: Arc::clone(&*state),
        }
    }
}

impl Serve for SimHandler {
    fn serve(
        &self,
        _ctx: Ctx,
        mut req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let url = req.request_url.url.clone();
        let (path, query_text) = url.split_once('?').unwrap_or((url.as_str(), ""));
        let query = parse_pairs(query_text);
        let body = req.body.take().map_or_else(String::new, read_body);
        let form = parse_pairs(&body);
        let headers = req
            .headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.join(", ")))
            .collect();
        let recorded = Recorded {
            method: req.method.to_string(),
            path: path.to_string(),
            query,
            headers,
            form,
        };
        self.state.requests.lock().unwrap().push(recorded.clone());

        let kind = self.state.kind;

        match path {
            DISCOVERY_PATH if kind.discovery_path().is_some() => {
                respond(conn, 200, Vec::new(), self.state.discovery())
            }
            JWKS_PATH => respond(conn, 200, Vec::new(), self.state.jwks()),
            AUTHORIZE_PATH => self.authorize(conn, &recorded),
            TOKEN_PATH => self.token(conn, &recorded),
            p if p == kind.userinfo_path() => self.userinfo(conn, &recorded),
            GITHUB_EMAILS_PATH if kind == Kind::GitHub => {
                respond(conn, 200, Vec::new(), github_emails())
            }
            _ => respond(
                conn,
                404,
                Vec::new(),
                json!({ "error": "unknown route", "path": path }),
            ),
        }
    }
}

impl SimHandler {
    fn authorize(
        &self,
        conn: SharedByteBufferStream<RawStream>,
        req: &Recorded,
    ) -> ConnectionResult {
        let (Some(state), Some(redirect_uri)) =
            (req.query_param("state"), req.query_param("redirect_uri"))
        else {
            return respond(conn, 400, Vec::new(), json!({ "error": "missing state" }));
        };
        let code = "sim-auth-code".to_string();
        {
            let mut flow = self.state.flow.lock().unwrap();
            flow.code = Some(code.clone());
            flow.pkce_challenge = req.query_param("code_challenge").map(String::from);
            flow.nonce = req.query_param("nonce").map(String::from);
            flow.redirect_uri = Some(redirect_uri.to_string());
        }
        let separator = if redirect_uri.contains('?') { '&' } else { '?' };
        let location = format!(
            "{redirect_uri}{separator}code={}&state={}",
            enc(&code),
            enc(state)
        );
        respond_redirect(conn, &location)
    }

    fn token(&self, conn: SharedByteBufferStream<RawStream>, req: &Recorded) -> ConnectionResult {
        let (code, redirect_uri) = {
            let flow = self.state.flow.lock().unwrap();
            (
                flow.code.clone(),
                flow.redirect_uri.clone().unwrap_or_default(),
            )
        };
        if req.form_param("code") != code.as_deref() {
            return respond(conn, 400, Vec::new(), json!({ "error": "bad code" }));
        }
        if req.form_param("redirect_uri") != Some(redirect_uri.as_str()) {
            return respond(
                conn,
                400,
                Vec::new(),
                json!({ "error": "redirect_uri mismatch" }),
            );
        }
        if req.form_param("client_id") != Some(PROVIDER_CLIENT_ID) {
            return respond(conn, 400, Vec::new(), json!({ "error": "bad client_id" }));
        }
        // A confidential client must prove itself. A missing or wrong secret
        // is a rejected exchange, as a real provider does.
        if req.form_param("client_secret") != Some(PROVIDER_CLIENT_SECRET) {
            return respond(conn, 401, Vec::new(), json!({ "error": "invalid_client" }));
        }
        {
            let flow = self.state.flow.lock().unwrap();
            if let Some(challenge) = &flow.pkce_challenge {
                let Some(verifier) = req.form_param("code_verifier") else {
                    return respond(
                        conn,
                        400,
                        Vec::new(),
                        json!({ "error": "missing code_verifier" }),
                    );
                };
                if &s256(verifier) != challenge {
                    return respond(conn, 400, Vec::new(), json!({ "error": "PKCE mismatch" }));
                }
            }
        }

        let kind = self.state.kind;
        let mut token = json!({
            "access_token": kind.access_token(),
            "token_type": "Bearer",
            "expires_in": 3600,
            "scope": kind.scopes().join(" "),
        });
        match kind {
            Kind::GitHub => {
                token["token_type"] = json!("bearer");
                token.as_object_mut().unwrap().remove("expires_in");
            }
            Kind::Facebook => {
                token["token_type"] = json!("bearer");
                token["expires_in"] = json!(5_184_000);
            }
            Kind::Twitter => {
                token["expires_in"] = json!(7_200);
                token["refresh_token"] = json!("sim-refresh-twitter");
            }
            Kind::Google => {
                let (nonce, audience, issuer, omit) = {
                    let flow = self.state.flow.lock().unwrap();
                    let nonce = if flow.wrong_nonce {
                        "wrong-nonce".to_string()
                    } else {
                        flow.nonce.clone().unwrap_or_default()
                    };
                    let audience = if flow.wrong_audience {
                        "some-other-client".to_string()
                    } else {
                        PROVIDER_CLIENT_ID.to_string()
                    };
                    let issuer = if flow.wrong_issuer {
                        "https://evil.example".to_string()
                    } else {
                        self.state.origin.clone()
                    };
                    (nonce, audience, issuer, flow.omit_id_token)
                };
                if !omit {
                    let now = now_seconds(&self.state.ctx);
                    let claims = json!({
                        "iss": issuer,
                        "sub": "google-sub-1",
                        "aud": audience,
                        "nonce": nonce,
                        "email": "alice@example.com",
                        "email_verified": true,
                        "name": "Alice Example",
                        "exp": now + 300,
                        "iat": now,
                    });
                    token["id_token"] = json!(self.state.sign_id_token(&claims));
                }
                token["refresh_token"] = json!("sim-refresh-google");
            }
        }
        *self.state.last_token_response.lock().unwrap() = Some(token.clone());
        respond(conn, 200, Vec::new(), token)
    }

    fn userinfo(
        &self,
        conn: SharedByteBufferStream<RawStream>,
        req: &Recorded,
    ) -> ConnectionResult {
        let expected = format!("Bearer {}", self.state.kind.access_token());
        if req.header("authorization") != Some(expected.as_str()) {
            return respond(conn, 401, Vec::new(), json!({ "error": "bad token" }));
        }
        respond(conn, 200, Vec::new(), self.state.kind.userinfo())
    }
}

// ── Simulator handle ────────────────────────────────────────────────────

struct Simulator {
    origin: String,
    state: Arc<SimState>,
    shutdown: Arc<ShutdownSignal>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for Simulator {
    fn drop(&mut self) {
        self.shutdown.turn_on();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Simulator {
    fn start(ctx: Ctx, kind: Kind) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind simulator");
        let addr = listener.local_addr().expect("simulator address");
        let origin = format!("http://{addr}");
        let state = Arc::new(SimState {
            kind,
            origin: origin.clone(),
            ctx: ctx.clone(),
            signing: JwtSigningKey::Ed25519(Ed25519KeyPair::generate().with_key_id(KEY_ID)),
            flow: Mutex::new(Flow::default()),
            requests: Mutex::new(Vec::new()),
            last_token_response: Mutex::new(None),
        });

        let mut app: HttpApp<Arc<dyn Serve>> = HttpApp::new_serve();
        app.ctx = ctx;
        app.ctx.store(Arc::clone(&state));
        for path in [
            DISCOVERY_PATH,
            JWKS_PATH,
            AUTHORIZE_PATH,
            TOKEN_PATH,
            USERINFO_PATH,
            GITHUB_USER_PATH,
            GITHUB_EMAILS_PATH,
        ] {
            app.route_any::<SimHandler>(path);
        }
        let server = HttpServer::with_config(app, &addr.to_string(), ServerConfig::defaults());
        let shutdown = Arc::new(ShutdownSignal::new());
        let shutdown_thread = Arc::clone(&shutdown);
        let handle = std::thread::spawn(move || {
            server.serve_with_listener(&listener, &shutdown_thread);
        });
        Self {
            origin,
            state,
            shutdown,
            handle: Some(handle),
        }
    }

    fn provider_row(&self) -> UpstreamProvider {
        let kind = self.state.kind;
        let explicit = |path: &str| format!("{}{path}", self.origin);
        UpstreamProvider {
            id: kind.id().to_string(),
            name: kind.id().to_string(),
            provider_type: kind.provider_type(),
            client_id: PROVIDER_CLIENT_ID.to_string(),
            client_secret_ciphertext: None,
            encryption_key_id: "default".into(),
            authorization_url: Some(explicit(AUTHORIZE_PATH)),
            token_url: Some(explicit(TOKEN_PATH)),
            userinfo_url: Some(explicit(kind.userinfo_path())),
            discovery_url: kind.discovery_path().map(explicit),
            scopes: kind.scopes(),
            is_active: true,
            mapping_config: kind.mapping(),
            created_at: 0,
            updated_at: 0,
        }
    }

    fn requests(&self) -> Vec<Recorded> {
        self.state.requests.lock().unwrap().clone()
    }

    fn request(&self, path: &str) -> Recorded {
        self.requests()
            .into_iter()
            .find(|r| r.path == path)
            .unwrap_or_else(|| panic!("the simulator saw no request for {path}"))
    }

    fn request_count(&self, path: &str) -> usize {
        self.requests().iter().filter(|r| r.path == path).count()
    }

    fn token_response(&self) -> Value {
        self.state
            .last_token_response
            .lock()
            .unwrap()
            .clone()
            .expect("the simulator sent a token response")
    }

    fn nonce(&self) -> String {
        self.state
            .flow
            .lock()
            .unwrap()
            .nonce
            .clone()
            .expect("the authorize request carried a nonce")
    }

    fn set_bad_signature(&self) {
        self.state.flow.lock().unwrap().bad_signature = true;
    }

    fn set_wrong_nonce(&self) {
        self.state.flow.lock().unwrap().wrong_nonce = true;
    }

    fn set_wrong_audience(&self) {
        self.state.flow.lock().unwrap().wrong_audience = true;
    }

    fn set_wrong_issuer(&self) {
        self.state.flow.lock().unwrap().wrong_issuer = true;
    }

    fn set_omit_id_token(&self) {
        self.state.flow.lock().unwrap().omit_id_token = true;
    }
}

// ── IdP test harness ────────────────────────────────────────────────────

struct IdpHarness {
    origin: String,
    storage: Arc<HandlerStorage<MemoryStorage>>,
    /// The service that encrypts the provider secret. The server gets a clone
    /// only when the test asks for the secret path.
    provider_service: Arc<ProviderService<dyn QueryStore>>,
    ctx: Ctx,
    shutdown: Arc<ShutdownSignal>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for IdpHarness {
    fn drop(&mut self) {
        self.shutdown.turn_on();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl IdpHarness {
    /// Start an `IdP` with the routes under `prefix`. The issuer is the live
    /// origin, so the upstream callback URL points at this server.
    fn start(ctx: Ctx, prefix: &str) -> Self {
        Self::start_with(ctx, prefix, false, true)
    }

    /// Start an `IdP` through [`IdpServer::http_app_with_prefix`].
    fn start_with_app_builder(ctx: Ctx, prefix: &str) -> Self {
        Self::start_with(ctx, prefix, true, true)
    }

    /// Start an `IdP` with no provider service. A provider that has a secret
    /// must fail, not fall back to a public-client exchange.
    fn start_without_provider_service(ctx: Ctx, prefix: &str) -> Self {
        Self::start_with(ctx, prefix, false, false)
    }

    fn start_with(
        ctx: Ctx,
        prefix: &str,
        use_app_builder: bool,
        with_provider_service: bool,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind IdP");
        let addr = listener.local_addr().expect("IdP address");
        let origin = format!("http://{addr}");

        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("social.db");
        let provider_store = Arc::new(super::turso_provider(db_path.to_str().unwrap().to_string()));
        std::mem::forget(dir);
        let query_store: Arc<dyn QueryStore> = provider_store.clone();
        let provider_service: Arc<ProviderService<dyn QueryStore>> =
            Arc::new(ProviderService::new(
                Arc::clone(&query_store),
                ProviderCrypto::from_passphrase("test-key", "default"),
                ctx.clone(),
            ));
        let storage = Arc::new(HandlerStorage::new(
            query_store,
            MemoryStorage::new(),
            ctx.clone(),
        ));

        let config = IdpConfig::new(origin.clone());
        let mut idp = IdpServer::new(config, Arc::clone(&storage), ctx.clone())
            .with_social_login(Arc::new(MemoryAuthSessionStore::new()));
        if with_provider_service {
            idp = idp.with_provider_service(Arc::clone(&provider_service));
        }
        let shutdown = Arc::new(ShutdownSignal::new());
        let app = if use_app_builder {
            let idp = idp.with_route_prefix(prefix);
            idp.http_app_with_prefix(prefix)
        } else {
            idp.with_route_prefix(prefix).http_app()
        };
        let server = HttpServer::with_config(app, &addr.to_string(), ServerConfig::defaults());
        let shutdown_thread = Arc::clone(&shutdown);
        let handle = std::thread::spawn(move || {
            server.serve_with_listener(&listener, &shutdown_thread);
        });
        Self {
            origin,
            storage,
            provider_service,
            ctx,
            shutdown,
            handle: Some(handle),
        }
    }

    /// Store a provider and give it the client secret the simulator expects.
    fn seed_provider(&self, provider: UpstreamProvider) {
        let id = provider.id.clone();
        let service = Arc::clone(&self.provider_service);
        async move {
            service
                .create(provider)
                .await
                .expect("store upstream provider");
            service
                .set_secret(&id, PROVIDER_CLIENT_SECRET)
                .await
                .expect("set the provider client secret");
        }
        .block();
    }

    fn seed_client(&self) {
        let client = OAuthClient {
            id: APP_CLIENT_ID.into(),
            name: "Test App".into(),
            client_secret_hash: hex_sha256(APP_CLIENT_SECRET),
            redirect_uris: vec![APP_REDIRECT_URI.into()],
            grant_types: vec!["authorization_code".into(), "refresh_token".into()],
            scopes: vec!["openid".into(), "profile".into(), "email".into()],
            is_public: false,
            created_at: 0,
        };
        let store = Arc::clone(&self.storage.query_store);
        async move { storage::store_oauth_client(store.as_ref(), &client).await }
            .block()
            .expect("store app client");
    }

    fn seed_user(&self, user: User) {
        let store = Arc::clone(&self.storage.query_store);
        let ctx = self.ctx.clone();
        async move { storage::create_user(store.as_ref(), &user, &ctx).await }
            .block()
            .expect("store user");
    }

    fn find_user_by_email(&self, email: &str) -> Option<User> {
        let store = Arc::clone(&self.storage.query_store);
        let email = email.to_string();
        async move { storage::find_user_by_email(store.as_ref(), &email).await }
            .block()
            .expect("find user")
    }

    fn url(&self, path_and_query: &str) -> String {
        format!("{}{path_and_query}", self.origin)
    }

    fn authorize_url(&self, kind: Kind, state: &str) -> String {
        self.url(&format!(
            "/idp/authorize?provider={}&client_id={}&redirect_uri={}&state={}",
            kind.id(),
            enc(APP_CLIENT_ID),
            enc(APP_REDIRECT_URI),
            enc(state),
        ))
    }

    fn exchange_code(&self, code: &str) -> Reply {
        raw_request(
            &self.url("/idp/token"),
            "POST",
            Some(&encode_form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", APP_REDIRECT_URI),
                ("client_id", APP_CLIENT_ID),
                ("client_secret", APP_CLIENT_SECRET),
            ])),
            &[],
        )
    }
}

// ── Raw HTTP ────────────────────────────────────────────────────────────

/// What the server answered, from a raw HTTP/1.1 exchange.
struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
}

impl Reply {
    fn location(&self) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("location"))
            .map(|(_, v)| v.as_str())
    }

    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or(Value::Null)
    }
}

/// Send one HTTP/1.1 request over TCP and read the whole answer. The client
/// follows no redirect, so every hop of the chain is visible.
fn raw_request(url: &str, method: &str, body: Option<&str>, extra: &[(&str, &str)]) -> Reply {
    let rest = url.strip_prefix("http://").expect("an http URL");
    let (authority, path) = match rest.split_once('/') {
        Some((authority, path)) => (authority, format!("/{path}")),
        None => (rest, String::from("/")),
    };
    let mut stream = TcpStream::connect(authority).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .expect("read timeout");
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .expect("write timeout");
    let mut head =
        format!("{method} {path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n");
    if let Some(body) = body {
        write!(
            head,
            "Content-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n",
            body.len()
        )
        .expect("write head");
    }
    for (name, value) in extra {
        write!(head, "{name}: {value}\r\n").expect("write head");
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes()).expect("write head");
    if let Some(body) = body {
        stream.write_all(body.as_bytes()).expect("write body");
    }
    let mut text = String::new();
    stream.read_to_string(&mut text).expect("read response");
    let (head, body) = text.split_once("\r\n\r\n").unwrap_or((text.as_str(), ""));
    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .expect("status line");
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    Reply {
        status,
        headers,
        body: body.to_string(),
    }
}

// ── Response writing ────────────────────────────────────────────────────

#[allow(
    clippy::needless_pass_by_value,
    reason = "callers pass a JSON value; the serializer reads it by reference"
)]
fn respond(
    mut conn: SharedByteBufferStream<RawStream>,
    status: u16,
    headers: Vec<(String, String)>,
    body: Value,
) -> ConnectionResult {
    let bytes = serde_json::to_vec(&body).expect("serialize response");
    write_response(&mut conn, status, headers, bytes)
}

fn respond_redirect(
    mut conn: SharedByteBufferStream<RawStream>,
    location: &str,
) -> ConnectionResult {
    write_response(
        &mut conn,
        302,
        vec![("Location".into(), location.to_string())],
        Vec::new(),
    )
}

fn write_response(
    conn: &mut SharedByteBufferStream<RawStream>,
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
) -> ConnectionResult {
    let mut builder = SimpleOutgoingResponse::builder()
        .with_status(status_for(status))
        .add_header(SimpleHeader::CONTENT_TYPE, "application/json");
    for (name, value) in headers {
        builder = builder.add_header(name, value);
    }
    let Ok(response) = builder.with_body(SendSafeBody::Bytes(body)).build() else {
        return ConnectionResult::Close(None);
    };
    match Http11::response(response).http_render_to_writer(conn) {
        Ok(_) => ConnectionResult::Keep,
        Err(_) => ConnectionResult::Close(None),
    }
}

fn status_for(code: u16) -> Status {
    match code {
        200 => Status::OK,
        302 => Status::Found,
        400 => Status::BadRequest,
        401 => Status::Unauthorized,
        404 => Status::NotFound,
        500 => Status::InternalServerError,
        other => Status::Numbered(usize::from(other), String::new()),
    }
}

// ── Small helpers ───────────────────────────────────────────────────────

fn read_body(body: SendSafeBody) -> String {
    match body {
        SendSafeBody::Text(text) => text,
        SendSafeBody::Bytes(bytes) => String::from_utf8_lossy(&bytes).to_string(),
        other => {
            let bytes = try_collect_bytes(other).expect("read request body");
            String::from_utf8_lossy(&bytes).to_string()
        }
    }
}

fn parse_pairs(input: &str) -> Vec<(String, String)> {
    input
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (decode(key), decode(value))
        })
        .collect()
}

fn pair<'a>(pairs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    pairs
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

fn decode(value: &str) -> String {
    urlencoding::decode(value)
        .unwrap_or_else(|_| value.into())
        .into_owned()
}

fn enc(value: &str) -> String {
    urlencoding::encode(value).into_owned()
}

fn encode_form(fields: &[(&str, &str)]) -> String {
    fields
        .iter()
        .map(|(k, v)| format!("{}={}", enc(k), enc(v)))
        .collect::<Vec<_>>()
        .join("&")
}

fn s256(verifier: &str) -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    use sha2::{Digest, Sha256};
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn hex_sha256(value: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn now_seconds(ctx: &Ctx) -> i64 {
    foundation_compact::time::unix_seconds(ctx.clock().wall_now())
}

fn github_emails() -> Value {
    json!([
        { "email": "old@github.example", "primary": false, "verified": true },
        { "email": "octocat@github.example", "primary": true, "verified": true },
        { "email": "unproven@github.example", "primary": false, "verified": false }
    ])
}

fn test_ctx() -> Ctx {
    Ctx::fixed(Arc::new(MonotonicClock::new()))
}

/// Store the provider and the app client on a fresh harness. The provider row
/// uses the simulator's endpoints.
fn seed_flow(idp: &IdpHarness, sim: &Simulator) {
    idp.seed_client();
    idp.seed_provider(sim.provider_row());
}

/// Walk the redirect chain from the `IdP` authorize call to the app redirect.
/// Returns the app redirect URL the `IdP` sent.
fn walk(idp: &IdpHarness, sim: &Simulator, kind: Kind) -> String {
    let authorize = raw_request(&idp.authorize_url(kind, "app-state-1"), "GET", None, &[]);
    assert_eq!(authorize.status, 302, "authorize did not redirect");
    let upstream = authorize
        .location()
        .expect("authorize Location")
        .to_string();
    assert!(
        upstream.starts_with(&sim.origin),
        "the authorize redirect must go to the provider: {upstream}"
    );

    let provider = raw_request(&upstream, "GET", None, &[]);
    assert_eq!(provider.status, 302, "provider did not redirect");
    let callback = provider.location().expect("provider Location").to_string();
    assert!(
        callback.starts_with(&format!("{}/idp/callback", idp.origin)),
        "the provider must redirect to the IdP callback: {callback}"
    );

    let answer = raw_request(&callback, "GET", None, &[]);
    assert_eq!(answer.status, 302, "callback did not redirect");
    answer.location().expect("callback Location").to_string()
}

fn app_code(redirect: &str) -> (String, String) {
    let query = redirect.split_once('?').map_or("", |(_, q)| q);
    let pairs = parse_pairs(query);
    (
        pair(&pairs, "code").unwrap_or_default().to_string(),
        pair(&pairs, "state").unwrap_or_default().to_string(),
    )
}

// ── Provider tests ──────────────────────────────────────────────────────

#[valtron_test(threads = 4, timeout = 60000)]
fn google_oidc_provider_login_end_to_end() {
    let ctx = test_ctx();
    let sim = Simulator::start(ctx.clone(), Kind::Google);
    let idp = IdpHarness::start(ctx.clone(), "/idp");
    seed_flow(&idp, &sim);

    // A verified Google identity links to the account with the same email.
    idp.seed_user(User {
        id: "existing-google-user".into(),
        email: "alice@example.com".into(),
        username: Some("alice-existing".into()),
        password_hash: None,
        email_verified: true,
        email_verified_at: None,
        created_at: 0,
        updated_at: 0,
        metadata: None,
        failed_login_attempts: 0,
        locked_until: None,
        deleted_at: None,
    });

    let redirect = walk(&idp, &sim, Kind::Google);
    let (code, state) = app_code(&redirect);
    assert_eq!(state, "app-state-1");
    assert!(redirect.starts_with(APP_REDIRECT_URI));

    let tokens = idp.exchange_code(&code);
    assert_eq!(tokens.status, 200, "token exchange: {}", tokens.body);
    assert_ne!(tokens.json()["access_token"].as_str().unwrap_or(""), "");

    // The verified email linked the login to the existing account.
    let user = idp
        .find_user_by_email("alice@example.com")
        .expect("the linked user exists");
    assert_eq!(user.id, "existing-google-user");

    // The discovery document was read, and the authorize URL carries PKCE and
    // the nonce.
    assert!(sim.request_count(DISCOVERY_PATH) >= 1);
    let authorize = sim.request(AUTHORIZE_PATH);
    assert_eq!(authorize.method, "GET");
    assert_eq!(
        authorize.query_param("redirect_uri"),
        Some(format!("{}/idp/callback?provider=google", idp.origin).as_str())
    );
    assert_eq!(authorize.query_param("code_challenge_method"), Some("S256"));
    assert!(authorize.query_param("code_challenge").is_some());
    let nonce = sim.nonce();
    assert_eq!(authorize.query_param("nonce"), Some(nonce.as_str()));

    // The token form carries the same redirect URI and the PKCE verifier.
    let token = sim.request(TOKEN_PATH);
    assert_eq!(token.method, "POST");
    assert_eq!(
        token.form_param("redirect_uri"),
        Some(format!("{}/idp/callback?provider=google", idp.origin).as_str())
    );
    assert!(token.form_param("code_verifier").is_some());
    assert_eq!(
        token.form_param("client_secret"),
        Some(PROVIDER_CLIENT_SECRET),
        "the confidential client proves itself at the token endpoint"
    );

    // The JWKS and userinfo were fetched; userinfo carried the Bearer token.
    assert_eq!(sim.request_count(JWKS_PATH), 1);
    let userinfo = sim.request(USERINFO_PATH);
    assert_eq!(userinfo.method, "GET");
    assert_eq!(
        userinfo.header("authorization"),
        Some("Bearer sim-access-google")
    );
}

#[valtron_test(threads = 4, timeout = 60000)]
fn github_oauth2_provider_login_uses_email_list() {
    let ctx = test_ctx();
    let sim = Simulator::start(ctx.clone(), Kind::GitHub);
    let idp = IdpHarness::start(ctx.clone(), "/idp");
    seed_flow(&idp, &sim);

    let redirect = walk(&idp, &sim, Kind::GitHub);
    let (code, _) = app_code(&redirect);
    let tokens = idp.exchange_code(&code);
    assert_eq!(tokens.status, 200, "token exchange: {}", tokens.body);

    // The profile has `email: null`; the address came from `/user/emails`.
    let user = idp
        .find_user_by_email("octocat@github.example")
        .expect("the GitHub email is the user email");
    assert_eq!(user.username.as_deref(), Some("The Octocat"));

    let userinfo = sim.request(GITHUB_USER_PATH);
    assert_eq!(
        userinfo.header("authorization"),
        Some("Bearer sim-access-github")
    );
    let emails = sim.request(GITHUB_EMAILS_PATH);
    assert_eq!(
        emails.header("authorization"),
        Some("Bearer sim-access-github")
    );
    // No discovery document for GitHub.
    assert_eq!(sim.request_count(DISCOVERY_PATH), 0);
}

#[valtron_test(threads = 4, timeout = 60000)]
fn facebook_oauth2_provider_login_end_to_end() {
    let ctx = test_ctx();
    let sim = Simulator::start(ctx.clone(), Kind::Facebook);
    let idp = IdpHarness::start(ctx.clone(), "/idp");
    seed_flow(&idp, &sim);

    let redirect = walk(&idp, &sim, Kind::Facebook);
    let (code, _) = app_code(&redirect);
    let tokens = idp.exchange_code(&code);
    assert_eq!(tokens.status, 200, "token exchange: {}", tokens.body);

    let user = idp
        .find_user_by_email("fiona@example.com")
        .expect("the Facebook user exists");
    // The Graph `name` maps to the display name (the username column).
    assert_eq!(user.username.as_deref(), Some("Fiona Book"));

    let authorize = sim.request(AUTHORIZE_PATH);
    assert_eq!(authorize.query_param("code_challenge_method"), Some("S256"));
    let userinfo = sim.request(USERINFO_PATH);
    assert_eq!(
        userinfo.header("authorization"),
        Some("Bearer sim-access-facebook")
    );
}

#[valtron_test(threads = 4, timeout = 60000)]
fn twitter_oauth2_pkce_provider_login_end_to_end() {
    let ctx = test_ctx();
    let sim = Simulator::start(ctx.clone(), Kind::Twitter);
    let idp = IdpHarness::start(ctx.clone(), "/idp");
    seed_flow(&idp, &sim);

    let redirect = walk(&idp, &sim, Kind::Twitter);
    let (code, _) = app_code(&redirect);
    let tokens = idp.exchange_code(&code);
    assert_eq!(tokens.status, 200, "token exchange: {}", tokens.body);

    // `data.name` reaches the display name through the nested mapping.
    let user = idp.find_user_by_email("").expect("the Twitter user exists");
    assert_eq!(user.username.as_deref(), Some("Tina Tweet"));

    // PKCE is in the authorize URL and the token form.
    let authorize = sim.request(AUTHORIZE_PATH);
    assert!(authorize.query_param("code_challenge").is_some());
    assert_eq!(authorize.query_param("code_challenge_method"), Some("S256"));
    let token = sim.request(TOKEN_PATH);
    assert!(token.form_param("code_verifier").is_some());

    // `expires_in` and `refresh_token` stay on the token object.
    let response = sim.token_response();
    assert_eq!(response["expires_in"], json!(7_200));
    assert_eq!(response["refresh_token"], json!("sim-refresh-twitter"));
}

// ── Refusal tests ───────────────────────────────────────────────────────

#[valtron_test(threads = 4, timeout = 60000)]
fn used_state_is_refused() {
    let ctx = test_ctx();
    let sim = Simulator::start(ctx.clone(), Kind::Google);
    let idp = IdpHarness::start(ctx.clone(), "/idp");
    seed_flow(&idp, &sim);

    // Walk to the callback URL, then call it twice with the same state.
    let authorize = raw_request(
        &idp.authorize_url(Kind::Google, "app-state-1"),
        "GET",
        None,
        &[],
    );
    let upstream = authorize.location().expect("Location").to_string();
    let provider = raw_request(&upstream, "GET", None, &[]);
    let callback = provider.location().expect("Location").to_string();

    let first = raw_request(&callback, "GET", None, &[]);
    assert_eq!(first.status, 302, "the first callback: {}", first.body);
    let second = raw_request(&callback, "GET", None, &[]);
    assert_eq!(second.status, 400, "a used state must be refused");
    assert_eq!(second.json()["error"], "bad_request");
}

#[valtron_test(threads = 4, timeout = 60000)]
fn wrong_nonce_is_refused() {
    let ctx = test_ctx();
    let sim = Simulator::start(ctx.clone(), Kind::Google);
    let idp = IdpHarness::start(ctx.clone(), "/idp");
    seed_flow(&idp, &sim);

    let authorize = raw_request(
        &idp.authorize_url(Kind::Google, "app-state-1"),
        "GET",
        None,
        &[],
    );
    let upstream = authorize.location().expect("Location").to_string();
    let provider = raw_request(&upstream, "GET", None, &[]);
    let callback = provider.location().expect("Location").to_string();
    let nonce = sim.nonce();

    sim.set_wrong_nonce();
    let answer = raw_request(&callback, "GET", None, &[]);
    assert_eq!(answer.status, 400, "a wrong nonce must be refused");
    assert!(!answer.body.contains(&nonce), "the answer names the nonce");
}

#[valtron_test(threads = 4, timeout = 60000)]
fn bad_id_token_signature_is_refused() {
    let ctx = test_ctx();
    let sim = Simulator::start(ctx.clone(), Kind::Google);
    let idp = IdpHarness::start(ctx.clone(), "/idp");
    seed_flow(&idp, &sim);

    let authorize = raw_request(
        &idp.authorize_url(Kind::Google, "app-state-1"),
        "GET",
        None,
        &[],
    );
    let upstream = authorize.location().expect("Location").to_string();
    let provider = raw_request(&upstream, "GET", None, &[]);
    let callback = provider.location().expect("Location").to_string();

    sim.set_bad_signature();
    let answer = raw_request(&callback, "GET", None, &[]);
    assert_eq!(answer.status, 400, "a bad signature must be refused");
    assert_eq!(answer.json()["error"], "bad_request");
}

#[valtron_test(threads = 4, timeout = 60000)]
fn wrong_audience_is_refused() {
    let ctx = test_ctx();
    let sim = Simulator::start(ctx.clone(), Kind::Google);
    let idp = IdpHarness::start(ctx.clone(), "/idp");
    seed_flow(&idp, &sim);

    let authorize = raw_request(
        &idp.authorize_url(Kind::Google, "app-state-1"),
        "GET",
        None,
        &[],
    );
    let upstream = authorize.location().expect("Location").to_string();
    let provider = raw_request(&upstream, "GET", None, &[]);
    let callback = provider.location().expect("Location").to_string();

    sim.set_wrong_audience();
    let answer = raw_request(&callback, "GET", None, &[]);
    assert_eq!(answer.status, 400, "a wrong audience must be refused");
}

#[valtron_test(threads = 4, timeout = 60000)]
fn wrong_issuer_is_refused() {
    let ctx = test_ctx();
    let sim = Simulator::start(ctx.clone(), Kind::Google);
    let idp = IdpHarness::start(ctx.clone(), "/idp");
    seed_flow(&idp, &sim);

    let authorize = raw_request(
        &idp.authorize_url(Kind::Google, "app-state-1"),
        "GET",
        None,
        &[],
    );
    let upstream = authorize.location().expect("Location").to_string();
    let provider = raw_request(&upstream, "GET", None, &[]);
    let callback = provider.location().expect("Location").to_string();

    sim.set_wrong_issuer();
    let answer = raw_request(&callback, "GET", None, &[]);
    assert_eq!(answer.status, 400, "a wrong issuer must be refused");
}

#[valtron_test(threads = 4, timeout = 60000)]
fn missing_id_token_is_refused() {
    let ctx = test_ctx();
    let sim = Simulator::start(ctx.clone(), Kind::Google);
    let idp = IdpHarness::start(ctx.clone(), "/idp");
    seed_flow(&idp, &sim);

    let authorize = raw_request(
        &idp.authorize_url(Kind::Google, "app-state-1"),
        "GET",
        None,
        &[],
    );
    let upstream = authorize.location().expect("Location").to_string();
    let provider = raw_request(&upstream, "GET", None, &[]);
    let callback = provider.location().expect("Location").to_string();

    sim.set_omit_id_token();
    let answer = raw_request(&callback, "GET", None, &[]);
    assert_eq!(answer.status, 400, "a missing id_token must be refused");
}

#[valtron_test(threads = 4, timeout = 60000)]
fn unknown_provider_is_refused() {
    let ctx = test_ctx();
    let idp = IdpHarness::start(ctx.clone(), "/idp");
    idp.seed_client();

    let url = idp.url(&format!(
        "/idp/authorize?provider=nope&client_id={}&redirect_uri={}&state=s",
        enc(APP_CLIENT_ID),
        enc(APP_REDIRECT_URI),
    ));
    let answer = raw_request(&url, "GET", None, &[]);
    assert_eq!(answer.status, 404, "an unknown provider must be refused");
}

#[valtron_test(threads = 4, timeout = 60000)]
fn inactive_provider_is_refused() {
    let ctx = test_ctx();
    let sim = Simulator::start(ctx.clone(), Kind::GitHub);
    let idp = IdpHarness::start(ctx.clone(), "/idp");
    idp.seed_client();
    let mut provider = sim.provider_row();
    provider.is_active = false;
    idp.seed_provider(provider);

    let url = idp.authorize_url(Kind::GitHub, "app-state-1");
    let answer = raw_request(&url, "GET", None, &[]);
    assert_eq!(answer.status, 403, "an inactive provider must be refused");
}

#[valtron_test(threads = 4, timeout = 60000)]
fn unregistered_app_redirect_uri_is_refused() {
    let ctx = test_ctx();
    let idp = IdpHarness::start(ctx.clone(), "/idp");
    idp.seed_client();

    let url = idp.url(&format!(
        "/idp/authorize?provider=google&client_id={}&redirect_uri={}&state=s",
        enc(APP_CLIENT_ID),
        enc("https://evil.example/callback"),
    ));
    let answer = raw_request(&url, "GET", None, &[]);
    assert_eq!(
        answer.status, 400,
        "an unregistered redirect_uri is refused"
    );
}

// ── Prefix test ─────────────────────────────────────────────────────────

#[valtron_test(threads = 4, timeout = 60000)]
fn custom_route_prefix_is_used_in_the_upstream_callback_url() {
    let ctx = test_ctx();
    let sim = Simulator::start(ctx.clone(), Kind::Google);
    let idp = IdpHarness::start_with_app_builder(ctx.clone(), "/custom-idp");
    seed_flow(&idp, &sim);

    let url = idp.url(&format!(
        "/custom-idp/authorize?provider=google&client_id={}&redirect_uri={}&state=s",
        enc(APP_CLIENT_ID),
        enc(APP_REDIRECT_URI),
    ));
    let authorize = raw_request(&url, "GET", None, &[]);
    assert_eq!(authorize.status, 302, "authorize: {}", authorize.body);

    let upstream = authorize.location().expect("Location").to_string();
    let provider = raw_request(&upstream, "GET", None, &[]);
    let callback = provider.location().expect("Location").to_string();
    assert!(
        callback.starts_with(&format!("{}/custom-idp/callback", idp.origin)),
        "the callback uses the custom prefix: {callback}"
    );

    let answer = raw_request(&callback, "GET", None, &[]);
    assert_eq!(answer.status, 302, "callback: {}", answer.body);
    assert!(answer
        .location()
        .unwrap_or("")
        .starts_with(APP_REDIRECT_URI));
}

// ── Client-secret tests ─────────────────────────────────────────────────

#[valtron_test(threads = 4, timeout = 60000)]
fn a_wrong_client_secret_is_refused_at_the_token_endpoint() {
    let ctx = test_ctx();
    let sim = Simulator::start(ctx.clone(), Kind::GitHub);
    let idp = IdpHarness::start(ctx.clone(), "/idp");
    seed_flow(&idp, &sim);

    // Replace the stored secret with a wrong one. The simulator rejects it.
    let service = Arc::clone(&idp.provider_service);
    async move {
        service
            .set_secret("github", "not-the-secret")
            .await
            .expect("replace the secret");
    }
    .block();

    let authorize = raw_request(
        &idp.authorize_url(Kind::GitHub, "app-state-1"),
        "GET",
        None,
        &[],
    );
    let upstream = authorize.location().expect("Location").to_string();
    let provider = raw_request(&upstream, "GET", None, &[]);
    let callback = provider.location().expect("Location").to_string();

    let answer = raw_request(&callback, "GET", None, &[]);
    assert_eq!(answer.status, 400, "the exchange must fail");
    assert_eq!(
        sim.request(TOKEN_PATH).form_param("client_secret"),
        Some("not-the-secret")
    );
}

#[valtron_test(threads = 4, timeout = 60000)]
fn a_provider_secret_without_a_provider_service_is_an_error() {
    let ctx = test_ctx();
    let sim = Simulator::start(ctx.clone(), Kind::Facebook);
    let idp = IdpHarness::start_without_provider_service(ctx.clone(), "/idp");
    seed_flow(&idp, &sim);

    let authorize = raw_request(
        &idp.authorize_url(Kind::Facebook, "app-state-1"),
        "GET",
        None,
        &[],
    );
    let upstream = authorize.location().expect("Location").to_string();
    let provider = raw_request(&upstream, "GET", None, &[]);
    let callback = provider.location().expect("Location").to_string();

    let answer = raw_request(&callback, "GET", None, &[]);
    assert_eq!(
        answer.status, 500,
        "a missing ProviderService must fail, not fall back"
    );
    assert_eq!(
        sim.request_count(TOKEN_PATH),
        0,
        "no public-client exchange is attempted"
    );
}
