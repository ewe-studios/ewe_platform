//! Retain an application's Metal device and command queue and import its initialized texture.
use crate::{lease::LeaseError, lease_native::ImportedDevice};
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_metal::{
    MTLCommandBuffer, MTLCommandBufferStatus, MTLCommandQueue, MTLDevice, MTLPixelFormat,
    MTLResource, MTLTexture, MTLTextureType, MTLTextureUsage,
};
use wgpu::hal::{self, api::Metal};

pub struct MetalGrant {
    pub device: Retained<ProtocolObject<dyn MTLDevice>>,
    pub queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    pub texture: Option<Retained<ProtocolObject<dyn MTLTexture>>>,
}
// SAFETY: Metal retains these thread-safe objects. The host transfers exclusive queue and texture use.
unsafe impl Send for MetalGrant {}
impl MetalGrant {
    /// Import references to the same device and queue. Keep prior host submissions ordered.
    ///
    /// # Errors
    /// Rejects foreign queues, unsupported texture formats, and device loss.
    pub async fn import(self) -> Result<ImportedDevice, LeaseError> {
        let Self {
            device: raw_device,
            queue: raw_queue,
            texture,
        } = self;
        if !std::ptr::eq(
            Retained::as_ptr(&raw_queue.device()),
            Retained::as_ptr(&raw_device),
        ) || texture.as_ref().is_some_and(|t| {
            !std::ptr::eq(Retained::as_ptr(&t.device()), Retained::as_ptr(&raw_device))
                || t.pixelFormat() != MTLPixelFormat::RGBA8Unorm
                || t.textureType() != MTLTextureType::Type2D
                || t.arrayLength() != 1
                || t.mipmapLevelCount() != 1
                || t.sampleCount() != 1
                || t.isFramebufferOnly()
                || !(t.usage().is_empty() || t.usage().contains(MTLTextureUsage::ShaderRead))
        }) {
            return Err(LeaseError::Incompatible);
        }
        finish_prior_commands(&raw_queue)?;
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = instance
            .enumerate_adapters(wgpu::Backends::METAL)
            .await
            .into_iter()
            .find(|adapter| {
                // SAFETY: We only compare the adapter's retained native device with the host's object.
                unsafe { adapter.as_hal::<Metal>() }
                    .is_some_and(|a| std::ptr::eq(a.raw_device(), Retained::as_ptr(&raw_device)))
            })
            .ok_or(LeaseError::Incompatible)?;
        let features = wgpu::Features::empty();
        let limits = wgpu::Limits::downlevel_defaults();
        if !limits.check_limits(&adapter.limits()) {
            return Err(LeaseError::Incompatible);
        }
        let descriptor = wgpu::DeviceDescriptor {
            required_features: features,
            required_limits: limits.clone(),
            ..wgpu::DeviceDescriptor::default()
        };
        // SAFETY: Both retained Metal objects belong to this adapter. Timestamp queries are disabled.
        let native = unsafe {
            hal::OpenDevice {
                device: hal::metal::Device::device_from_raw(raw_device, features, &limits),
                queue: hal::metal::Queue::queue_from_raw(raw_queue, 1.0),
            }
        };
        // SAFETY: The adapter identity, enabled features and required limits match the opened device.
        let (device, queue) =
            unsafe { adapter.create_device_from_hal::<Metal>(native, &descriptor) }
                .map_err(|e| LeaseError::Unavailable(e.to_string()))?;
        let texture = texture
            .map(|raw| {
                let size = wgpu::Extent3d {
                    width: u32::try_from(raw.width()).map_err(|_| LeaseError::Incompatible)?,
                    height: u32::try_from(raw.height()).map_err(|_| LeaseError::Incompatible)?,
                    depth_or_array_layers: 1,
                };
                let desc = wgpu::TextureDescriptor {
                    label: Some("leased Metal texture"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                };
                // SAFETY: The descriptor matches the checked Metal texture. The host initialized its contents.
                let wrapper = unsafe {
                    hal::metal::Device::texture_from_raw(
                        raw,
                        desc.format,
                        MTLTextureType::Type2D,
                        1,
                        1,
                        size.into(),
                        None,
                    )
                };
                // SAFETY: Metal uses implicit image layouts. The retained texture came from this precise device.
                Ok::<_, LeaseError>(unsafe {
                    device.create_texture_from_hal::<Metal>(
                        wrapper,
                        &desc,
                        wgpu::TextureUses::RESOURCE,
                    )
                })
            })
            .transpose()?;
        Ok(ImportedDevice {
            instance,
            adapter,
            device,
            queue,
            texture,
        })
    }
}

fn finish_prior_commands(queue: &ProtocolObject<dyn MTLCommandQueue>) -> Result<(), LeaseError> {
    // This command follows all prior work on the queue that the host lends.
    let fence = queue.commandBuffer().ok_or(LeaseError::DeviceLost)?;
    fence.commit();
    fence.waitUntilCompleted();
    if fence.status() == MTLCommandBufferStatus::Completed {
        Ok(())
    } else {
        Err(LeaseError::DeviceLost)
    }
}
