use std::sync::Arc;

use foundation_http::shared::app::HttpApp;
use foundation_http::shared::serve::Serve;
use foundation_http::SimpleMethod;

use super::config::IdpConfig;
use super::handlers::ServeAdapter;
use super::storage::HandlerStorage;

pub struct IdpServer {
    config: IdpConfig,
    storage: Arc<HandlerStorage>,
}

impl IdpServer {
    #[must_use]
    pub fn new(config: IdpConfig, storage: Arc<HandlerStorage>) -> Self {
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
        // F02 — auth endpoints (unified login, MFA, logout)
        app.route::<ServeAdapter>(SimpleMethod::POST, "/auth/v1/oidc/authorize");
        app.route::<ServeAdapter>(SimpleMethod::POST, "/auth/v1/mfa");
        app.route::<ServeAdapter>(SimpleMethod::POST, "/auth/v1/oidc/logout");
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::server::storage::HandlerStorage;
    use foundation_db::core::storage_provider::{QueryStore, SqlRow, StorageItemStream, DataValue};
    use foundation_db::core::errors::StorageError;
    use foundation_core::valtron::Stream;

    struct TestStore;
    impl QueryStore for TestStore {
        fn query(&self, _sql: &str, _params: &[DataValue]) -> Result<StorageItemStream<'_, SqlRow>, StorageError> {
            Ok(Box::new(std::iter::empty()))
        }
        fn execute(&self, _sql: &str, _params: &[DataValue]) -> Result<u64, StorageError> { Ok(0) }
        fn execute_batch(&self, _sql: &str) -> Result<(), StorageError> { Ok(()) }
    }

    #[test]
    fn test_build_http_app() {
        let config = IdpConfig::new("https://auth.example.com".into());
        let storage = Arc::new(HandlerStorage::new(Arc::new(TestStore)));
        let server = IdpServer::new(config, storage);
        let app = server.http_app();
        assert!(app.ctx.contains::<IdpConfig>());
        // HandlerStorage is stored in ctx and used by ServeFactory at route registration time
    }

    #[test]
    fn test_default_prefix_routes() {
        let config = IdpConfig::new("https://auth.example.com".into());
        let storage = Arc::new(HandlerStorage::new(Arc::new(TestStore)));
        let server = IdpServer::new(config, storage);
        let app = server.http_app();

        // Routes are registered during http_app() — just verify they exist
        assert!(app.router.dispatch(&SimpleMethod::GET, "/idp/.well-known/openid-configuration").is_some());
        assert!(app.router.dispatch(&SimpleMethod::GET, "/idp/.well-known/jwks.json").is_some());
        assert!(app.router.dispatch(&SimpleMethod::GET, "/idp/authorize").is_some());
        assert!(app.router.dispatch(&SimpleMethod::POST, "/idp/token").is_some());
        assert!(app.router.dispatch(&SimpleMethod::GET, "/idp/userinfo").is_some());
        assert!(app.router.dispatch(&SimpleMethod::POST, "/idp/introspect").is_some());
        assert!(app.router.dispatch(&SimpleMethod::POST, "/idp/device/authorize").is_some());
    }

    #[test]
    fn test_custom_prefix() {
        let config = IdpConfig::new("https://auth.example.com".into());
        let storage = Arc::new(HandlerStorage::new(Arc::new(TestStore)));
        let mut app = HttpApp::new_serve();
        app.ctx.store(config);
        app.ctx.store(storage);
        // Now routes can be registered because both config and storage are in ctx
        IdpServer::register_routes(&mut app, "/auth/v1");

        assert!(app.router.dispatch(&SimpleMethod::GET, "/auth/v1/.well-known/openid-configuration").is_some());
        assert!(app.router.dispatch(&SimpleMethod::POST, "/auth/v1/token").is_some());
    }

    #[test]
    fn test_no_route_without_prefix() {
        let config = IdpConfig::new("https://auth.example.com".into());
        let storage = Arc::new(HandlerStorage::new(Arc::new(TestStore)));
        let server = IdpServer::new(config, storage);
        let app = server.http_app();

        assert!(app.router.dispatch(&SimpleMethod::GET, "/token").is_none());
        assert!(app.router.dispatch(&SimpleMethod::GET, "/unknown").is_none());
    }
}
