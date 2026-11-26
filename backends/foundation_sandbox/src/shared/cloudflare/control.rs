//! The control plane of one Durable Object (DO).
//!
//! [`DoControlPlane`] restores the sandbox once for each DO object, touches the
//! idle deadline on each request, routes the request through a
//! `foundation_connectrpc` router, and stops an idle sandbox when the one alarm
//! fires.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use foundation_connectrpc::ConnectRpcHandler;
use foundation_context::Ctx;

use crate::shared::cloudflare::container_service::ContainerControlService;
use crate::shared::cloudflare::driver::{CloudflareContainerDriver, CloudflareDoDriver};
use crate::shared::cloudflare::transport::PortConnector;
use crate::shared::cloudflare::{CloudflareConfig, CloudflareHost, DoSlaveConnector, DoStorage};
use crate::shared::cloudflare::{CloudflareContainerControl, DoRecordStore};
use crate::shared::container::ContainerControl;
use crate::shared::control::record::created_unix_ms;
use crate::shared::control::stop::spawn_detached;
use crate::shared::control::wait_until;
use crate::shared::control::{
    ControlService, DropPolicy, ManagerConfig, SandboxIdPolicy, SandboxManager, SandboxProvider,
};
use crate::shared::error::SandboxResult;
use crate::shared::ids::SandboxId;
use crate::shared::limits::SandboxLimits;
use crate::shared::rpc::{register_container_service, register_sandbox_service};

/// The storage key of the DO's sandbox identifier.
pub const SANDBOX_ID_KEY: &str = "sandbox_id";
/// The storage key of the idle deadline as Unix milliseconds.
pub const IDLE_DEADLINE_KEY: &str = "idle_deadline_ms";

/// One request to the Durable Object.
pub type HttpRequestParts = foundation_netio::shared::http::SimpleIncomingRequest;

/// One response from the Durable Object.
pub type HttpResponseParts = foundation_netio::shared::http::SimpleOutgoingResponse;

/// The shared state of one DO control plane.
struct Inner {
    ctx: Ctx,
    host: CloudflareHost,
    manager: Arc<SandboxManager>,
    handler: ConnectRpcHandler,
    id: SandboxId,
    restored: AtomicBool,
}

/// The control plane of one Durable Object.
pub struct DoControlPlane {
    inner: Arc<Inner>,
}

impl DoControlPlane {
    /// Builds the control plane of one DO.
    ///
    /// `id` is `Some` on a request and `None` on an alarm. The identifier is
    /// stored under [`SANDBOX_ID_KEY`] the first time it is known.
    ///
    /// # Errors
    /// The error of the DO storage.
    pub async fn open(
        ctx: Ctx,
        host: CloudflareHost,
        id: Option<SandboxId>,
        limits: SandboxLimits,
    ) -> SandboxResult<Option<Self>> {
        let id = match id {
            Some(id) => {
                host.storage
                    .put(SANDBOX_ID_KEY, id.to_string().into_bytes())
                    .await?;
                Some(id)
            }
            None => stored_sandbox_id(&*host.storage).await?,
        };
        let Some(id) = id else {
            if let Err(error) = host.alarm.clear().await {
                tracing::warn!(error = %error, "the idle alarm was not cleared");
            }
            return Ok(None);
        };
        let mut limits = limits;
        limits.boot_timeout = host.config.boot_timeout;

        let connector = Arc::new(PortConnector {
            container: Arc::clone(&host.container),
            config: Arc::clone(&host.config),
            do_slave: host
                .service
                .as_ref()
                .map(|service| Arc::new(DoSlaveConnector::new(Arc::clone(service)))),
        });

        let drivers: Vec<Arc<dyn crate::shared::driver::SandboxDriver>> = vec![
            Arc::new(CloudflareContainerDriver::new(
                host.clone(),
                Arc::clone(&connector) as Arc<dyn crate::shared::host::SlaveConnector>,
            )),
            Arc::new(CloudflareDoDriver::new(&host.config)),
        ];
        let store = Arc::new(DoRecordStore::new(
            Arc::clone(&host.storage),
            Arc::clone(&host.index),
            Arc::clone(&host.alarm),
        ));
        let manager_config = ManagerConfig::new()
            .with_limits(limits.clone())
            .with_max_sandboxes(1)
            .with_id_policy(SandboxIdPolicy::Fixed(id.clone()))
            .with_on_drop(DropPolicy::Detach);
        let manager = Arc::new(SandboxManager::new(
            ctx.clone(),
            drivers,
            Arc::clone(&connector) as Arc<dyn crate::shared::host::SlaveConnector>,
            store,
            manager_config,
        ));

        let provider: Arc<dyn SandboxProvider> = manager.clone();
        let sandbox_service = Arc::new(ControlService::new(provider, limits.clone()));
        let control: Arc<dyn ContainerControl> = Arc::new(
            CloudflareContainerControl::new(
                host.clone(),
                Arc::clone(&connector) as Arc<dyn crate::shared::host::SlaveConnector>,
            )
            .with_limits(limits.clone()),
        );
        let container_service = Arc::new(ContainerControlService::new(
            ctx.clone(),
            control,
            limits.clone(),
        ));
        let mut router = foundation_connectrpc::Router::new();
        register_sandbox_service(&mut router, sandbox_service);
        register_container_service(&mut router, container_service);
        let handler = router.into_handler();

        Ok(Some(Self {
            inner: Arc::new(Inner {
                ctx,
                host,
                manager,
                handler,
                id,
                restored: AtomicBool::new(false),
            }),
        }))
    }

    /// The sandbox identifier of this DO.
    #[must_use]
    pub fn id(&self) -> &SandboxId {
        &self.inner.id
    }

    /// This DO's sandbox manager.
    #[must_use]
    pub fn sandboxes(&self) -> Arc<dyn SandboxProvider> {
        self.inner.manager.clone()
    }

    /// The configuration of this DO.
    #[must_use]
    pub fn config(&self) -> &CloudflareConfig {
        &self.inner.host.config
    }

    /// Loads the record and finds the substrate again. It runs one time for each
    /// object.
    async fn restore(&self) {
        if self.inner.restored.swap(true, Ordering::SeqCst) {
            return;
        }
        if let Err(error) = self.inner.manager.restore().await {
            tracing::warn!(error = %error, id = %self.inner.id, "restore failed");
        }
    }

    /// Moves the idle deadline to `now + idle_timeout` and sets the alarm.
    async fn touch(&self) {
        touch_inner(&self.inner).await;
    }

    /// Handles one HTTP request to this DO.
    pub async fn handle(&self, request: HttpRequestParts) -> HttpResponseParts {
        self.restore().await;
        self.touch().await;
        self.inner
            .handler
            .dispatch(self.inner.ctx.clone(), request)
            .await
    }

    /// The one DO alarm.
    pub async fn on_alarm(&self) {
        self.restore().await;
        let now = created_unix_ms(&self.inner.ctx);
        let deadline = read_deadline(&*self.inner.host.storage).await;
        let sandboxes = self.inner.manager.list().await.unwrap_or_default();
        if sandboxes.is_empty() {
            if let Err(error) = self.inner.host.alarm.clear().await {
                tracing::warn!(error = %error, "the idle alarm was not cleared");
            }
            return;
        }
        match deadline {
            Some(deadline) if now >= deadline => {
                if let Err(error) = self.inner.manager.stop(self.inner.id.clone()).await {
                    tracing::warn!(error = %error, id = %self.inner.id, "the idle stop failed");
                }
            }
            Some(deadline) => {
                if let Err(error) = self.inner.host.alarm.set(deadline).await {
                    tracing::warn!(error = %error, "the idle alarm was not set again");
                }
            }
            None => {
                if let Err(error) = self.inner.host.alarm.clear().await {
                    tracing::warn!(error = %error, "the idle alarm was not cleared");
                }
            }
        }
    }

    /// A guard that keeps the sandbox alive while a response streams.
    ///
    /// While the guard lives, a timer task touches the sandbox every
    /// `idle_timeout / 2`.
    #[must_use]
    pub fn stream_guard(&self) -> StreamGuard {
        let inner = Arc::clone(&self.inner);
        let alive = Arc::new(AtomicBool::new(true));
        let touches = Arc::new(AtomicUsize::new(0));
        let task_alive = Arc::clone(&alive);
        let task_touches = Arc::clone(&touches);
        let period = self.inner.host.config.idle_timeout / 2;
        spawn_detached(async move {
            loop {
                if !task_alive.load(Ordering::SeqCst) {
                    return;
                }
                touch_inner(&inner).await;
                task_touches.fetch_add(1, Ordering::SeqCst);
                let deadline = inner.ctx.clock().now() + period;
                if wait_until(deadline, &inner.ctx).await.is_err() {
                    return;
                }
            }
        });
        StreamGuard { alive, touches }
    }
}

/// A guard that stops the stream touch task when it is dropped.
pub struct StreamGuard {
    alive: Arc<AtomicBool>,
    touches: Arc<AtomicUsize>,
}

impl StreamGuard {
    /// How many touches the guard's task already made.
    #[must_use]
    pub fn touches(&self) -> usize {
        self.touches.load(Ordering::SeqCst)
    }
}

impl Drop for StreamGuard {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::SeqCst);
    }
}

/// The stored sandbox identifier.
async fn stored_sandbox_id(storage: &dyn DoStorage) -> SandboxResult<Option<SandboxId>> {
    let Some(bytes) = storage.get(SANDBOX_ID_KEY).await? else {
        return Ok(None);
    };
    let Ok(text) = String::from_utf8(bytes) else {
        return Ok(None);
    };
    Ok(text.parse().ok())
}

/// The stored idle deadline.
async fn read_deadline(storage: &dyn DoStorage) -> Option<u64> {
    let value = match storage.get(IDLE_DEADLINE_KEY).await {
        Ok(value) => value?,
        Err(error) => {
            tracing::warn!(error = %error, "the idle deadline was not read");
            return None;
        }
    };
    String::from_utf8(value).ok()?.parse().ok()
}

/// Moves the idle deadline and sets the alarm.
async fn touch_inner(inner: &Inner) {
    let now = created_unix_ms(&inner.ctx);
    let timeout = millis(inner.host.config.idle_timeout);
    let next = now.saturating_add(timeout);
    let current = read_deadline(&*inner.host.storage).await;
    let deadline = current.map_or(next, |current| current.max(next));
    if let Err(error) = inner
        .host
        .storage
        .put(IDLE_DEADLINE_KEY, deadline.to_string().into_bytes())
        .await
    {
        tracing::warn!(error = %error, "the idle deadline was not stored");
    }
    if let Err(error) = inner.host.alarm.set(deadline).await {
        tracing::warn!(error = %error, "the idle alarm was not set");
    }
}

/// A duration in whole milliseconds, saturating at `u64`.
fn millis(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
