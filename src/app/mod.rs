pub mod shader_structs;
pub mod windows;
mod renderer;
mod tray;
mod ui;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowId};

use crate::app::renderer::Renderer;
use crate::app::renderer::effects::god_rays::GodRaysEffect;
use crate::app::renderer::effects::particles::ParticleEffect;
use crate::app::renderer::effects::{OverlayEffect, PostEffect};
use crate::app::renderer::effects::ripple::RippleEffect;
use crate::app::tray::{AppTray, TrayAction};
use crate::app::ui::{SettingsWindow, UiAction};
use crate::wallpaper::settings::WallpaperSettings;

use windows::attach_to_desktop;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayKind {
    Particles,
    GodRays,
}

#[derive(Debug, Clone)]
pub enum UserEvent {
    Tray(TrayAction),
    BackgroundChanged(PathBuf),
    EffectTextureChanged { effect_id: u64, path: PathBuf },
    AddOverlay(OverlayKind),
    RemoveOverlay(usize),
    ReorderOverlay { from: usize, to: usize },
    ReorderPostEffect { from: usize, to: usize },
}


pub struct App {
    proxy: EventLoopProxy<UserEvent>,
    renderer: Option<Renderer>,
    tray: Option<AppTray>,
    settings_window: Option<SettingsWindow>,
    settings: WallpaperSettings,
    post_effects: Vec<Box<dyn PostEffect>>,
    overlay_effects: Vec<Box<dyn OverlayEffect>>,
    is_paused: bool,
    next_frame: Instant,
}

impl App {
    pub fn run() -> anyhow::Result<()> {
        let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
        event_loop.set_control_flow(ControlFlow::Poll);

        let mut app = Self {
            proxy: event_loop.create_proxy(),
            renderer: None,
            tray: None,
            settings_window: None,
            settings: WallpaperSettings::default(),
            post_effects: Vec::new(),
            overlay_effects: Vec::new(),
            is_paused: false,
            next_frame: Instant::now()
        };
        event_loop.run_app(&mut app)?;
        Ok(())
    }

    fn handle_ui_action(&mut self, action: UiAction) {
        match action {
            UiAction::Close => {
                self.settings_window = None;
            }
            UiAction::SettingsChanged => {
                // Сюда позже легко добавится сохранение конфига на диск
            }
        }
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.tray.is_none() {
            self.tray = Some(AppTray::new(self.proxy.clone()));
        }

        if self.renderer.is_some() {
            return;
        }

        let mut attrs = Window::default_attributes()
            .with_visible(false)
            .with_decorations(false)
            .with_resizable(false);

        if let Some(monitor) = event_loop.primary_monitor() {
            attrs = attrs
                .with_position(monitor.position())
                .with_inner_size(monitor.size());
        }

        let window = Arc::new(event_loop.create_window(attrs).unwrap());
        let renderer = Renderer::new(window.clone());

        self.post_effects = vec![
            Box::new(RippleEffect::new(renderer.device(), renderer.format(), renderer.global_layout()))
        ];

        self.overlay_effects = vec![
            Box::new(ParticleEffect::new(
                renderer.device(),
                renderer.queue(),
                renderer.format(),
                renderer.global_layout()
            ))
        ];

        attach_to_desktop(&window);
        window.set_visible(true);

        self.renderer = Some(renderer);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        // 1. Окно настроек
        if let Some(ui_win) = &mut self.settings_window {
            if ui_win.window_id() == id {
                
                if let Some(action) = ui_win.handle_window_event(
                    &event, 
                    &mut self.settings, 
                    &mut self.overlay_effects,
                    &mut self.post_effects
                ) {
                    self.handle_ui_action(action);
                }
                return;
            }
        }

        // 2. Окно живых обоев
        let Some(renderer) = &mut self.renderer else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => renderer.resize(size.width, size.height),
            WindowEvent::RedrawRequested => {
                if self.is_paused {
                    return;
                }
                match renderer.render(&mut self.overlay_effects, &self.post_effects) {
                    Ok(true) => {
                        self.next_frame = Instant::now() + Duration::from_millis(16);
                    },
                    Ok(false) => {
                        self.next_frame = Instant::now() + Duration::from_millis(250);
                    },
                    Err(e) => {
                        eprintln!("Ошибка рендера: {e}");
                        event_loop.exit();
                    }
                }
                if let Err(e) = renderer.render(&mut self.overlay_effects, &self.post_effects) {
                    eprintln!("Ошибка рендера: {e}");
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Tray(action) => match action {
                TrayAction::OpenSettings => {
                    if self.settings_window.is_none() {
                        self.settings_window =
                            Some(SettingsWindow::new(event_loop, self.proxy.clone()));
                    }
                }
                TrayAction::TogglePause => {
                    self.is_paused = !self.is_paused;
                    event_loop.set_control_flow(if self.is_paused {
                        ControlFlow::Wait
                    } else {
                        ControlFlow::WaitUntil(Instant::now())
                    });
                }
                TrayAction::Exit => {
                    event_loop.exit();
                }
            },
            UserEvent::BackgroundChanged(path) => {
                self.settings.image_path = Some(path.clone());
                if let Some(renderer) = &mut self.renderer {
                    if let Err(e) = renderer.set_wallpaper_from_file(&path) {
                        eprintln!("Ошибка загрузки изображения: {e}");
                    }
                }
            },
            UserEvent::EffectTextureChanged { effect_id, path } => {
                if let (Some(renderer), Ok(bytes)) = (&self.renderer, std::fs::read(path)) {
                    if let Some(effect) = self.overlay_effects
                        .iter_mut().find(|e| e.id() == effect_id) 
                    {
                        if let Err(e) = effect.load_texture(renderer.device(), renderer.queue(), &bytes) {
                             eprintln!("Ошибка загрузки текстуры эффекта: {e}");
                        }
                    }
                }
            },
            UserEvent::AddOverlay(kind) => {
                if let Some(renderer) = &self.renderer {
                    let new_effect: Box<dyn OverlayEffect> = match kind {
                        OverlayKind::Particles => {
                            Box::new(ParticleEffect::new(
                                renderer.device(),
                                renderer.queue(),
                                renderer.format(),
                                renderer.global_layout(),
                            ))
                        },
                        OverlayKind::GodRays => {
                            Box::new(GodRaysEffect::new(
                                renderer.device(),
                                renderer.format(),
                                renderer.global_layout(),
                            ))
                        },
                    };
                    self.overlay_effects.push(new_effect);
                }
            },
            UserEvent::RemoveOverlay(idx) => {
                if idx < self.overlay_effects.len() {
                    self.overlay_effects.remove(idx);
                }
            },
            UserEvent::ReorderOverlay { from, to } => {
                if from < self.overlay_effects.len() && to < self.overlay_effects.len() && from != to {
                    let item = self.overlay_effects.remove(from);
                    self.overlay_effects.insert(to, item);
                }
            },
            UserEvent::ReorderPostEffect { from, to } => {
                if from < self.post_effects.len() && to < self.post_effects.len() && from != to {
                    let item = self.post_effects.remove(from);
                    self.post_effects.insert(to, item);
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.is_paused {
            return;
        }
        let now = std::time::Instant::now();
        
        if now >= self.next_frame {
            if let Some(renderer) = &self.renderer {
                renderer.request_redraw();
            }
        } else {
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
        }
    }
}