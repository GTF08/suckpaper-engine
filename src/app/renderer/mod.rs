mod global_uniforms;
mod render_targets;
mod background;
pub mod effects;

use std::{sync::Arc, time::Instant};

use winit::window::Window;

use crate::{app::renderer::{background::BackgroundPass, effects::{OverlayContext, OverlayEffect, PostContext, PostEffect}, global_uniforms::{GlobalUniforms, GlobalUniformsData}, render_targets::RenderTargets}, wallpaper::settings::WallpaperSettings};

// const SHADER_FILE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/shader.wgsl"));
const TEXTURE_BYTES: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/image.png"));

pub struct Renderer {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    global_uniforms: GlobalUniforms,
    background: BackgroundPass,
    targets: RenderTargets,
    start_time: Instant,
}

impl Renderer {
    pub fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();

        // Используем Vulkan, чтобы обойти ограничения DX12 Flip-Swapchain внутри Progman на Win 11
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::DX12,
            flags: Default::default(),
            memory_budget_thresholds: Default::default(),
            backend_options: wgpu::BackendOptions {
                dx12: wgpu::Dx12BackendOptions {
                    presentation_system: wgpu::Dx12SwapchainKind::DxgiFromVisual,
                    ..Default::default()
                },
                ..Default::default()
            },
            display: None,
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .expect("Не найден Vulkan адаптер");

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                required_limits: adapter.limits(),
                ..Default::default()
            },
        ))
        .unwrap();

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);

        let width = size.width.max(1);
        let height = size.height.max(1);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width,
            height: height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto
        };
        surface.configure(&device, &config);

        let background = BackgroundPass::new(&device, &queue, format, width, height, TEXTURE_BYTES)
            .expect("Не удалось инициализировать фоновый рендер");

        let targets = RenderTargets::new(&device, width, height, format);

        let global_uniforms = GlobalUniforms::new(&device, width, height);

        Self {
            window,
            surface,
            device,
            queue,
            config,
            global_uniforms,
            background,
            targets,
            start_time: Instant::now(),
        }
    }

    pub fn global_layout(&self) -> &wgpu::BindGroupLayout { &self.global_uniforms.layout }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    pub fn set_wallpaper_from_file(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        let bytes = std::fs::read(path)?;
        self.background
            .set_image_bytes(&self.device, &self.queue, &bytes)
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    pub fn render(&mut self, overlay_effects: &mut [Box<dyn OverlayEffect>], post_effects: &[Box<dyn PostEffect>]) -> anyhow::Result<bool> {
        self.global_uniforms.update(&self.queue, &GlobalUniformsData {
            time: self.start_time.elapsed().as_secs_f32(),
            delta_time: 0.016,
            resolution: [self.config.width as f32, self.config.height as f32],
            mouse: [0.5, 0.5],
            _pad: [0.0, 0.0],
        });
        
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => {
                return Ok(false);
            },
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Lost => anyhow::bail!("Lost device"),
        };

        let surface_view = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());

        let active_overlays: Vec<&mut dyn OverlayEffect> = overlay_effects
            .iter_mut()
            .filter(|e| e.is_enabled())
            .map(|e| e.as_mut())
            .collect();

        let active_posts: Vec<&dyn PostEffect> = post_effects
            .iter()
            .filter(|e| e.is_enabled())
            .map(|e| e.as_ref())
            .collect();

        let base_target = if active_posts.is_empty() {
            &surface_view
        } else {
            &self.targets.current_view
        };

        // 1. Фон
        self.background.render(&mut encoder, base_target);

        // 2. Оверлеи (частицы, лучи) прямо поверх фона
        for overlay in active_overlays {
            let mut ctx = OverlayContext {
                device: &self.device,
                queue: &self.queue,
                encoder: &mut encoder,
                target_view: base_target,
                global_bind_group: &self.global_uniforms.bind_group,
            };
            overlay.render(&mut ctx);
        }

        // 3. Постобработка (если есть активные)
        if !active_posts.is_empty() {
            let last_idx = active_posts.len() - 1;
            for (i, effect) in post_effects.into_iter().enumerate() {
                let is_last = i == last_idx;
                let output_view = if is_last {
                    &surface_view
                } else {
                    &self.targets.next_view
                };

                let mut ctx = PostContext {
                    device: &self.device,
                    queue: &self.queue,
                    encoder: &mut encoder,
                    input_view: &self.targets.current_view,
                    output_view,
                    global_bind_group: &self.global_uniforms.bind_group,
                };

                effect.render(&mut ctx);

                if !is_last {
                    self.targets.swap();
                }
            }
        }

        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);

        Ok(true)
    }
}