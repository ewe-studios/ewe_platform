//! Execute leased work on the original native device and queue.
use crate::{
    compute::compute_pipeline,
    lease::{Identity, LeaseError, LeasedGraphics},
    GpuConfig, GpuError, GpuOwner, GpuService, GraphicsDevice, SystemDevice,
};
use futures_util::future::BoxFuture;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// These wgpu objects refer to one external device and its optional initialized texture.
pub struct ImportedDevice {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub texture: Option<wgpu::Texture>,
}

/// The callback runs after the final fence and resource release.
pub struct NativeLease {
    identity: Identity,
    service: GpuService,
    withdrawn: Arc<AtomicBool>,
    lost: Arc<AtomicBool>,
}
impl NativeLease {
    /// Transfer exclusive queue access to the owner. The host must finish its prior work first.
    ///
    /// # Errors
    /// Returns an incompatibility or thread-start error.
    pub fn import(
        identity: Identity,
        parts: ImportedDevice,
        config: GpuConfig,
        released: impl FnOnce(Result<(), LeaseError>) + Send + 'static,
    ) -> Result<Arc<Self>, LeaseError> {
        if identity.generation == 0
            || config.queue_capacity == 0
            || identity.selection.surface.is_some() != parts.texture.is_some()
        {
            return Err(LeaseError::Incompatible);
        }
        if let Some(texture) = &parts.texture {
            if texture.format() != wgpu::TextureFormat::Rgba8Unorm
                || texture.dimension() != wgpu::TextureDimension::D2
                || texture.depth_or_array_layers() != 1
                || texture.sample_count() != 1
                || !texture
                    .usage()
                    .contains(wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST)
            {
                return Err(LeaseError::Incompatible);
            }
        }
        let (work, receiver) = async_channel::bounded::<crate::Work>(config.queue_capacity);
        let (finished, stopped_signal) = async_channel::bounded(1);
        let texture = parts.texture;
        let withdrawn = Arc::new(AtomicBool::new(false));
        let stopped = withdrawn.clone();

        let sender = work.downgrade();
        let graphics =
            GraphicsDevice::from_parts(parts.adapter, parts.device, parts.queue, move || {
                if let Some(queue) = sender.upgrade() {
                    queue.close();
                }
            });
        let lost = graphics.lost.clone();
        let completion_loss = lost.clone();
        std::thread::Builder::new()
            .name("foundation-ui-device-lease".into())
            .spawn(move || {
                let device = graphics.device.clone();
                let mut owner = GpuOwner {
                    instance: parts.instance,
                    config,
                    provider: Arc::new(SystemDevice),
                    device: Some(Ok(graphics)),
                    surfaces: crate::surfaces::Surfaces::default(),
                    leased_texture: texture,
                };
                futures_lite::future::block_on(async {
                    while let Ok(job) = receiver.recv().await {
                        job(&mut owner).await;
                    }
                });
                // A timeout cannot authorize the host to reuse a busy external resource.
                let completion = device
                    .poll(wgpu::PollType::wait_indefinitely())
                    .map(|_| ())
                    .map_err(|_| LeaseError::DeviceLost);
                stopped.store(true, Ordering::Release);

                let completion = if completion_loss.load(Ordering::Acquire) {
                    Err(LeaseError::DeviceLost)
                } else {
                    completion
                };
                drop(owner);
                drop(device);
                released(completion);
                drop(finished);
            })
            .map_err(|e| LeaseError::Unavailable(e.to_string()))?;
        Ok(Arc::new(Self {
            identity,
            service: GpuService {
                work,
                stopped: stopped_signal,
            },
            withdrawn,
            lost,
        }))
    }
    /// Reject new work. The completion callback still waits for accepted device commands.
    pub fn withdraw(&self) {
        self.withdrawn.store(true, Ordering::Release);
        self.service.close();
    }
}

impl LeasedGraphics for NativeLease {
    fn identity(&self) -> &Identity {
        &self.identity
    }
    fn compute(
        &self,
        shader: String,
        words: Vec<u32>,
        groups: u32,
    ) -> BoxFuture<'_, Result<Vec<u32>, LeaseError>> {
        let stopped = self.withdrawn.clone();
        Box::pin(async move {
            if self.lost.load(Ordering::Acquire) {
                return Err(LeaseError::DeviceLost);
            }
            if stopped.load(Ordering::Acquire) {
                return Err(LeaseError::Stale);
            }
            self.service
                .run(move |owner| {
                    Box::pin(async move {
                        if stopped.load(Ordering::Acquire) {
                            return Err(GpuError::StaleSurface);
                        }
                        owner.compute_words(&shader, &words, groups).await
                    })
                })
                .await
                .map_err(|error| {
                    if self.lost.load(Ordering::Acquire) {
                        LeaseError::DeviceLost
                    } else {
                        error.into()
                    }
                })
        })
    }
    fn process_texture(
        &self,
        shader: String,
        groups: [u32; 3],
    ) -> BoxFuture<'_, Result<(), LeaseError>> {
        let stopped = self.withdrawn.clone();

        Box::pin(async move {
            if self.lost.load(Ordering::Acquire) {
                return Err(LeaseError::DeviceLost);
            }
            if stopped.load(Ordering::Acquire) {
                return Err(LeaseError::Stale);
            }
            self.service
                .run(move |owner| {
                    Box::pin(async move {
                        if stopped.load(Ordering::Acquire) {
                            return Err(GpuError::StaleSurface);
                        }
                        let texture = owner
                            .leased_texture
                            .clone()
                            .ok_or(GpuError::IncompatibleSurface)?;
                        process(owner.device().await?, &texture, &shader, groups).await
                    })
                })
                .await
                .map_err(|error| {
                    if self.lost.load(Ordering::Acquire) {
                        LeaseError::DeviceLost
                    } else {
                        error.into()
                    }
                })
        })
    }
    fn release(&self) {
        self.withdraw();
    }
}
impl From<GpuError> for LeaseError {
    fn from(error: GpuError) -> Self {
        match error {
            GpuError::Cancelled => Self::Cancelled,
            GpuError::DeviceLost => Self::DeviceLost,
            GpuError::StaleSurface => Self::Stale,
            GpuError::IncompatibleSurface | GpuError::Unsupported | GpuError::Suspended => {
                Self::Incompatible
            }
            GpuError::Unavailable(reason) | GpuError::Operation(reason) => {
                Self::Unavailable(reason)
            }
        }
    }
}

async fn process(
    gpu: &GraphicsDevice,
    original: &wgpu::Texture,
    shader: &str,
    groups: [u32; 3],
) -> Result<(), GpuError> {
    let device = &gpu.device;
    if groups
        .into_iter()
        .any(|n| n == 0 || n > device.limits().max_compute_workgroups_per_dimension)
    {
        return Err(GpuError::Unsupported);
    }
    let pipeline = compute_pipeline(device, shader).await?;
    let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let size = original.size();
    let result = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("leased texture result"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let src = original.create_view(&wgpu::TextureViewDescriptor::default());
    let dst = result.create_view(&wgpu::TextureViewDescriptor::default());
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&src),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&dst),
            },
        ],
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(groups[0], groups[1], groups[2]);
    }
    encoder.copy_texture_to_texture(result.as_image_copy(), original.as_image_copy(), size);
    encoder.transition_resources(
        std::iter::empty(),
        std::iter::once(wgpu::TextureTransition {
            texture: original,
            selector: None,
            state: wgpu::TextureUses::RESOURCE,
        }),
    );
    gpu.queue.submit([encoder.finish()]);
    if let Some(error) = validation.pop().await {
        return Err(GpuError::Operation(error.to_string()));
    }
    Ok(())
}
