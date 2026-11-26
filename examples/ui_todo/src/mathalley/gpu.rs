//! Draw the game into a texture supplied by the host.
#![expect(
    clippy::cast_precision_loss,
    reason = "The vertex shader uses floating-point drawable dimensions."
)]
use super::game::{DESIGN_H, DESIGN_W};
use super::scene::{self, Instance, Scene};
use foundation_ui_gpu::{wgpu, GraphicsDevice};
const SHADER: &str = r"
struct Screen {
    scale: vec2<f32>,
    offset: vec2<f32>,
    srgb: f32,
    _pad: f32,
    _pad2: vec2<f32>,
}

@group(0) @binding(0) var<uniform> screen: Screen;

struct Instance {
    @location(0) center: vec2<f32>,
    @location(1) half_size: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) radius: f32,
    @location(4) angle: f32,
}

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) half_size: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) radius: f32,
}

@vertex
fn vs_main(@builtin(vertex_index) index: u32, instance: Instance) -> VertexOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
    );

    let local = corners[index] * instance.half_size;
    let c = cos(instance.angle);
    let s = sin(instance.angle);
    let rotated = vec2<f32>(
        local.x * c - local.y * s,
        local.x * s + local.y * c,
    );
    let design = instance.center + rotated;

    var out: VertexOut;
    out.position = vec4<f32>(design * screen.scale + screen.offset, 0.0, 1.0);
    out.local = rotated;
    out.half_size = instance.half_size;
    out.color = instance.color;
    out.radius = instance.radius;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    // Signed distance to a rounded box; radius == 0 is a sharp rectangle.
    let inner = max(in.half_size - vec2<f32>(in.radius), vec2<f32>(0.0));
    let q = abs(in.local) - inner;
    let d = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - in.radius;

    var alpha = 1.0;
    if (in.radius > 0.0) {
        alpha = 1.0 - smoothstep(-1.0, 1.0, d);
    } else {
        alpha = 1.0 - step(0.0, d);
    }

    // An sRGB target encodes linear values for us. WebGL2 hands back a plain
    // RGBA8 canvas, so encode here to keep both backends looking the same.
    let encoded = select(
        vec3<f32>(
            12.92 * in.color.rgb.x,
            12.92 * in.color.rgb.y,
            12.92 * in.color.rgb.z,
        ),
        vec3<f32>(
            1.055 * pow(max(in.color.rgb.x, 0.0), 1.0 / 2.4) - 0.055,
            1.055 * pow(max(in.color.rgb.y, 0.0), 1.0 / 2.4) - 0.055,
            1.055 * pow(max(in.color.rgb.z, 0.0), 1.0 / 2.4) - 0.055,
        ),
        in.color.rgb >= vec3<f32>(0.0031308),
    );
    let rgb = select(encoded, in.color.rgb, screen.srgb > 0.5);
    return vec4<f32>(rgb, in.color.a * alpha);
}
";

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Screen {
    scale: [f32; 2],
    offset: [f32; 2],
    srgb: f32,
    _pad: f32,
    _pad2: [f32; 2],
}

/// Maps design coordinates into clip space, preserving aspect ratio.
fn screen_transform(width: f32, height: f32, srgb: bool) -> Screen {
    let scale = (width / DESIGN_W).min(height / DESIGN_H);
    let offset_x = (width - DESIGN_W * scale) / 2.0;
    let offset_y = (height - DESIGN_H * scale) / 2.0;
    Screen {
        scale: [2.0 * scale / width, -2.0 * scale / height],
        offset: [2.0 * offset_x / width - 1.0, 1.0 - 2.0 * offset_y / height],
        srgb: if srgb { 1.0 } else { 0.0 },
        _pad: 0.0,
        _pad2: [0.0; 2],
    }
}

pub struct Renderer {
    format: wgpu::TextureFormat,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    screen_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
}

impl Renderer {
    #[must_use]
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, capacity: usize) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mathalley-shapes"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mathalley-screen-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let screen_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mathalley-screen"),
            size: std::mem::size_of::<Screen>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mathalley-screen"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: screen_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mathalley-layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mathalley-shapes"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        vertex_attribute(0, 0, wgpu::VertexFormat::Float32x2),
                        vertex_attribute(1, 8, wgpu::VertexFormat::Float32x2),
                        vertex_attribute(2, 16, wgpu::VertexFormat::Float32x4),
                        vertex_attribute(3, 32, wgpu::VertexFormat::Float32),
                        vertex_attribute(4, 36, wgpu::VertexFormat::Float32),
                    ],
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let instance_buffer = create_instance_buffer(device, capacity);

        Self {
            format,
            pipeline,
            bind_group,
            screen_buffer,
            instance_buffer,
            instance_capacity: capacity,
        }
    }

    /// # Errors
    /// Returns an error when the draw exceeds the device instance range.
    pub fn render(
        &mut self,
        gpu: &GraphicsDevice,
        texture: &wgpu::Texture,
        scene: &Scene,
    ) -> Result<(), foundation_ui_gpu::GpuError> {
        let count = u32::try_from(scene.instances.len()).map_err(|_| {
            foundation_ui_gpu::GpuError::Operation("Too many scene instances".into())
        })?;
        let (width, height) = (texture.width(), texture.height());
        let screen = screen_transform(width as f32, height as f32, self.format.is_srgb());
        gpu.queue
            .write_buffer(&self.screen_buffer, 0, bytemuck::bytes_of(&screen));
        self.upload(gpu, scene);

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("mathalley-encoder"),
            });

        {
            let mut background = scene::srgb(scene::BG);
            if !self.format.is_srgb() {
                for channel in &mut background[..3] {
                    *channel = encode_srgb(*channel);
                }
            }
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mathalley-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: f64::from(background[0]),
                            g: f64::from(background[1]),
                            b: f64::from(background[2]),
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
            pass.draw(0..6, 0..count);
        }

        gpu.queue.submit([encoder.finish()]);
        Ok(())
    }

    fn upload(&mut self, gpu: &GraphicsDevice, scene: &Scene) {
        if scene.instances.is_empty() {
            return;
        }
        if scene.instances.len() > self.instance_capacity {
            let capacity = scene.instances.len().next_power_of_two();
            self.instance_buffer = create_instance_buffer(&gpu.device, capacity);
            self.instance_capacity = capacity;
        }
        gpu.queue.write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(&scene.instances),
        );
    }
}

fn encode_srgb(value: f32) -> f32 {
    if value <= 0.003_130_8 {
        12.92 * value
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

fn vertex_attribute(
    shader_location: u32,
    offset: u64,
    format: wgpu::VertexFormat,
) -> wgpu::VertexAttribute {
    wgpu::VertexAttribute {
        shader_location,
        offset,
        format,
    }
}

fn create_instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("mathalley-instances"),
        size: (capacity * std::mem::size_of::<Instance>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
