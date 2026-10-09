use std::sync::Arc;

use foundation_http::shared::app::HttpApp;
use foundation_http::shared::serve::Serve;
use foundation_http::SimpleMethod;

use super::config::IdpConfig;
use super::handlers::ServeAdapter;
use super::storage::HandlerStorage;
use foundation_db::{KeyValueStore, MemoryStorage};

pub struct IdpServer<KV: KeyValueStore + 'static = MemoryStorage> {
    config: IdpConfig,
    storage: Arc<HandlerStorage<KV>>,
}

impl<KV: KeyValueStore + 'static> IdpServer<KV> {
    #[must_use]
    pub fn new(config: IdpConfig, storage: Arc<HandlerStorage<KV>>) -> Self {
        Self { config, storage }
    }

    pub fn register_routes(app: &mut HttpApp<Arc<dyn Serve>>, prefix: &str) {
        let p = prefix.trim_end_matches('/');
        app.route::<ServeAdapter>(SimpleMethod::GET, &format!("{p}/.well-known/openid-configuration"));
        app.route::<ServeAdapter>(SimpleMethod::GET, &format!("{p}/.well-known/jwks.json"));
        app.route::<ServeAdapter>(SimpleMethod::GET, &format!("{p}/authorize"));
        app.route::<ServeAdapter>(SimpleMethod::POST, &format!("{p}/token"));
        app.route::<ServeAdapter>(SimpleMethod::GET, &format!("{p}/userinfo"));
        app.route::<ServeAdapter>(SimpleMethod::POST, &format!("{p}/introspect"));
        app.route::<ServeAdapter>(SimpleMethod::POST, &format!("{p}/device/authorize"));
        // F011 — social login callback (authorize is already registered above)
        app.route::<ServeAdapter>(SimpleMethod::GET, &format!("{p}/callback"));
        // F02 — auth endpoints (unified login, MFA, logout)
        app.route::<ServeAdapter>(SimpleMethod::POST, "/auth/v1/oidc/authorize");
        app.route::<ServeAdapter>(SimpleMethod::POST, "/auth/v1/mfa");
        app.route::<ServeAdapter>(SimpleMethod::POST, "/auth/v1/oidc/logout");
        // F03 — registration endpoints
        app.route::<ServeAdapter>(SimpleMethod::POST, "/auth/v1/users/register");
        app.route::<ServeAdapter>(SimpleMethod::POST, "/auth/v1/dev/register");
        // F04 — password reset endpoints
        app.route::<ServeAdapter>(SimpleMethod::POST, "/auth/v1/users/request_reset");
        app.route::<ServeAdapter>(SimpleMethod::PUT, "/auth/v1/users/{id}/reset");
        // F05 — Proof of Work endpoints
        app.route::<ServeAdapter>(SimpleMethod::GET, "/auth/v1/pow");
        app.route::<ServeAdapter>(SimpleMethod::POST, "/auth/v1/pow");
        // F08 — template/config API endpoints
        app.route::<ServeAdapter>(SimpleMethod::GET, "/auth/v1/templates/config");
        app.route::<ServeAdapter>(SimpleMethod::GET, "/auth/v1/templates/password_policy");
        // F09 — account management endpoints
        app.route::<ServeAdapter>(SimpleMethod::GET, "/auth/v1/users/{id}");
        app.route::<ServeAdapter>(SimpleMethod::PUT, "/auth/v1/users/{id}");
        app.route::<ServeAdapter>(SimpleMethod::POST, "/auth/v1/users/{id}/change_password");
        app.route::<ServeAdapter>(SimpleMethod::GET, "/auth/v1/users/{id}/sessions");
        app.route::<ServeAdapter>(SimpleMethod::POST, "/auth/v1/users/{id}/revoke");
        // F06 — WebAuthn/FIDO2 endpoints
        app.route::<ServeAdapter>(SimpleMethod::POST, "/auth/v1/webauthn/register/start");
        app.route::<ServeAdapter>(SimpleMethod::POST, "/auth/v1/webauthn/register/finish");
        app.route::<ServeAdapter>(SimpleMethod::POST, "/auth/v1/webauthn/login/start");
        app.route::<ServeAdapter>(SimpleMethod::POST, "/auth/v1/webauthn/login/finish");
        app.route::<ServeAdapter>(SimpleMethod::DELETE, "/auth/v1/webauthn/{id}");
        app.route::<ServeAdapter>(SimpleMethod::PUT, "/auth/v1/webauthn/{id}");
        // F06 — passkey-only login endpoints
        app.route::<ServeAdapter>(SimpleMethod::POST, "/auth/v1/passkey/login/start");
        app.route::<ServeAdapter>(SimpleMethod::POST, "/auth/v1/passkey/login/finish");
        // F07 — Terms of Service endpoints
        app.route::<ServeAdapter>(SimpleMethod::GET, "/auth/v1/tos/latest");
        app.route::<ServeAdapter>(SimpleMethod::POST, "/auth/v1/tos/accept");
    }

    #[must_use]
    pub fn http_app(self) -> HttpApp<Arc<dyn Serve>> {
        let mut app = HttpApp::new_serve();
        app.ctx.store(self.config);
        app.ctx.store(self.storage);
        Self::register_routes(&mut app, "/idp");
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

