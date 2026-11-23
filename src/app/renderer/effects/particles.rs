use std::{sync::atomic::{AtomicU64, Ordering}, time::Instant};
use wgpu::util::DeviceExt;
use crate::app::ui::layer::{LayerItem, UiContext};

use super::{OverlayContext, OverlayEffect};

const PARTICLE_SHADER: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/shaders/particles.wgsl"));
const MAX_PARTICLES: usize = 3000;

static NEXT_EFFECT_ID: AtomicU64 = AtomicU64::new(1);

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ParticleInstance {
    pos: [f32; 2],
    size: f32,
    rotation: f32,
    alpha: f32,
}

impl ParticleInstance {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ParticleInstance>() as wgpu::BufferAddress,
            // Важно: шаг по инстансам (одна запись = один квадратик), а не по вершинам!
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute { offset: 0,  shader_location: 0, format: wgpu::VertexFormat::Float32x2 },
                wgpu::VertexAttribute { offset: 8,  shader_location: 1, format: wgpu::VertexFormat::Float32 },
                wgpu::VertexAttribute { offset: 12, shader_location: 2, format: wgpu::VertexFormat::Float32 },
                wgpu::VertexAttribute { offset: 16, shader_location: 3, format: wgpu::VertexFormat::Float32 },
            ],
        }
    }
}

struct Particle {
    x: f32,
    y: f32,
    speed_y: f32,
    sway_phase: f32,
    sway_freq: f32,
    size: f32,
    rotation: f32,
    rot_speed: f32,
    alpha: f32,
}

impl Particle {
    fn spawn(seed: u32, random_y: bool) -> Self {
        // Легкий детерминированный рандом без внешних библиотек
        let hash = |n: u32| -> f32 {
            let mut x = n.wrapping_mul(747796405).wrapping_add(2891336453);
            x = ((x >> ((x >> 28).wrapping_add(4))) ^ x).wrapping_mul(277803737);
            ((x >> 22) ^ x) as f32 / u32::MAX as f32
        };

        // depth создает эффект параллакса: ближние частицы крупнее, быстрее и ярче
        let depth = hash(seed * 7 + 1);
        Self {
            x: hash(seed * 7 + 2),
            y: if random_y { hash(seed * 7 + 3) } else { -0.05 },
            speed_y: 0.05 + depth * 0.18,
            sway_phase: hash(seed * 7 + 4) * std::f32::consts::TAU,
            sway_freq: 1.0 + hash(seed * 7 + 5) * 2.0,
            size: 6.0 + depth * 18.0,
            rotation: hash(seed * 7 + 6) * std::f32::consts::TAU,
            rot_speed: (hash(seed * 7 + 7) - 0.5) * 2.5,
            alpha: 0.25 + depth * 0.75,
        }
    }
}

struct SimulationState {
    particles: Vec<Particle>,
    last_frame: Instant,
}

pub struct ParticleEffect {
    id: u64,
    name: String,
    enabled: bool,
    pub count: usize,
    pub fall_speed: f32,
    pub wind: f32,
    pub size_scale: f32,
    pub opacity: f32,

    sim: SimulationState,
    pipeline: wgpu::RenderPipeline,
    texture_layout: wgpu::BindGroupLayout,
    sprite_bind_group: wgpu::BindGroup,
    sampler: wgpu::Sampler,
    instance_buffer: wgpu::Buffer,
}

impl ParticleEffect {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        global_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Particle Texture Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Particle Shader"),
            source: wgpu::ShaderSource::Wgsl(PARTICLE_SHADER.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Particle Pipeline Layout"),
            bind_group_layouts: &[Some(&texture_layout), Some(global_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Particle Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(ParticleInstance::desc())],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    // Включаем смешивание по альфа-каналу спрайта
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

        let sprite_bind_group = Self::create_default_sprite(device, queue, &texture_layout, &sampler);

        let particles: Vec<Particle> = (0..MAX_PARTICLES)
            .map(|i| Particle::spawn(i as u32, true))
            .collect();

        let initial_instances = vec![
            ParticleInstance { pos: [0.0, 0.0], size: 10.0, rotation: 0.0, alpha: 1.0 };
            MAX_PARTICLES
        ];

        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Particle Instance Buffer"),
            contents: bytemuck::cast_slice(&initial_instances),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let id = NEXT_EFFECT_ID.fetch_add(1, Ordering::Relaxed);

        Self {
            id,
            name: format!("Particles #{}", id),
            enabled: true,
            count: 600,
            fall_speed: 1.0,
            wind: 0.05,
            size_scale: 1.0,
            opacity: 0.9,
            sim: SimulationState {
                particles,
                last_frame: Instant::now(),
            },
            pipeline,
            texture_layout,
            sprite_bind_group,
            sampler,
            instance_buffer,
        }
    }

    /// Позволяет заменить текстуру частицы на любую PNG с диска
    pub fn set_sprite_from_bytes(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
    ) -> anyhow::Result<()> {
        let img = image::load_from_memory(bytes)?.to_rgba8();
        let (width, height) = img.dimensions();
        self.sprite_bind_group =
            Self::upload_rgba_texture(device, queue, &self.texture_layout, &self.sampler, &img, width, height);
        Ok(())
    }

    fn create_default_sprite(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        let size = 64u32;
        let mut rgba = vec![0u8; (size * size * 4) as usize];
        let center = size as f32 * 0.5;

        for y in 0..size {
            for x in 0..size {
                let dx = (x as f32 - center) / center;
                let dy = (y as f32 - center) / center;
                let dist = (dx * dx + dy * dy).sqrt();
                let angle = dy.atan2(dx);

                let star = 0.85 + 0.15 * (angle * 6.0).cos();
                let alpha = ((1.0 - dist / star).clamp(0.0, 1.0)).powf(1.8);

                let idx = ((y * size + x) * 4) as usize;
                rgba[idx..idx + 3].fill(255);
                rgba[idx + 3] = (alpha * 255.0) as u8;
            }
        }

        Self::upload_rgba_texture(device, queue, layout, sampler, &rgba, size, size)
    }

    fn upload_rgba_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        rgba: &[u8],
        width: u32,
        height: u32,
    ) -> wgpu::BindGroup {
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Particle Sprite Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            size,
        );

        let view = texture.create_view(&Default::default());
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Particle Sprite Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
            ],
        })
    }
}

impl LayerItem for ParticleEffect {
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
        ctx.texture_picker_button("Change Sprite...", self.id);
        changed |= ctx.ui.add(egui::Slider::new(&mut self.count, 10..=MAX_PARTICLES).text("Count")).changed();
        changed |= ctx.ui.add(egui::Slider::new(&mut self.fall_speed, 0.0..=4.0).text("Fall speed")).changed();
        changed |= ctx.ui.add(egui::Slider::new(&mut self.wind, -0.5..=0.5).text("Wind")).changed();
        changed |= ctx.ui.add(egui::Slider::new(&mut self.size_scale, 0.2..=3.0).text("Particle size")).changed();
        changed |= ctx.ui.add(egui::Slider::new(&mut self.opacity, 0.0..=1.0).text("Opacity")).changed();
        changed
    }
}

impl OverlayEffect for ParticleEffect {
    fn render(&mut self, ctx: &mut OverlayContext) {
        let active_count = self.count.min(MAX_PARTICLES);

        {
            let now = Instant::now();
            let dt = (now - self.sim.last_frame).as_secs_f32().min(0.05);
            self.sim.last_frame = now;

            let mut instances = Vec::with_capacity(active_count);
            for p in self.sim.particles.iter_mut().take(active_count) {
                p.sway_phase += p.sway_freq * dt;
                p.y += p.speed_y * self.fall_speed * dt;
                p.x += (self.wind + p.sway_phase.sin() * 0.03) * dt;
                p.rotation += p.rot_speed * dt;

                if p.y > 1.05 { p.y = -0.05; }
                if p.x > 1.05 { p.x -= 1.1; } else if p.x < -0.05 { p.x += 1.1; }

                instances.push(ParticleInstance {
                    pos: [p.x, p.y],
                    size: p.size * self.size_scale,
                    rotation: p.rotation,
                    alpha: p.alpha * self.opacity,
                });
            }

            ctx.queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));
        }
        let mut pass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Particle Overlay Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: ctx.target_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    // Load — рисуем поверх уже существующего фона без очистки!
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
        ..Default::default()
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.sprite_bind_group, &[]);
        pass.set_bind_group(1, ctx.global_bind_group, &[]);
        pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
        pass.draw(0..4, 0..active_count as u32);
    }

    fn load_texture(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, bytes: &[u8]) -> anyhow::Result<()> {
        self.set_sprite_from_bytes(device, queue, bytes)
    }
}