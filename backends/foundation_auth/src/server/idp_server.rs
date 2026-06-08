use std::sync::Arc;

use foundation_http::shared::app::HttpApp;
use foundation_http::shared::serve::Serve;
use foundation_http::SimpleMethod;

use super::config::IdpConfig;
use super::handlers::ServeAdapter;

pub struct IdpServer {
    config: IdpConfig,
}

impl IdpServer {
    #[must_use]
    pub fn new(config: IdpConfig) -> Self {
        Self { config }
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
    }

    #[must_use]
    pub fn http_app(self) -> HttpApp<Arc<dyn Serve>> {
        let mut app = HttpApp::new_serve();
        app.ctx.store(self.config);
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

    #[test]
    fn test_build_http_app() {
        let config = IdpConfig::new("https://auth.example.com".into());
        let server = IdpServer::new(config);
        let app = server.http_app();
        assert!(app.ctx.contains::<IdpConfig>());
    }

    #[test]
    fn test_default_prefix_routes() {
        let config = IdpConfig::new("https://auth.example.com".into());
        let server = IdpServer::new(config);
        let app = server.http_app();

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
        let mut app = HttpApp::new_serve();
        app.ctx.store(config);
        IdpServer::register_routes(&mut app, "/auth/v1");

        assert!(app.router.dispatch(&SimpleMethod::GET, "/auth/v1/.well-known/openid-configuration").is_some());
        assert!(app.router.dispatch(&SimpleMethod::POST, "/auth/v1/token").is_some());
    }

    #[test]
    fn test_no_route_without_prefix() {
        let config = IdpConfig::new("https://auth.example.com".into());
        let server = IdpServer::new(config);
        let app = server.http_app();

        assert!(app.router.dispatch(&SimpleMethod::GET, "/token").is_none());
        assert!(app.router.dispatch(&SimpleMethod::GET, "/unknown").is_none());
    }
}
