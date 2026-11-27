//! Keep native drawing targets alive until the graphics owner releases their surfaces.
use foundation_context::Ctx;
use foundation_ui_gpu::{wgpu, GpuError, GpuService, RenderHost, SurfaceId};
use std::sync::Arc;
pub mod handoff;
mod target;
#[cfg(target_os = "android")]
mod vulkan;

#[derive(Debug, derive_more::Display, derive_more::Error, Clone, uniffi::Error)]
pub enum GraphicsError {
    #[display("The host denied the graphics request")]
    Denied,
    #[display("Graphics service unavailable: {reason}")]
    Unavailable { reason: String },
    #[display("The device does not support the requested features or limits")]
    Unsupported,
    #[display("The graphics device was lost")]
    DeviceLost,
    #[display("The surface has been removed")]
    StaleSurface,
    #[display("The device cannot use this surface")]
    IncompatibleSurface,
    #[display("The surface is suspended")]
    Suspended,
    #[display("The service stopped")]
    Cancelled,
    #[display("Graphics operation failed: {reason}")]
    Operation { reason: String },
}
impl From<GpuError> for GraphicsError {
    fn from(error: GpuError) -> Self {
        match error {
            GpuError::Unavailable(reason) => Self::Unavailable { reason },
            GpuError::Operation(reason) => Self::Operation { reason },
            GpuError::Unsupported => Self::Unsupported,
            GpuError::DeviceLost => Self::DeviceLost,
            GpuError::StaleSurface => Self::StaleSurface,
            GpuError::ExtentUnsupported { .. } | GpuError::IncompatibleSurface => {
                Self::IncompatibleSurface
            }
            GpuError::Suspended => Self::Suspended,
            GpuError::Cancelled => Self::Cancelled,
        }
    }
}

#[derive(uniffi::Object)]
pub struct NativeGraphics {
    service: GpuService,
}

#[cfg(target_os = "android")]
pub(crate) fn configure_android_host(ctx: &Ctx) {
    use foundation_ui_gpu::OwnedInstanceFlags;
    use std::ffi::CStr;

    fn property_is(name: &CStr, expected: &CStr) -> bool {
        let mut value = [0; libc::PROP_VALUE_MAX as usize];
        // SAFETY: Android writes at most PROP_VALUE_MAX bytes, with a final zero.
        let length = unsafe { libc::__system_property_get(name.as_ptr(), value.as_mut_ptr()) };
        // SAFETY: A positive length means Android wrote a zero-terminated value.
        length > 0 && unsafe { CStr::from_ptr(value.as_ptr()) } == expected
    }

    // Ranchu can fail when wgpu names Vulkan objects through VK_EXT_debug_utils.
    // Keep validation and omit that extension for the affected emulator only.
    let flags = OwnedInstanceFlags(wgpu::InstanceFlags::default()).for_android(
        property_is(c"ro.hardware", c"ranchu"),
        property_is(c"ro.boot.qemu", c"1"),
    );
    drop(ctx.get_or_insert(flags));
}

impl NativeGraphics {
    pub(crate) fn from_context(ctx: &Ctx) -> Result<Arc<Self>, GraphicsError> {
        let service = match ctx.get::<GpuService>() {
            Some(installed) => (*installed).clone(),
            None => GpuService::install(ctx, foundation_ui_gpu::GpuConfig::default())?,
        };
        Ok(Arc::new(Self { service }))
    }
}

#[derive(uniffi::Object)]
pub struct NativeDrawingTarget {
    target: target::Target,
    #[cfg(target_os = "android")]
    retired: std::sync::atomic::AtomicBool,
}
#[derive(uniffi::Object)]
pub struct NativeDrawingSurface {
    service: GpuService,
    id: SurfaceId,
}

struct DrawingHost {
    target: Arc<NativeDrawingTarget>,
    width: u32,
    height: u32,
}
impl RenderHost for DrawingHost {
    fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
    #[cfg(target_os = "android")]
    fn is_live(&self) -> bool {
        !self
            .target
            .retired
            .load(std::sync::atomic::Ordering::Acquire)
    }

    fn create_surface(
        &self,
        instance: &wgpu::Instance,
    ) -> Result<wgpu::Surface<'static>, GpuError> {
        self.target.target.surface(instance)
    }
}

#[uniffi::export]
impl NativeGraphics {
    /// Read the backend and adapter name.
    ///
    /// # Errors
    /// Returns the device initialization error.
    pub async fn adapter(&self) -> Result<String, GraphicsError> {
        Ok(self
            .service
            .run(|owner| {
                Box::pin(async move {
                    let info = owner.device().await?.adapter.get_info();
                    Ok(format!("{:?}: {}", info.backend, info.name))
                })
            })
            .await?)
    }

    /// Run a local shader over the supplied words.
    ///
    /// # Errors
    /// Returns a validation, execution, or readback error.
    pub async fn compute(
        &self,
        shader: String,
        words: Vec<u32>,
        groups: u32,
    ) -> Result<Vec<u32>, GraphicsError> {
        Ok(self
            .service
            .run(move |owner| {
                Box::pin(async move { owner.compute_words(&shader, &words, groups).await })
            })
            .await?)
    }

    /// Register a target and keep its native reference until removal.
    ///
    /// # Errors
    /// Returns an error for duplicate names or incompatible devices.
    pub async fn attach(
        &self,
        name: String,
        target: Arc<NativeDrawingTarget>,
        width: u32,
        height: u32,
    ) -> Result<Arc<NativeDrawingSurface>, GraphicsError> {
        let id = self
            .service
            .run(move |owner| {
                Box::pin(async move {
                    owner
                        .attach(
                            name,
                            Box::new(DrawingHost {
                                target,
                                width,
                                height,
                            }),
                        )
                        .await
                })
            })
            .await?;
        Ok(Arc::new(NativeDrawingSurface {
            id,
            service: self.service.clone(),
        }))
    }

    /// Invalidate all old surface handles and open a new device.
    ///
    /// # Errors
    /// Returns the initialization error.
    pub async fn reinitialize(&self) -> Result<(), GraphicsError> {
        Ok(self
            .service
            .run(|owner| Box::pin(async { owner.reinitialize().await.map(|_| ()) }))
            .await?)
    }
}

#[uniffi::export]
impl NativeDrawingSurface {
    /// Change the drawing size in physical pixels. A zero dimension suspends the target.
    ///
    /// # Errors
    /// Returns a stale handle or device error.
    pub async fn resize(&self, width: u32, height: u32) -> Result<(), GraphicsError> {
        let id = self.id.clone();
        Ok(self
            .service
            .run(move |owner| Box::pin(async move { owner.resize(id, width, height).await }))
            .await?)
    }
    /// Fill and present the surface. Each color component ranges from zero to one.
    ///
    /// # Errors
    /// Returns a stale handle, suspended target, or presentation error.
    pub async fn clear(
        &self,
        red: f64,
        green: f64,
        blue: f64,
        alpha: f64,
    ) -> Result<(), GraphicsError> {
        let id = self.id.clone();
        Ok(self
            .service
            .run(move |owner| {
                Box::pin(async move {
                    owner
                        .clear(
                            id,
                            wgpu::Color {
                                r: red,
                                g: green,
                                b: blue,
                                a: alpha,
                            },
                        )
                        .await
                })
            })
            .await?)
    }
    /// Release the target on its graphics owner.
    ///
    /// # Errors
    /// Returns a stale handle error if the target was already removed.
    pub async fn remove(&self) -> Result<(), GraphicsError> {
        let id = self.id.clone();
        Ok(self
            .service
            .run(move |owner| Box::pin(async move { owner.detach(&id) }))
            .await?)
    }
}

impl NativeDrawingSurface {
    /// Draw one frame on the thread that owns the device.
    ///
    /// # Errors
    /// Returns an error if the surface is unavailable or the draw fails.
    pub async fn present(
        &self,
        draw: impl FnOnce(
                &foundation_ui_gpu::GraphicsDevice,
                &wgpu::Texture,
                wgpu::TextureFormat,
            ) -> Result<(), GpuError>
            + Send
            + 'static,
    ) -> Result<(), GpuError> {
        let id = self.id.clone();
        self.service
            .run(move |owner| Box::pin(async move { owner.present(id, draw).await }))
            .await
    }
}
