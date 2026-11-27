//! The native host: configuration, operator keys, listeners and the core.
//!
//! WHY: a program on a native machine starts one binary and serves every agent
//! instance from one loaded model. The host builds each part after the part it
//! needs.
//!
//! WHAT: [`NativeHost::start`], the modules of the host, and [`RunningServer`].
//!
//! HOW: keys, the certificate authority, the trust store, the catalog, the
//! registry, the core service and finally the listeners. Every side effect
//! comes from the [`Ctx`] and its [`Env`](foundation_context::env::Env). No
//! path reads a global clock, a global random source or a global identifier
//! source.

mod catalog;
pub mod client;
mod config;
mod listener;
mod security;

use std::sync::Arc;

use foundation_auth::trust::{SqlTrustStore, TrustedClients};
use foundation_context::Ctx;
use foundation_errstacks::ErrorTrace;

use crate::core::registry::{InMemoryStores, LocalInstanceRegistry};
use crate::core::service::AgentRpcService;
use crate::error::{AgentRpcError, AgentRpcResult};
use crate::trust::TRUSTED_CLIENTS_DB_FILE;

pub use catalog::{BackgroundLoader, LoadFuture, LocalCatalog, ModelLoader};
pub use config::{
    NativeHostConfig, NativeSecurity, OperatorKeyConfig, ServerIdentityConfig, CA_KEY_ENV,
    CA_KEY_FILE, SERVER_KEY_ENV, SERVER_KEY_FILE, SIGNING_KEY_ENV, SIGNING_KEY_FILE,
};
pub use listener::RunningServer;
pub use security::{
    build_mtls, certificate_authority, load_operator_keys, server_identity, MtlsParts,
    NativeSecurityParts, OperatorKeys, ENROLL_SECRET_VALIDITY,
};

/// The native host.
pub struct NativeHost;

impl NativeHost {
    /// Build every part, bind the listeners and serve.
    ///
    /// Call it on the valtron pool: the binary's `#[valtron] async fn main`
    /// awaits it. The `ctx` carries the clock and the installed
    /// [`Env`](foundation_context::env::Env).
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] for a bad configuration or a missing operator
    /// key; [`AgentRpcError::Storage`] when a store cannot open;
    /// [`AgentRpcError::ModelLoad`] when `preload` is on and a model cannot
    /// load; [`AgentRpcError::Transport`] when an address cannot be bound.
    pub async fn start(config: NativeHostConfig, ctx: Ctx) -> AgentRpcResult<RunningServer> {
        // Invariant I2: the insecure mode refuses a non-loopback bind before
        // the host binds anything.
        if config.security == NativeSecurity::InsecurePlaintext && !is_loopback_bind(&config.bind) {
            return Err(state_error(format!(
                "InsecurePlaintext refuses the non-loopback bind {:?}: it accepts any caller. \
                 Bind a loopback address (127.0.0.1 or ::1), or use mTLS",
                config.bind
            )));
        }

        let catalog = if let Some(catalog) = config.catalog.clone() {
            catalog
        } else {
            let catalog = Arc::new(LocalCatalog::new(
                &config.models,
                Arc::new(BackgroundLoader),
                ctx.clone(),
            )?);
            if config.preload {
                catalog.preload_all().await?;
            }
            catalog
        };

        let registry = Arc::new(LocalInstanceRegistry::new(
            Arc::clone(&catalog),
            InMemoryStores,
            config.registry.clone(),
            config.turns.clone(),
            ctx.clone(),
        )?);

        let registry_trait: Arc<dyn crate::core::registry::InstanceRegistry> = registry.clone();
        let service = Arc::new(AgentRpcService::new(
            registry_trait,
            catalog,
            config.registry.clone(),
            config.turns.clone(),
            vec!["connect".into(), "grpc".into(), "grpc-web".into()],
            vec!["mtls".into()],
            "native",
        ));

        let keys = match config.security {
            NativeSecurity::InsecurePlaintext => listener::HostKeys::Insecure,
            NativeSecurity::Mtls { .. } => {
                let OperatorKeys {
                    signing,
                    ca: ca_key,
                    server: server_key,
                } = load_operator_keys(&config)?;
                let ca = certificate_authority(&config, ca_key, &ctx)?;
                let identity = server_identity(&config, &ca, &server_key, &ctx)?;
                let db_path = config.state_dir.join(TRUSTED_CLIENTS_DB_FILE);
                let store = SqlTrustStore::open_sqlite(db_path, ctx.clone())
                    .await
                    .map_err(|error| {
                        state_error(format!("opening the trusted-client store: {error}"))
                    })?;
                let trust = TrustedClients::open(Arc::new(store), &ctx, config.trust.clone())
                    .await
                    .map_err(|error| {
                        state_error(format!("opening the trusted clients: {error}"))
                    })?;
                listener::HostKeys::Mtls {
                    signing: Box::new(signing),
                    ca,
                    identity,
                    trust,
                }
            }
        };

        listener::serve(&config, service, keys, &ctx, registry)
    }
}

/// Why a host part failed to build.
fn state_error(what: impl Into<String>) -> ErrorTrace<AgentRpcError> {
    AgentRpcError::Config(what.into()).into()
}

/// True for a bind address on the loopback interface.
///
/// `localhost`, an IPv4 address in `127.0.0.0/8`, or `::1`. A domain other
/// than `localhost` is not loopback: the host cannot prove where it resolves.
#[must_use]
pub fn is_loopback_bind(bind: &str) -> bool {
    if let Ok(address) = bind.parse::<std::net::SocketAddr>() {
        return address.ip().is_loopback();
    }
    let host = match bind.rsplit_once(':') {
        Some((host, _port)) => host,
        None => bind,
    };
    let host = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    host.parse::<std::net::IpAddr>()
        .is_ok_and(|address| address.is_loopback())
}
