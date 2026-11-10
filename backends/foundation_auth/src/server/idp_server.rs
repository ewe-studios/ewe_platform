use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::{execute, from_future, sleep_async, Clock};
use foundation_http::ShutdownSignal;

use foundation_http::native::serve::Serve;
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::Ctx;
use foundation_http::SimpleMethod;

use super::config::IdpConfig;
use super::handlers::IdpError;
use super::handlers::ServeAdapter;
use super::storage::{HandlerStorage, StorageOpError};
use crate::shared::auth_session::memory::MemoryAuthSessionStore;
use foundation_db::{KeyValueStore, MemoryStorage};

/// The `IdP` server: routes plus the config and storage its handlers share.
///
/// `KV` is the cache for short-lived handshake state (`PoW`, WebAuthn); the
/// default is the in-memory store.
pub struct IdpServer<KV: KeyValueStore + Clone + 'static = MemoryStorage> {
    config: IdpConfig,
    storage: Arc<HandlerStorage<KV>>,
    social: Option<Arc<MemoryAuthSessionStore>>,
    clock: Arc<dyn Clock>,
}

impl<KV: KeyValueStore + Clone + 'static> IdpServer<KV> {
    /// Build the server over a config and storage.
    ///
    /// # Panics
    ///
    /// Panics if the cache cannot make single-use data single use, or the
    /// session key is too short (see [`try_new`](Self::try_new)); use
    /// `try_new` to handle those errors.
    #[must_use]
    pub fn new(config: IdpConfig, storage: Arc<HandlerStorage<KV>>, clock: Arc<dyn Clock>) -> Self {
        Self::try_new(config, storage, clock).unwrap_or_else(|e| panic!("{e}"))
    }

    /// Build the server over a config and storage, or refuse.
    ///
    /// `ToS` codes and MFA challenges live in the cache and must work once. That
    /// needs an atomic `KeyValueStore::take`. A cache without it (Workers KV)
    /// is refused unless `IdpConfig::allow_non_atomic_cache` is set. Use D1
    /// or a Durable Object for the cache instead.
    ///
    /// # Errors
    ///
    /// Returns [`IdpError::Internal`] if the cache `take` is not atomic and the
    /// risk is not accepted, or if `IdpConfig::session_key` is shorter than 32
    /// bytes.
    pub fn try_new(
        config: IdpConfig,
        storage: Arc<HandlerStorage<KV>>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, IdpError> {
        if !storage.cache.take_is_atomic() && !config.allow_non_atomic_cache {
            return Err(IdpError::Internal(
                "the cache has no atomic take, so one-time codes (ToS codes, MFA challenges) \
                 could be used twice by two simultaneous requests. Use a cache with an atomic \
                 take (memory, SQL, D1) or set IdpConfig::allow_non_atomic_cache to accept the risk"
                    .into(),
            ));
        }
        if config.session_key.len() < 32 {
            return Err(IdpError::Internal(
                "IdpConfig::session_key must be at least 32 bytes".into(),
            ));
        }
        Ok(Self {
            config,
            storage,
            social: None,
            clock,
        })
    }

    /// Delete expired short-lived state (`ToS` codes, MFA challenges, `PoW`
    /// challenges) from the cache; returns how many entries it deleted. Call
    /// it on a timer. See [`HandlerStorage::sweep_expired`].
    ///
    /// # Errors
    ///
    /// Returns [`StorageOpError`] if a cache call fails.
    pub async fn sweep_expired(&self) -> Result<usize, StorageOpError> {
        self.storage.sweep_expired().await
    }

    /// Start the cache sweeper: a valtron task that calls
    /// [`sweep_expired`](Self::sweep_expired) every `IdpConfig::sweep_interval`
    /// (default 5 minutes) and ends when `shutdown` is set. Pass the signal you
    /// give to the HTTP server. Needs an initialised valtron pool.
    ///
    /// The task sleeps with the valtron timer (no thread, no worker is held
    /// while it waits) and checks `shutdown` at least every 500 ms.
    ///
    /// # Errors
    ///
    /// Returns [`IdpError::Internal`] if the valtron pool is not initialised.
    pub fn start_sweeper(&self, shutdown: Arc<ShutdownSignal>) -> Result<(), IdpError> {
        let storage = Arc::clone(&self.storage);
        let interval = self.config.sweep_interval;
        let slice = interval.min(Duration::from_millis(500));
        let task = from_future(async move {
            while !shutdown.probe() {
                if let Err(e) = storage.sweep_expired().await {
                    eprintln!("IdP cache sweep failed: {e}");
                }
                let mut waited = Duration::ZERO;
                while waited < interval && !shutdown.probe() {
                    sleep_async(slice).await;
                    waited += slice;
                }
            }
        });
        // The task runs on the pool; the handle is not needed.
        execute(task, None).map_err(|e| IdpError::Internal(format!("valtron: {e}")))?;
        Ok(())
    }

    /// Turn on social login: the routes `GET {prefix}/authorize?provider=...`
    /// and `GET {prefix}/callback` keep their handshake state in this store.
    #[must_use]
    pub fn with_social_login(mut self, store: Arc<MemoryAuthSessionStore>) -> Self {
        self.social = Some(store);
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
        app.route::<ServeAdapter<KV>>(SimpleMethod::POST, "/auth/v1/mfa/totp/enroll/start");
        app.route::<ServeAdapter<KV>>(SimpleMethod::POST, "/auth/v1/mfa/totp/enroll/confirm");
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
        // The app context carries this server's injected clock, so every handler
        // reads the clock the host chose.
        app.ctx = Ctx::new(Arc::clone(&self.clock));
        let dev_mode = self.config.dev_mode;
        app.ctx.store(self.config);
        // Stored as `Arc<HandlerStorage<KV>>`; `ServeAdapter::create` looks up
        // exactly that type.
        app.ctx.store(self.storage);
        if let Some(social) = self.social {
            // Stored as `Arc<MemoryAuthSessionStore>`; `ServeAdapter::create`
            // looks up exactly that type.
            app.ctx.store(social);
        }
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
