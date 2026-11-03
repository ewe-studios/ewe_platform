use std::sync::Arc;

use foundation_http::shared::app::HttpApp;
use foundation_http::shared::serve::Serve;
use foundation_http::SimpleMethod;

use super::config::IdpConfig;
use super::handlers::ServeAdapter;
use super::services::SessionService;
use super::storage::HandlerStorage;
use foundation_db::{KeyValueStore, MemoryStorage};

/// The `IdP` server: routes plus the config and storage its handlers share.
///
/// `KV` is the cache for short-lived handshake state (`PoW`, WebAuthn); the
/// default is the in-memory store.
pub struct IdpServer<KV: KeyValueStore + Clone + 'static = MemoryStorage> {
    config: IdpConfig,
    storage: Arc<HandlerStorage<KV>>,
    sessions: Arc<SessionService>,
}

impl<KV: KeyValueStore + Clone + 'static> IdpServer<KV> {
    /// Build the server over a config and storage.
    ///
    /// # Panics
    ///
    /// Never in practice: it panics only if building the default in-memory
    /// session service fails, and a 32-byte random key is always valid.
    #[must_use]
    pub fn new(config: IdpConfig, storage: Arc<HandlerStorage<KV>>) -> Self {
        let sessions = Arc::new(
            SessionService::in_memory().expect("a 32-byte signing key is a valid session key"),
        );
        Self {
            config,
            storage,
            sessions,
        }
    }

    /// Use this session service for all routes. The default is an in-memory
    /// service with a random signing key: sessions end when the process stops.
    #[must_use]
    pub fn with_session_service(mut self, sessions: Arc<SessionService>) -> Self {
        self.sessions = sessions;
        self
    }

    pub fn register_routes(app: &mut HttpApp<Arc<dyn Serve>>, prefix: &str) {
        let p = prefix.trim_end_matches('/');
        app.route::<ServeAdapter<KV>>(
            SimpleMethod::GET,
            &format!("{p}/.well-known/openid-configuration"),
        );
        app.route::<ServeAdapter<KV>>(SimpleMethod::GET, &format!("{p}/.well-known/jwks.json"));
        app.route::<ServeAdapter<KV>>(SimpleMethod::GET, &format!("{p}/authorize"));
        app.route::<ServeAdapter<KV>>(SimpleMethod::POST, &format!("{p}/token"));
        app.route::<ServeAdapter<KV>>(SimpleMethod::GET, &format!("{p}/userinfo"));
        app.route::<ServeAdapter<KV>>(SimpleMethod::POST, &format!("{p}/introspect"));
        app.route::<ServeAdapter<KV>>(SimpleMethod::POST, &format!("{p}/device/authorize"));
        // F011 — social login callback (authorize is already registered above)
        app.route::<ServeAdapter<KV>>(SimpleMethod::GET, &format!("{p}/callback"));
        // F02 — auth endpoints (unified login, MFA, logout)
        app.route::<ServeAdapter<KV>>(SimpleMethod::POST, "/auth/v1/oidc/authorize");
        app.route::<ServeAdapter<KV>>(SimpleMethod::POST, "/auth/v1/mfa");
        app.route::<ServeAdapter<KV>>(SimpleMethod::POST, "/auth/v1/oidc/logout");
        // F03 — registration endpoints
        app.route::<ServeAdapter<KV>>(SimpleMethod::POST, "/auth/v1/users/register");
        // F04 — password reset endpoints
        app.route::<ServeAdapter<KV>>(SimpleMethod::POST, "/auth/v1/users/request_reset");
        app.route::<ServeAdapter<KV>>(SimpleMethod::PUT, "/auth/v1/users/{id}/reset");
        // F05 — Proof of Work endpoints
        app.route::<ServeAdapter<KV>>(SimpleMethod::GET, "/auth/v1/pow");
        app.route::<ServeAdapter<KV>>(SimpleMethod::POST, "/auth/v1/pow");
        // F08 — template/config API endpoints
        app.route::<ServeAdapter<KV>>(SimpleMethod::GET, "/auth/v1/templates/config");
        app.route::<ServeAdapter<KV>>(SimpleMethod::GET, "/auth/v1/templates/password_policy");
        // F09 — account management endpoints
        app.route::<ServeAdapter<KV>>(SimpleMethod::GET, "/auth/v1/users/{id}");
        app.route::<ServeAdapter<KV>>(SimpleMethod::PUT, "/auth/v1/users/{id}");
        app.route::<ServeAdapter<KV>>(SimpleMethod::POST, "/auth/v1/users/{id}/change_password");
        app.route::<ServeAdapter<KV>>(SimpleMethod::GET, "/auth/v1/users/{id}/sessions");
        app.route::<ServeAdapter<KV>>(SimpleMethod::POST, "/auth/v1/users/{id}/revoke");
        // F06 — WebAuthn/FIDO2 endpoints
        app.route::<ServeAdapter<KV>>(SimpleMethod::POST, "/auth/v1/webauthn/register/start");
        app.route::<ServeAdapter<KV>>(SimpleMethod::POST, "/auth/v1/webauthn/register/finish");
        app.route::<ServeAdapter<KV>>(SimpleMethod::POST, "/auth/v1/webauthn/login/start");
        app.route::<ServeAdapter<KV>>(SimpleMethod::POST, "/auth/v1/webauthn/login/finish");
        app.route::<ServeAdapter<KV>>(SimpleMethod::DELETE, "/auth/v1/webauthn/{id}");
        app.route::<ServeAdapter<KV>>(SimpleMethod::PUT, "/auth/v1/webauthn/{id}");
        // F06 — passkey-only login endpoints
        app.route::<ServeAdapter<KV>>(SimpleMethod::POST, "/auth/v1/passkey/login/start");
        app.route::<ServeAdapter<KV>>(SimpleMethod::POST, "/auth/v1/passkey/login/finish");
        // F07 — Terms of Service endpoints
        app.route::<ServeAdapter<KV>>(SimpleMethod::GET, "/auth/v1/tos/latest");
        app.route::<ServeAdapter<KV>>(SimpleMethod::POST, "/auth/v1/tos/accept");
    }

    /// Register the routes that exist only in development mode
    /// ([`IdpConfig::dev_mode`]). `http_app` calls this when the flag is on.
    pub fn register_dev_routes(app: &mut HttpApp<Arc<dyn Serve>>) {
        app.route::<ServeAdapter<KV>>(SimpleMethod::POST, "/auth/v1/dev/register");
    }

    #[must_use]
    pub fn http_app(self) -> HttpApp<Arc<dyn Serve>> {
        let mut app = HttpApp::new_serve();
        let dev_mode = self.config.dev_mode;
        app.ctx.store(self.config);
        // Stored as `Arc<HandlerStorage<KV>>`; `ServeAdapter::create` looks up
        // exactly that type.
        app.ctx.store(self.storage);
        // Stored as `Arc<SessionService>`; `ServeAdapter::create` looks up
        // exactly that type.
        app.ctx.store(self.sessions);
        Self::register_routes(&mut app, "/idp");
        if dev_mode {
            eprintln!(
                "WARNING: IdP dev_mode is on: POST /auth/v1/dev/register is served. \
                 Do not use dev_mode in production."
            );
            Self::register_dev_routes(&mut app);
        }
        app
    }

    #[must_use]
    pub fn server(self, addr: &str) -> foundation_http::native::server::HttpServer {
        self.http_app().server(addr)
    }

    pub fn server_with_config(
        self,
        addr: &str,
        config: foundation_http::native::server::ServerConfig,
    ) -> foundation_http::native::server::HttpServer {
        self.http_app().server_with_config(addr, config)
    }
}
