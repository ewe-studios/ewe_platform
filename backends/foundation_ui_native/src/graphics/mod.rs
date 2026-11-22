//! Keep native drawing targets alive until the graphics owner releases their surfaces.
use foundation_context::Ctx;
use foundation_ui_gpu::{wgpu, GpuError, GpuService, RenderHost, SurfaceId};
use std::sync::Arc;
pub mod handoff;
mod target;
#[cfg(target_os = "android")]
mod vulkan;

#[derive(Debug, thiserror::Error, Clone, uniffi::Error)]
pub enum GraphicsError {
    #[error("The host denied the graphics request")]
    Denied,
    #[error("Graphics service unavailable: {reason}")]
    Unavailable { reason: String },
    #[error("The device does not support the requested features or limits")]
    Unsupported,
    #[error("The graphics device was lost")]
    DeviceLost,
    #[error("The surface has been removed")]
    StaleSurface,
    #[error("The device cannot use this surface")]
    IncompatibleSurface,
    #[error("The surface is suspended")]
    Suspended,
    #[error("The service stopped")]
    Cancelled,
    #[error("Graphics operation failed: {reason}")]
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
            GpuError::IncompatibleSurface => Self::IncompatibleSurface,
            GpuError::Suspended => Self::Suspended,
            GpuError::Cancelled => Self::Cancelled,
        }
    }
}

#[derive(uniffi::Object)]
pub struct NativeGraphics {
    service: GpuService,
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
