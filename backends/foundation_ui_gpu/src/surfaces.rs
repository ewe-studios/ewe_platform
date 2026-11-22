//! Keep native drawing targets alive until their graphics surfaces have been released.
use crate::{GpuError, GpuOwner, GraphicsDevice};
use std::collections::BTreeMap;

/// The host supplies a retained canvas, window, or layer and its physical size.
pub trait RenderHost {
    ///
    /// # Errors
    /// Returns an error if the backend cannot create a surface for this target.
    fn create_surface(&self, instance: &wgpu::Instance)
        -> Result<wgpu::Surface<'static>, GpuError>;
    fn size(&self) -> (u32, u32);
}

struct Registered {
    surface: wgpu::Surface<'static>,
    _host: Box<dyn RenderHost>,
    config: wgpu::SurfaceConfiguration,
    suspended: bool,
}
#[derive(Default)]
pub(super) struct Surfaces {
    identity: std::sync::Arc<()>,
    names: BTreeMap<String, u64>,
    next: u64,
    slots: BTreeMap<u64, Registered>,
}
impl Surfaces {
    pub fn clear(&mut self) {
        self.slots.clear();
        self.names.clear();
    }
}

/// A handle belongs to one graphics service. Its identifier is never reused by that service.
#[derive(Debug, Clone)]
pub struct SurfaceId(u64, std::sync::Weak<()>);

impl GpuOwner {
    #[must_use]
    pub fn named_surface(&self, name: &str) -> Option<SurfaceId> {
        self.surfaces
            .names
            .get(name)
            .map(|id| SurfaceId(*id, std::sync::Arc::downgrade(&self.surfaces.identity)))
    }
    fn check_owner(&self, id: &SurfaceId) -> Result<(), GpuError> {
        if id
            .1
            .ptr_eq(&std::sync::Arc::downgrade(&self.surfaces.identity))
        {
            Ok(())
        } else {
            Err(GpuError::StaleSurface)
        }
    }
    ///
    /// # Errors
    /// Returns an error for duplicate names, incompatible targets, or device loss.
    pub async fn attach(
        &mut self,
        name: String,
        host: Box<dyn RenderHost>,
    ) -> Result<SurfaceId, GpuError> {
        if self.surfaces.names.contains_key(&name) {
            return Err(GpuError::Operation(
                "Surface name already registered".into(),
            ));
        }
        let surface = host.create_surface(&self.instance)?;
        let (width, height) = host.size();
        let graphics = self.device().await?;
        if !graphics.adapter.is_surface_supported(&surface) {
            return Err(GpuError::IncompatibleSurface);
        }
        let config = surface
            .get_default_config(&graphics.adapter, width.max(1), height.max(1))
            .ok_or(GpuError::IncompatibleSurface)?;
        let suspended = width == 0 || height == 0;
        if !suspended {
            surface.configure(&graphics.device, &config);
        }
        let id = self
            .surfaces
            .next
            .checked_add(1)
            .ok_or(GpuError::StaleSurface)?;
        self.surfaces.next = id;
        self.surfaces.slots.insert(
            id,
            Registered {
                surface,
                _host: host,
                config,
                suspended,
            },
        );
        self.surfaces.names.insert(name, id);
        Ok(SurfaceId(
            id,
            std::sync::Arc::downgrade(&self.surfaces.identity),
        ))
    }

    ///
    /// # Errors
    /// Returns a stale handle error if this owner has already released the target.
    pub fn detach(&mut self, id: &SurfaceId) -> Result<(), GpuError> {
        self.check_owner(id)?;
        self.surfaces.names.retain(|_, v| *v != id.0);
        self.surfaces
            .slots
            .remove(&id.0)
            .map(drop)
            .ok_or(GpuError::StaleSurface)
    }

    ///
    /// # Errors
    /// Returns an error if the device or surface is unavailable.
    pub async fn resize(&mut self, id: SurfaceId, width: u32, height: u32) -> Result<(), GpuError> {
        self.check_owner(&id)?;
        let device = self.device().await?.device.clone();
        let slot = self
            .surfaces
            .slots
            .get_mut(&id.0)
            .ok_or(GpuError::StaleSurface)?;
        slot.suspended = width == 0 || height == 0;
        if !slot.suspended {
            slot.config.width = width;
            slot.config.height = height;
            slot.surface.configure(&device, &slot.config);
        }
        Ok(())
    }

    ///
    /// # Errors
    /// Returns the presentation error or the error from the drawing function.
    pub async fn present<T>(
        &mut self,
        id: SurfaceId,
        draw: impl FnOnce(&GraphicsDevice, &wgpu::Texture, wgpu::TextureFormat) -> Result<T, GpuError>,
    ) -> Result<T, GpuError> {
        self.check_owner(&id)?;
        self.device().await?;
        let slot = self
            .surfaces
            .slots
            .get_mut(&id.0)
            .ok_or(GpuError::StaleSurface)?;
        if slot.suspended {
            return Err(GpuError::Suspended);
        }
        let graphics = self
            .device
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .ok_or(GpuError::DeviceLost)?;
        let frame = match slot.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f)
            | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Occluded | wgpu::CurrentSurfaceTexture::Timeout => {
                return Err(GpuError::Suspended)
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                slot.surface.configure(&graphics.device, &slot.config);
                return Err(GpuError::Suspended);
            }
            wgpu::CurrentSurfaceTexture::Validation => return Err(GpuError::IncompatibleSurface),
        };
        let result = draw(graphics, &frame.texture, slot.config.format)?;
        graphics.queue.present(frame);
        Ok(result)
    }
}

impl GpuOwner {
    /// Fill the target through a render pass on the local device.
    ///
    /// # Errors
    /// Returns a presentation error if the device or surface is unavailable.
    pub async fn clear(&mut self, id: SurfaceId, color: wgpu::Color) -> Result<(), GpuError> {
        self.present(id, |gpu, texture, _| {
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let mut encoder = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
            {
                let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(color),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                });
            }
            gpu.queue.submit([encoder.finish()]);
            Ok(())
        })
        .await
    }
}
