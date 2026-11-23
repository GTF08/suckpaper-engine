use std::sync::atomic::{AtomicU64, Ordering};
use wgpu::util::DeviceExt;

use super::{OverlayContext, OverlayEffect};
use crate::app::ui::layer::{LayerItem, UiContext};

const GOD_RAYS_SHADER: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/shaders/god_rays.wgsl"));

static NEXT_RAY_ID: AtomicU64 = AtomicU64::new(5000);

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct GodRaysUniforms {
    source_pos: [f32; 2], // 0..8
    intensity: f32,       // 8..12
    speed: f32,           // 12..16
    density: f32,         // 16..20
    falloff: f32,         // 20..24
    _pad0: [f32; 2],      // 24..32 (выравнивание vec3 до 16 байт!)
    color: [f32; 3],      // 32..44
    _pad1: f32,           // 44..48 (общий размер 48 байт)
}

pub struct GodRaysEffect {
    id: u64,
    name: String,
    enabled: bool,

    pub source_x: f32,
    pub source_y: f32,
    pub intensity: f32,
    pub speed: f32,
    pub density: f32,
    pub falloff: f32,
    pub color: [f32; 3],

    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
}

impl GodRaysEffect {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        global_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let id = NEXT_RAY_ID.fetch_add(1, Ordering::Relaxed);

        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("GodRays Uniform Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("GodRays Shader"),
            source: wgpu::ShaderSource::Wgsl(GOD_RAYS_SHADER.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("GodRays Pipeline Layout"),
            bind_group_layouts: &[Some(global_layout), Some(&uniform_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("GodRays Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });

        let default_color = [1.0, 0.92, 0.75]; // Теплый солнечный оттенок
        let initial = GodRaysUniforms {
            source_pos: [0.15, -0.05],
            intensity: 0.55,
            speed: 0.8,
            density: 14.0,
            falloff: 1.4,
            _pad0: [0.0; 2],
            color: default_color,
            _pad1: 0.0,
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("GodRays Uniform Buffer"),
            contents: bytemuck::bytes_of(&initial),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("GodRays Uniform Bind Group"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        Self {
            id,
            name: format!("God Rays #{}", id - 4999),
            enabled: true,
            source_x: 0.15,
            source_y: -0.05,
            intensity: 0.55,
            speed: 0.8,
            density: 14.0,
            falloff: 1.4,
            color: default_color,
            pipeline,
            uniform_buffer,
            uniform_bind_group,
        }
    }
}

impl LayerItem for GodRaysEffect {
    fn id(&self) -> u64 {
        self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn draw_ui(&mut self, ctx: &mut UiContext) -> bool {
        let mut changed = false;
        changed |= ctx.ui.add(egui::Slider::new(&mut self.source_x, -0.5..=1.5).text("Sun X")).changed();
        changed |= ctx.ui.add(egui::Slider::new(&mut self.source_y, -0.5..=1.5).text("Sun Y")).changed();
        changed |= ctx.ui.add(egui::Slider::new(&mut self.intensity, 0.0..=1.5).text("Intensity")).changed();
        changed |= ctx.ui.add(egui::Slider::new(&mut self.speed, 0.0..=3.0).text("Shimmer speed")).changed();
        changed |= ctx.ui.add(egui::Slider::new(&mut self.density, 3.0..=40.0).text("Ray density")).changed();
        changed |= ctx.ui.add(egui::Slider::new(&mut self.falloff, 0.2..=4.0).text("Falloff")).changed();

        ctx.ui.horizontal(|ui| {
            ui.label("Light color:");
            changed |= ui.color_edit_button_rgb(&mut self.color).changed();
        });

        changed
    }
}

impl OverlayEffect for GodRaysEffect {
    fn render(&mut self, ctx: &mut OverlayContext) {
        let uniforms = GodRaysUniforms {
            source_pos: [self.source_x, self.source_y],
            intensity: self.intensity,
            speed: self.speed,
            density: self.density,
            falloff: self.falloff,
            _pad0: [0.0; 2],
            color: self.color,
            _pad1: 0.0,
        };
        ctx.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        let mut pass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("GodRays Overlay Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: ctx.target_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, ctx.global_bind_group, &[]);
        pass.set_bind_group(1, &self.uniform_bind_group, &[]);
        pass.draw(0..4, 0..1);
    }
    
    fn load_texture(&mut self, _device: &wgpu::Device, _queue: &wgpu::Queue, _bytes: &[u8]) -> anyhow::Result<()> {
        Ok(())
    }
}
