use crate::app::{ui::layer::LayerItem};

pub mod ripple;
pub mod god_rays;

pub mod particles;

// Контекст для эффектов постобработки (Ping-Pong: читают input, пишут в output)
pub struct PostContext<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub encoder: &'a mut wgpu::CommandEncoder,
    pub input_view: &'a wgpu::TextureView,
    pub output_view: &'a wgpu::TextureView,
    pub global_bind_group: &'a wgpu::BindGroup,
}

/// Единый интерфейс для любого пост-процессинг эффекта (Ping-Pong)
pub trait PostEffect: LayerItem {
    /// Выполняет рендер-пасс эффекта из input_view в output_view
    fn render(&self, ctx: &mut PostContext);
}

/// Стандартный пайплайн для эффектов:
/// @group(0) -> входная текстура и сэмплер (t_input, s_input)
/// @group(1) -> глобальные переменные кадра (GlobalUniforms)
/// @group(2) -> локальные параметры эффекта
pub fn create_post_pipeline(
    device: &wgpu::Device,
    label: &str,
    shader_source: &str,
    format: wgpu::TextureFormat,
    global_layout: &wgpu::BindGroupLayout,
) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout, wgpu::BindGroupLayout, wgpu::Sampler) {
    let input_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(&format!("{label} Input Layout")),
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

    let local_uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(&format!("{label} Local Uniform Layout")),
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
        label: Some(&format!("{label} Shader")),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(&format!("{label} Pipeline Layout")),
        bind_group_layouts: &[
            Some(&input_layout),
            Some(global_layout),
            Some(&local_uniform_layout),
        ],
        immediate_size: 0,
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(&format!("{label} Pipeline")),
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
                blend: Some(wgpu::BlendState::REPLACE),
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

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    (pipeline, input_layout, local_uniform_layout, sampler)
}

// Контекст для оверлеев (рисуют поверх target_view с прозрачностью)
pub struct OverlayContext<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub encoder: &'a mut wgpu::CommandEncoder,
    pub target_view: &'a wgpu::TextureView,
    pub global_bind_group: &'a wgpu::BindGroup,
}

pub trait OverlayEffect: LayerItem {
    fn render(&mut self, ctx: &mut OverlayContext);
    fn load_texture(&mut self, _device: &wgpu::Device, _queue: &wgpu::Queue, _bytes: &[u8]) -> anyhow::Result<()>;
}