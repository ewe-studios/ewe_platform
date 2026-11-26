//! Submit a storage-buffer shader and return its owned output.
use crate::{GpuError, GpuOwner};
use wgpu::util::DeviceExt;

impl GpuOwner {
    /// Execute the application's main compute entry with one writable u32 storage buffer at group zero, binding zero.
    ///
    /// # Errors
    /// Returns a limit, validation, device, or readback error.
    pub async fn compute_words(
        &mut self,
        source: &str,
        input: &[u32],
        groups: u32,
    ) -> Result<Vec<u32>, GpuError> {
        #[cfg(not(target_family = "wasm"))]
        let timeout = self.config.readback_timeout;
        let gpu = self.device().await?;
        let size = u64::try_from(input.len())
            .ok()
            .and_then(|n| n.checked_mul(4))
            .ok_or(GpuError::Unsupported)?;
        let limits = gpu.device.limits();
        if size == 0
            || size > limits.max_storage_buffer_binding_size
            || size > limits.max_buffer_size
            || groups == 0
            || groups > limits.max_compute_workgroups_per_dimension
        {
            return Err(GpuError::Unsupported);
        }
        let device = &gpu.device;
        let pipeline = compute_pipeline(device, source).await?;
        let bytes = input
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<_>>();
        let storage = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: &bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        });
        let result = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: storage.as_entire_binding(),
            }],
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups(groups, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&storage, 0, &result, 0, size);
        let submitted = gpu.queue.submit([encoder.finish()]);
        if let Some(e) = validation.pop().await {
            return Err(GpuError::Operation(e.to_string()));
        }
        let (reply, read) = async_channel::bounded(1);
        result
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |mapped| {
                let _caller = reply.try_send(mapped);
            });
        #[cfg(not(target_family = "wasm"))]
        device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submitted),
                timeout: Some(timeout),
            })
            .map_err(|e| GpuError::Operation(e.to_string()))?;
        #[cfg(target_family = "wasm")]
        let _ = submitted;
        read.recv()
            .await
            .map_err(|_| GpuError::Cancelled)?
            .map_err(|e| GpuError::Operation(e.to_string()))?;
        let mapped = result
            .slice(..)
            .get_mapped_range()
            .map_err(|e| GpuError::Operation(e.to_string()))?;
        let words = mapped
            .as_chunks::<4>()
            .0
            .iter()
            .map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
            .collect();
        drop(mapped);
        result.unmap();
        Ok(words)
    }
}

pub(crate) async fn compute_pipeline(
    device: &wgpu::Device,
    source: &str,
) -> Result<wgpu::ComputePipeline, GpuError> {
    let check = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("application compute"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });
    if let Some(e) = check.pop().await {
        return Err(GpuError::Operation(e.to_string()));
    }
    Ok(pipeline)
}
