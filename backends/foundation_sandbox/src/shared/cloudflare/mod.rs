//! The Cloudflare part of the platform.
//!
//! One Durable Object (DO) holds one container and one sandbox. This module
//! holds the traits that the DO implements, the host bundle and the
//! configuration. The logic here depends only on the traits, so it compiles and
//! passes its tests on a native host with fakes. Only [`crate::cloudflare`]
//! names a `worker` type.
//!
//! The trait set: [`ContainerApi`] (the DO's container), [`DoStorage`] (the DO
//! key-value store), [`DoAlarm`] (the DO's one timer) and [`SandboxIndex`] (the
//! Worker-wide list of sandboxes).

use std::sync::Arc;
use std::time::Duration;

use crate::shared::error::SandboxResult;
use crate::shared::BoxFuture;

pub mod container;
pub mod container_service;
pub mod control;
pub mod do_slave;
pub mod driver;
pub mod model_slave;
pub mod platform;
pub mod routing;
pub mod store;
pub mod transport;

#[cfg(feature = "testing")]
pub mod testing;

pub use container::{CloudflareContainerControl, DoContainerClient, DoContainerOps};
pub use container_service::ContainerControlService;
pub use control::{DoControlPlane, HttpRequestParts, HttpResponseParts, StopReason, StreamGuard};
pub use do_slave::{DoSlaveConnector, ServiceBinding};
pub use driver::{CloudflareContainerDriver, CloudflareDoDriver};
pub use model_slave::{
    open_router_delta, push_answer, ModelBackend, ModelClient, ModelConfig, ModelRequest,
    ModelSecrets, ModelSlave,
};
pub use platform::{as_cloudflare, CloudflarePlatform};
pub use routing::{route, RouteTarget};
pub use store::DoRecordStore;
pub use transport::{ContainerPortTransport, PortConnector, PortSource};

// The port values of this part are the port values of the contract (feature 00,
// section 6.13). One type, not two.
pub use crate::shared::container::{PortError, PortRequest, PortResponse};

/// A futures stream that is `Send` on every target.
///
/// A `worker` response body is a JavaScript stream, so it is `!Send`. On
/// single-threaded wasm [`foundation_compact::SendWrapper`] supplies the bound.
/// On a native target the wrapped stream is `Send` already, and the wrapper is
/// transparent.
pub(crate) struct SendStream<S>(foundation_compact::SendWrapper<S>);

impl<S> SendStream<S> {
    pub(crate) fn new(inner: S) -> Self {
        Self(foundation_compact::SendWrapper::new(inner))
    }
}

impl<S: futures::Stream> futures::Stream for SendStream<S> {
    type Item = S::Item;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        // SAFETY: the wrapper is boxed and pinned, so the inner stream never
        // moves after this point.
        let this = unsafe { self.get_unchecked_mut() };
        let inner = unsafe { std::pin::Pin::new_unchecked(&mut *this.0) };
        inner.poll_next(context)
    }
}

/// The container of one DO. It mirrors `worker::Container`.
pub trait ContainerApi: Send + Sync + 'static {
    /// Whether the container runs.
    fn running(&self) -> bool;

    /// Starts the container with `options`.
    ///
    /// # Errors
    /// [`crate::shared::error::SandboxError::SubstrateApi`] when the platform
    /// refuses the start.
    fn start(&self, options: ContainerStart) -> SandboxResult<()>;

    /// Destroys the container and every resource it holds.
    ///
    /// # Errors
    /// The error of the platform call.
    fn destroy(&self) -> BoxFuture<'_, SandboxResult<()>>;

    /// Sends one signal to the container.
    ///
    /// # Errors
    /// The error of the platform call.
    fn signal(&self, signo: i32) -> SandboxResult<()>;

    /// Completes when the container exits.
    ///
    /// # Errors
    /// The error of the platform call.
    fn wait_for_exit(&self) -> BoxFuture<'_, SandboxResult<()>>;

    /// Sends one HTTP request to `port` in the container.
    ///
    /// The response body streams.
    ///
    /// # Errors
    /// [`PortError::NotListening`] when no process listens on the port, or
    /// [`PortError::Other`] for every other failure.
    fn fetch(
        &self,
        port: u16,
        request: PortRequest,
    ) -> BoxFuture<'_, Result<PortResponse, PortError>>;
}

/// What the DO asks the platform to start.
///
/// The image and the instance type are in the Worker configuration, not here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContainerStart {
    /// The entry point. It replaces the image default.
    pub entrypoint: Vec<String>,
    /// Whether the container has the internet.
    pub enable_internet: bool,
    /// The container environment.
    pub env: Vec<(String, String)>,
}

/// The key-value store of one DO.
pub trait DoStorage: Send + Sync + 'static {
    /// The value under `key`, or `None`.
    ///
    /// # Errors
    /// The error of the store.
    fn get(&self, key: &str) -> BoxFuture<'_, SandboxResult<Option<Vec<u8>>>>;

    /// Writes `value` under `key`.
    ///
    /// # Errors
    /// The error of the store.
    fn put(&self, key: &str, value: Vec<u8>) -> BoxFuture<'_, SandboxResult<()>>;

    /// Removes `key`. A missing key is not an error.
    ///
    /// # Errors
    /// The error of the store.
    fn delete(&self, key: &str) -> BoxFuture<'_, SandboxResult<()>>;
}

/// The one timer of a DO.
pub trait DoAlarm: Send + Sync + 'static {
    /// Sets the alarm at `at_unix_ms`. It replaces a pending alarm.
    ///
    /// # Errors
    /// The error of the platform call.
    fn set(&self, at_unix_ms: u64) -> BoxFuture<'_, SandboxResult<()>>;

    /// Clears the alarm.
    ///
    /// # Errors
    /// The error of the platform call.
    fn clear(&self) -> BoxFuture<'_, SandboxResult<()>>;
}

/// The Worker-wide list of sandboxes.
///
/// It is a view: an index error is logged and does not fail the call.
pub trait SandboxIndex: Send + Sync + 'static {
    /// Adds or replaces one entry.
    ///
    /// # Errors
    /// The error of the index.
    fn put(&self, info: crate::shared::control::SandboxInfo) -> BoxFuture<'_, SandboxResult<()>>;

    /// Removes one entry. A missing entry is not an error.
    ///
    /// # Errors
    /// The error of the index.
    fn remove(&self, id: &crate::shared::ids::SandboxId) -> BoxFuture<'_, SandboxResult<()>>;
}

/// What one DO can do.
///
/// It is one bundle: the container, the store, the alarm, the index and the
/// configuration. A caller builds it from the DO state.
#[derive(Clone)]
pub struct CloudflareHost {
    /// The container of this DO.
    pub container: Arc<dyn ContainerApi>,
    /// The key-value store of this DO.
    pub storage: Arc<dyn DoStorage>,
    /// The one timer of this DO.
    pub alarm: Arc<dyn DoAlarm>,
    /// The Worker-wide list of sandboxes.
    pub index: Arc<dyn SandboxIndex>,
    /// The service binding of the model slave, when the host has one.
    pub service: Option<Arc<dyn do_slave::ServiceBinding>>,
    /// The configuration of this part.
    pub config: Arc<CloudflareConfig>,
}

/// The configuration of the Cloudflare part.
///
/// Every field is configuration, never a constant in a code path. Build a value
/// with [`CloudflareConfig::default`] and change the fields that matter with the
/// `with_` methods.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloudflareConfig {
    /// The name that a sandbox spec must use for its container image.
    ///
    /// Default `sandbox`: the image name of the Worker configuration.
    pub image_name: String,
    /// The name of this DO: the container identifier and the sandbox identifier.
    ///
    /// Default `sandbox`. Feature 08 sets the DO name.
    pub name: String,
    /// The port of the slave in the container.
    ///
    /// Default 1024.
    pub slave_port: u16,
    /// The path of the slave in the container.
    ///
    /// Default `/usr/local/bin/agent-slave`.
    pub slave_path: String,
    /// The time with no request after which the sandbox stops.
    ///
    /// Default 10 minutes: the default `sleepAfter` of `@cloudflare/containers`.
    pub idle_timeout: Duration,
    /// The deadline for the slave to answer after the container starts.
    ///
    /// Default 60 s. A cold container start with a first image pull takes much
    /// longer than a micro virtual machine (VM) boot.
    pub boot_timeout: Duration,
    /// The deadline for a remote `Wait` call, or `None` for no deadline.
    ///
    /// Default `None`: a container runs for hours, so a boot deadline is wrong.
    pub wait_timeout: Option<Duration>,
    /// The model slave configuration of decision D16.
    ///
    /// Default `None`. The model slave needs this configuration.
    pub model: Option<ModelConfig>,
    /// The name of the service binding of the model slave.
    ///
    /// Default `SLAVE`.
    pub slave_binding: String,
    /// The most bytes of error text that one port error carries.
    ///
    /// Default 64 KiB: [`crate::shared::limits::SandboxLimits::max_error_message`].
    pub max_error_message: usize,
    /// The most response frames held for one streaming response.
    ///
    /// Default 4: [`foundation_core::valtron::Pipe::DEFAULT_DEPTH`].
    pub stream_queue_len: usize,
    /// The longest time that one streaming response stays open.
    ///
    /// The limit applies to each `Exec`, `Create` and `FileRead` response, not
    /// to the sandbox. A second command gets a new stream and a new deadline.
    /// At the deadline the stream ends with `deadline_exceeded`, and the slave
    /// kills its program when the stream ends. An open stream is an open
    /// request, so it keeps the Durable Object (DO) awake and billed.
    ///
    /// Default 1 hour: the value of feature 18. It is longer than the
    /// 60-second `proxy_read_timeout` default of nginx, because one build
    /// command can run for hours. An operator can raise it.
    pub stream_max_duration: Duration,
}

impl Default for CloudflareConfig {
    fn default() -> Self {
        Self {
            image_name: "sandbox".to_string(),
            name: "sandbox".to_string(),
            slave_port: 1024,
            slave_path: "/usr/local/bin/agent-slave".to_string(),
            idle_timeout: Duration::from_secs(600),
            boot_timeout: Duration::from_secs(60),
            wait_timeout: None,
            model: None,
            slave_binding: "SLAVE".to_string(),
            max_error_message: crate::shared::limits::SandboxLimits::default().max_error_message,
            stream_queue_len: foundation_core::valtron::Pipe::<()>::DEFAULT_DEPTH,
            stream_max_duration: Duration::from_secs(3_600),
        }
    }
}

impl CloudflareConfig {
    /// A configuration with every default.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Replaces [`Self::image_name`].
    #[must_use]
    pub fn with_image_name(mut self, value: impl Into<String>) -> Self {
        self.image_name = value.into();
        self
    }

    /// Replaces [`Self::name`].
    #[must_use]
    pub fn with_name(mut self, value: impl Into<String>) -> Self {
        self.name = value.into();
        self
    }

    /// Replaces [`Self::slave_port`].
    #[must_use]
    pub fn with_slave_port(mut self, value: u16) -> Self {
        self.slave_port = value;
        self
    }

    /// Replaces [`Self::slave_path`].
    #[must_use]
    pub fn with_slave_path(mut self, value: impl Into<String>) -> Self {
        self.slave_path = value.into();
        self
    }

    /// Replaces [`Self::idle_timeout`].
    #[must_use]
    pub fn with_idle_timeout(mut self, value: Duration) -> Self {
        self.idle_timeout = value;
        self
    }

    /// Replaces [`Self::boot_timeout`].
    #[must_use]
    pub fn with_boot_timeout(mut self, value: Duration) -> Self {
        self.boot_timeout = value;
        self
    }

    /// Replaces [`Self::wait_timeout`].
    #[must_use]
    pub fn with_wait_timeout(mut self, value: Option<Duration>) -> Self {
        self.wait_timeout = value;
        self
    }

    /// Replaces [`Self::model`].
    #[must_use]
    pub fn with_model(mut self, value: Option<ModelConfig>) -> Self {
        self.model = value;
        self
    }

    /// Replaces [`Self::slave_binding`].
    #[must_use]
    pub fn with_slave_binding(mut self, value: impl Into<String>) -> Self {
        self.slave_binding = value.into();
        self
    }

    /// Replaces [`Self::max_error_message`].
    #[must_use]
    pub fn with_max_error_message(mut self, value: usize) -> Self {
        self.max_error_message = value;
        self
    }

    /// Replaces [`Self::stream_queue_len`].
    #[must_use]
    pub fn with_stream_queue_len(mut self, value: usize) -> Self {
        self.stream_queue_len = value;
        self
    }

    /// Replaces [`Self::stream_max_duration`].
    #[must_use]
    pub fn with_stream_max_duration(mut self, value: Duration) -> Self {
        self.stream_max_duration = value;
        self
    }

    /// The entry point of the slave in the container.
    #[must_use]
    pub fn slave_entrypoint(&self) -> Vec<String> {
        vec![
            self.slave_path.clone(),
            "--listen".to_string(),
            format!("tcp:0.0.0.0:{}", self.slave_port),
        ]
    }
}

/// The text rule of a failed port fetch.
///
/// The Cloudflare library says "the container is not listening"; every other
/// error is an [`PortError::Other`]. The readiness wait treats both kinds as
/// "try again", so a changed text costs nothing but a less exact message. This
/// function is platform-agnostic, so its test runs on a native host.
#[must_use]
pub fn port_error_from_text(text: &str, max_error_message: usize) -> PortError {
    let max = max_error_message;
    if text.contains("the container is not listening") {
        PortError::NotListening
    } else {
        let mut end = text.len().min(max);
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        PortError::Other(text[..end].to_string())
    }
}
