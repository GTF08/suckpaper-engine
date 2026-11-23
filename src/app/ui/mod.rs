pub mod layer;
mod panel;

use std::sync::{Arc};

use egui_wgpu::{RendererOptions, ScreenDescriptor};
use winit::event_loop::EventLoopProxy;
use winit::{
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::ActiveEventLoop,
    window::{Window, WindowId},
};

use crate::app::UserEvent;
use crate::app::renderer::effects::{OverlayEffect, PostEffect};
use crate::app::ui::panel::SettingsPanel;
use crate::wallpaper::settings::WallpaperSettings;


/// События, которые окно настроек отправляет приложению
#[derive(Debug, Clone)]
pub enum UiAction {
    Close,
    SettingsChanged,
}

pub struct SettingsWindow {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
    panel: SettingsPanel
    // proxy: EventLoopProxy<UserEvent>,
    // is_picking_file: Arc<AtomicBool>,
    // selected_effect_id: Option<u64>,
}

impl SettingsWindow {
    pub fn new(event_loop: &ActiveEventLoop, proxy: EventLoopProxy<UserEvent>) -> Self {
        let attrs = Window::default_attributes()
            .with_title("Wallpaper Settings")
            .with_inner_size(LogicalSize::new(620.0, 420.0))
            .with_min_inner_size(LogicalSize::new(320.0, 240.0));

        let window = Arc::new(event_loop.create_window(attrs).unwrap());
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: Default::default(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            display: None,
        });

        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .unwrap();

        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .find(|f| !f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);

        // Отключаем VSync для окна настроек, чтобы оно не тормозило основной рендер
        let present_mode = caps
            .present_modes
            .iter()
            .copied()
            .find(|m| matches!(m, wgpu::PresentMode::Mailbox | wgpu::PresentMode::Immediate))
            .unwrap_or(wgpu::PresentMode::Fifo);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &config);

        let egui_ctx = Self::create_egui_context();
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        let egui_renderer = egui_wgpu::Renderer::new(&device, format, RendererOptions::default());

        window.request_redraw();

        Self {
            window,
            surface,
            device,
            queue,
            config,
            egui_ctx,
            egui_state,
            egui_renderer,
            panel: SettingsPanel::new(proxy)
            // proxy,
            // is_picking_file: Arc::new(AtomicBool::new(false)),
            // selected_effect_id: None
        }
    }

    pub fn window_id(&self) -> WindowId {
        self.window.id()
    }

    /// Единая точка входа для всех событий окна winit
    pub fn handle_window_event(
        &mut self,
        event: &WindowEvent,
        settings: &mut WallpaperSettings,
        overlay_effects: &mut [Box<dyn OverlayEffect>],
        post_effects: &mut [Box<dyn PostEffect>]
    ) -> Option<UiAction> {
        let _ = self.egui_state.on_window_event(&self.window, event);

        match event {
            WindowEvent::CloseRequested => Some(UiAction::Close),
            WindowEvent::Resized(size) => {
                if size.width > 0 && size.height > 0 {
                    self.config.width = size.width;
                    self.config.height = size.height;
                    self.surface.configure(&self.device, &self.config);
                    self.window.request_redraw();
                }
                None
            }
            WindowEvent::RedrawRequested => {
                if self.render(settings, overlay_effects, post_effects) {
                    Some(UiAction::SettingsChanged)
                } else {
                    None
                }
            }
            _ => {
                self.window.request_redraw();
                None
            }
        }
    }

    fn create_egui_context() -> egui::Context {
        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();

        if let Ok(font_data) = std::fs::read("C:\\Windows\\Fonts\\segoeui.ttf") {
            fonts.font_data.insert(
                "segoe_ui".to_owned(),
                Arc::new(egui::FontData::from_owned(font_data)),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "segoe_ui".to_owned());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("segoe_ui".to_owned());
            ctx.set_fonts(fonts);
        }
        ctx
    }

    fn render(
        &mut self, 
        settings: &mut WallpaperSettings, 
        overlay_effects: &mut [Box<dyn OverlayEffect>], 
        post_effects: &mut [Box<dyn PostEffect>]
    ) -> bool {
        let mut settings_changed = false;

        let raw_input = self.egui_state.take_egui_input(&self.window);

        let full_output = self.egui_ctx.run_ui(raw_input, |ui| {
            settings_changed |= self.panel.show(ui, settings, overlay_effects, post_effects);
            // egui::CentralPanel::default().show(ui, |ui| {
            //     ui.heading("Live Wallpaper Settings");
            //     ui.separator();

            //     let btn_text = if is_picking { "Selecting..." } else { "Select image..." };
            //     if ui.add_enabled(!is_picking, egui::Button::new(btn_text)).clicked() {
            //         open_dialog = true;
            //     }

            //     if let Some(path) = &settings.image_path {
            //         ui.label(format!(
            //             "File: {}",
            //             path.file_name().unwrap_or_default().to_string_lossy()
            //         ));
            //     }

            //     ui.add_space(10.0);
            //     egui::ScrollArea::vertical().show(ui, |ui| {
            //         for overlay_eff in overlay_effects.iter_mut() {
            //             let mut enabled = overlay_eff.is_enabled();
            //             if ui.checkbox(&mut enabled, overlay_eff.name()).changed() {
            //                 overlay_eff.set_enabled(enabled);
            //                 settings_changed = true;
            //             }

            //             if enabled {
            //                 ui.indent(overlay_eff.name(), |ui| {
            //                     let mut ctx = UiContext {
            //                         ui,
            //                         proxy: &self.proxy,
            //                         is_picking_file: &self.is_picking_file,
            //                     };
            //                     settings_changed |= overlay_eff.draw_ui(&mut ctx);
            //                 });
            //             }
            //             ui.add_space(6.0);
            //         }

            //         for post_eff in post_effects.iter_mut() {
            //             let mut enabled = post_eff.is_enabled();
            //             if ui.checkbox(&mut enabled, post_eff.name()).changed() {
            //                 post_eff.set_enabled(enabled);
            //                 settings_changed = true;
            //             }
            //             if enabled {
            //                 let mut ctx = UiContext {
            //                     ui,
            //                     proxy: &self.proxy,
            //                     is_picking_file: &self.is_picking_file,
            //                 };
            //                 ctx.ui.indent(post_eff.name(), |ui| {
            //                     settings_changed |= post_eff.draw_ui(&mut ctx);
            //                 });
            //             }
            //             ui.add_space(6.0);
            //         }
            //     });
            // });
        });

        // if open_dialog {
        //     self.open_file_dialog();
        // }

        self.egui_state
            .handle_platform_output(&self.window, full_output.platform_output);

        let mut textures_delta = full_output.textures_delta;
        for (id, deltas) in &textures_delta.set {
            for delta in deltas {
                self.egui_renderer
                    .update_texture(&self.device, &self.queue, *id, delta);
            }
        }

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => Some(t),
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                None
            }
            _ => None,
        };

        if let Some(frame) = frame {
            let view = frame.texture.create_view(&Default::default());
            let mut encoder = self.device.create_command_encoder(&Default::default());

            let paint_jobs = self
                .egui_ctx
                .tessellate(full_output.shapes, full_output.pixels_per_point);
            let screen_desc = ScreenDescriptor {
                size_in_pixels: [self.config.width, self.config.height],
                pixels_per_point: full_output.pixels_per_point,
            };

            self.egui_renderer.update_buffers(
                &self.device,
                &self.queue,
                &mut encoder,
                &paint_jobs,
                &screen_desc,
            );

            {
                let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 30.0 / 255.0,
                                g: 30.0 / 255.0,
                                b: 30.0 / 255.0,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                });
                self.egui_renderer
                    .render(&mut pass.forget_lifetime(), &paint_jobs, &screen_desc);
            }
            self.queue.submit(Some(encoder.finish()));
            self.queue.present(frame);
        }

        for id in &textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        textures_delta.clear();
        settings_changed
    }
}