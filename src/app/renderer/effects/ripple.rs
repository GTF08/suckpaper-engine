use wgpu::util::DeviceExt;

use crate::app::ui::layer::{LayerItem, UiContext};

use super::{create_post_pipeline, PostContext, PostEffect};

const RIPPLE_SHADER: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/shaders/ripple.wgsl"));

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct RippleUniforms {
    strength: f32,
    frequency: f32,
    speed: f32,
    _pad: f32
}

pub struct RippleEffect {
    id: u64,
    enabled: bool,
    pub strength: f32,
    pub frequency: f32,
    pub speed: f32,

    pipeline: wgpu::RenderPipeline,
    input_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
}

impl RippleEffect {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, global_layout: &wgpu::BindGroupLayout) -> Self {
        let (pipeline, input_layout, uniform_layout, sampler) =
            create_post_pipeline(device, "Ripple Effect", RIPPLE_SHADER, format, global_layout);

        let initial = RippleUniforms {
            strength: 0.5,
            frequency: 15.0,
            speed: 2.0,
            _pad: 0.0
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Ripple Uniform Buffer"),
            contents: bytemuck::bytes_of(&initial),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Ripple Uniform Bind Group"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        Self {
            id: 0,
            enabled: false, // По умолчанию выключен, включишь галочкой в UI
            strength: 0.5,
            frequency: 15.0,
            speed: 2.0,
            pipeline,
            input_layout,
            sampler,
            uniform_buffer,
            uniform_bind_group,
        }
    }
}

impl LayerItem for RippleEffect {
    fn id(&self) -> u64 {
        self.id
    }

    fn name(&self) -> &'static str {
        "Water Ripple"
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn draw_ui(&mut self, ctx: &mut UiContext) -> bool {
        let mut changed = false;
        changed |= ctx.ui
            .add(egui::Slider::new(&mut self.strength, 0.0..=2.0).text("Wave strength"))
            .changed();
        changed |= ctx.ui
            .add(egui::Slider::new(&mut self.frequency, 2.0..=40.0).text("Frequency"))
            .changed();
        changed |= ctx.ui
            .add(egui::Slider::new(&mut self.speed, 0.0..=5.0).text("Wave speed"))
            .changed();
        changed
    }
}

impl PostEffect for RippleEffect {
    fn render(&self, ctx: &mut PostContext) {
        let uniforms = RippleUniforms {
            strength: self.strength,
            frequency: self.frequency,
            speed: self.speed,
            _pad: 0.0
        };
        ctx.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        let input_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Ripple Input Bind Group"),
            layout: &self.input_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(ctx.input_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        let mut pass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Ripple Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: ctx.output_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &input_bind_group, &[]);
        pass.set_bind_group(1, ctx.global_bind_group, &[]);
        pass.set_bind_group(2, &self.uniform_bind_group, &[]);
        pass.draw(0..4, 0..1);
    }
}