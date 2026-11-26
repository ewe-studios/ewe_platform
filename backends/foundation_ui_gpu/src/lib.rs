//! Submit local graphics jobs to the event loop or thread that owns the device.
use async_channel::{Receiver, Sender};
use foundation_context::Ctx;
use futures_util::future::LocalBoxFuture;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
pub use wgpu;

#[cfg(target_family = "wasm")]
pub mod browser;
#[cfg(target_family = "wasm")]
pub mod browser_handoff;
mod compute;
pub mod lease;
#[cfg(not(target_family = "wasm"))]
pub mod lease_native;
#[cfg(target_vendor = "apple")]
pub mod metal;
mod surfaces;
#[cfg(any(target_os = "android", target_os = "linux"))]
pub mod vulkan;
pub use surfaces::{check_surface_extent, RenderHost, SurfaceId};

/// Requested features and limits apply to this host's single device.
#[derive(Clone, Debug)]
pub struct GpuConfig {
    pub features: wgpu::Features,
    /// Requested device limits.
    ///
    /// The device requests the adapter's two-dimensional texture limit.
    /// `max_texture_dimension_2d` is the fallback for an adapter that reports
    /// no limit. Its default is 2048 pixels, the WebGPU downlevel default.
    pub limits: wgpu::Limits,
    pub backends: wgpu::Backends,
    /// The default queue accepts 64 waiting commands. Further callers await capacity.
    pub queue_capacity: usize,
    /// Native readback fails when device work exceeds this duration.
    pub readback_timeout: std::time::Duration,
}
impl Default for GpuConfig {
    fn default() -> Self {
        Self {
            features: wgpu::Features::empty(),
            limits: wgpu::Limits::downlevel_defaults(),
            backends: wgpu::Backends::all(),
            queue_capacity: 64,
            readback_timeout: std::time::Duration::from_secs(30),
        }
    }
}

impl GpuConfig {
    /// Return the surface extent limit for one adapter report.
    ///
    /// A non-zero adapter report wins. A zero report uses the configured
    /// fallback, `limits.max_texture_dimension_2d`. The fallback default is
    /// 2048 pixels, the WebGPU downlevel default.
    #[must_use]
    pub fn surface_extent_limit(&self, adapter: u32) -> u32 {
        if adapter > 0 {
            adapter
        } else {
            self.limits.max_texture_dimension_2d
        }
    }

    /// Return the limits to request from one adapter.
    fn requested_limits(&self, adapter: &wgpu::Limits) -> wgpu::Limits {
        let mut limits = self.limits.clone();
        limits.max_texture_dimension_2d =
            self.surface_extent_limit(adapter.max_texture_dimension_2d);
        limits
    }
}

/// Compare requested limits against an adapter report.
///
/// A zero two-dimensional texture report counts as absent, so the configured
/// fallback applies.
fn check_adapter_limits(reported: &wgpu::Limits, requested: &wgpu::Limits) -> bool {
    if reported.max_texture_dimension_2d == 0 {
        let mut comparable = reported.clone();
        comparable.max_texture_dimension_2d = requested.max_texture_dimension_2d;
        requested.check_limits(&comparable)
    } else {
        requested.check_limits(reported)
    }
}

#[derive(Debug, Clone, derive_more::Display, derive_more::Error)]
pub enum GpuError {
    #[display("The host graphics service has stopped")]
    Cancelled,
    #[display("The host has no compatible graphics adapter: {_0}")]
    Unavailable(#[error(not(source))] String),
    #[display("The requested features or limits exceed the adapter capabilities")]
    Unsupported,
    #[display(
        "The surface extent {width} by {height} exceeds the maximum texture dimension {limit}"
    )]
    ExtentUnsupported { width: u32, height: u32, limit: u32 },
    #[display("The graphics device was lost")]
    DeviceLost,
    #[display("The registered surface has been removed")]
    StaleSurface,
    #[display("The surface cannot present on this device")]
    IncompatibleSurface,
    #[display("The surface cannot present at its current size or visibility")]
    Suspended,
    #[display("Graphics operation failed: {_0}")]
    Operation(#[error(not(source))] String),
}

pub struct GraphicsDevice {
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    lost: Arc<AtomicBool>,
}

/// A host can inject a device provider. Tests can return a typed failure.
pub trait DeviceProvider: Send + Sync {
    fn open<'a>(
        &'a self,
        instance: &'a wgpu::Instance,
        config: &'a GpuConfig,
    ) -> LocalBoxFuture<'a, Result<GraphicsDevice, GpuError>>;
}

pub struct SystemDevice;
impl DeviceProvider for SystemDevice {
    fn open<'a>(
        &'a self,
        instance: &'a wgpu::Instance,
        config: &'a GpuConfig,
    ) -> LocalBoxFuture<'a, Result<GraphicsDevice, GpuError>> {
        Box::pin(async move { GraphicsDevice::open(instance, config).await })
    }
}

/// The owner passes itself to each local application job.
pub struct GpuOwner {
    instance: wgpu::Instance,
    config: GpuConfig,
    provider: Arc<dyn DeviceProvider>,
    device: Option<Result<GraphicsDevice, GpuError>>,
    surfaces: surfaces::Surfaces,
    #[cfg(not(target_family = "wasm"))]
    leased_texture: Option<wgpu::Texture>,
}
impl GpuOwner {
    /// Reuse the first initialization. One owner executes all initialization and application jobs.
    ///
    /// # Errors
    /// Returns the device initialization error or device loss.
    pub async fn device(&mut self) -> Result<&GraphicsDevice, GpuError> {
        let result = match &mut self.device {
            Some(result) => result,
            vacant => vacant.insert(self.provider.open(&self.instance, &self.config).await),
        };
        let graphics = result.as_ref().map_err(Clone::clone)?;
        if graphics.lost.load(Ordering::Acquire) {
            self.surfaces.clear();
            Err(GpuError::DeviceLost)
        } else {
            Ok(graphics)
        }
    }

    /// Release old surfaces and invalidate their handles before opening another device.
    ///
    /// # Errors
    /// Returns the error from the replacement device.
    pub async fn reinitialize(&mut self) -> Result<&GraphicsDevice, GpuError> {
        self.surfaces.clear();
        self.device = None;
        self.device().await
    }
}

impl GraphicsDevice {
    async fn open(
        instance: &wgpu::Instance,
        config: &GpuConfig,
    ) -> Result<GraphicsDevice, GpuError> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .map_err(|e| GpuError::Unavailable(e.to_string()))?;
        let reported = adapter.limits();
        let requested = config.requested_limits(&reported);
        if !adapter.features().contains(config.features)
            || !check_adapter_limits(&reported, &requested)
        {
            return Err(GpuError::Unsupported);
        }
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features: config.features,
                required_limits: requested,
                ..Default::default()
            })
            .await
            .map_err(|e| GpuError::Unavailable(e.to_string()))?;
        Ok(Self::from_parts(adapter, device, queue, || {}))
    }

    fn from_parts(
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
        on_loss: impl Fn() + Send + 'static,
    ) -> Self {
        let lost = Arc::new(AtomicBool::new(false));
        let signal = lost.clone();
        device.set_device_lost_callback(move |_, _| {
            signal.store(true, Ordering::Release);
            on_loss();
        });
        Self {
            adapter,
            device,
            queue,
            lost,
        }
    }
}
type Work = Box<dyn for<'a> FnOnce(&'a mut GpuOwner) -> LocalBoxFuture<'a, ()> + Send>;

/// This context entry can cross threads. The device and its surfaces stay on their owner.
#[derive(Clone)]
pub struct GpuService {
    work: Sender<Work>,
    stopped: Receiver<()>,
}

impl GpuService {
    /// Install a lazy device service on the host's graphics execution thread.
    ///
    /// # Errors
    /// Returns an error when the host cannot start a graphics owner.
    pub fn install(ctx: &Ctx, config: GpuConfig) -> Result<Self, GpuError> {
        Self::install_with(ctx, config, Arc::new(SystemDevice))
    }

    ///
    /// # Errors
    /// Returns an error for an invalid queue capacity or unavailable execution thread.
    pub fn install_with(
        ctx: &Ctx,
        config: GpuConfig,
        provider: Arc<dyn DeviceProvider>,
    ) -> Result<Self, GpuError> {
        if let Some(service) = ctx.get_cloned::<Self>() {
            return Ok(service);
        }
        if config.queue_capacity == 0 {
            return Err(GpuError::Operation(
                "The queue capacity must be positive".into(),
            ));
        }
        #[cfg(target_family = "wasm")]
        browser::ensure_available()?;
        let (work, receiver) = async_channel::bounded(config.queue_capacity);
        let (finished, stopped) = async_channel::bounded(1);
        #[cfg(not(target_family = "wasm"))]
        std::thread::Builder::new()
            .name("foundation-ui-gpu".into())
            .spawn(move || {
                futures_lite::future::block_on(Self::drive(config, receiver, provider));
                drop(finished);
            })
            .map_err(|e| GpuError::Unavailable(e.to_string()))?;
        #[cfg(target_family = "wasm")]
        wasm_bindgen_futures::spawn_local(async move {
            Self::drive(config, receiver, provider).await;
            drop(finished);
        });
        Ok((*ctx.get_or_insert(Self { work, stopped })).clone())
    }
    fn owner(config: GpuConfig, provider: Arc<dyn DeviceProvider>) -> GpuOwner {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: config.backends,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        GpuOwner {
            instance,
            config,
            provider,
            device: None,
            surfaces: surfaces::Surfaces::default(),
            #[cfg(not(target_family = "wasm"))]
            leased_texture: None,
        }
    }
    async fn drive(config: GpuConfig, receiver: Receiver<Work>, provider: Arc<dyn DeviceProvider>) {
        let mut owner = Self::owner(config, provider);
        while let Ok(job) = receiver.recv().await {
            job(&mut owner).await;
        }
    }

    /// Execute on the graphics owner. Dropping a pending request prevents its job from starting.
    ///
    /// # Errors
    /// Returns the job error or cancellation if the service stopped.
    pub async fn run<T: Send + 'static>(
        &self,
        job: impl for<'a> FnOnce(&'a mut GpuOwner) -> LocalBoxFuture<'a, Result<T, GpuError>>
            + Send
            + 'static,
    ) -> Result<T, GpuError> {
        let (send, receive) = async_channel::bounded(1);
        self.work
            .send(Box::new(move |owner| {
                Box::pin(async move {
                    if !send.is_closed() {
                        let result = job(owner).await;
                        // A caller can drop its result while the submitted work completes.
                        if send.send(result).await.is_err() {}
                    }
                })
            }))
            .await
            .map_err(|_| GpuError::Cancelled)?;
        receive.recv().await.map_err(|_| GpuError::Cancelled)?
    }
    /// Stop accepting work. The owner finishes accepted jobs before it releases the device.
    pub fn close(&self) {
        self.work.close();
    }

    /// Stop accepting work and wait until the owner releases its graphics resources.
    /// Call from the host outside a graphics job.
    pub async fn shutdown(&self) {
        self.close();
        // Closing this signal wakes every service clone after the owner releases its resources.
        let _ = self.stopped.recv().await;
    }
}
