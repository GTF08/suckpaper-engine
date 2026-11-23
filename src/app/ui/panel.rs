use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use winit::event_loop::EventLoopProxy;

use crate::app::{OverlayKind, renderer::effects::{OverlayEffect, PostEffect}};
use crate::app::ui::layer::{LayerItem, UiContext};
use crate::app::UserEvent;
use crate::wallpaper::settings::WallpaperSettings;

#[derive(Clone, Copy)]
struct DragPayload {
    group: &'static str,
    index: usize,
}

pub struct SettingsPanel {
    proxy: EventLoopProxy<UserEvent>,
    is_picking_file: Arc<AtomicBool>,
    selected_effect_id: Option<u64>,
}

impl SettingsPanel {
    pub fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
        Self {
            proxy,
            is_picking_file: Arc::new(AtomicBool::new(false)),
            selected_effect_id: None,
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        settings: &mut WallpaperSettings,
        overlays: &mut [Box<dyn OverlayEffect>],
        posts: &mut [Box<dyn PostEffect>],
    ) -> bool {
        let mut changed = false;

        // Автовыбор первого слоя, если ничего не выбрано или текущий слой удалили
        let exists = self.selected_effect_id.is_some_and(|id| {
            overlays.iter().any(|e| e.id() == id) || posts.iter().any(|e| e.id() == id)
        });
        if !exists {
            self.selected_effect_id = overlays
                .first()
                .map(|e| e.id())
                .or_else(|| posts.first().map(|e| e.id()));
        }

        // 1. Левая панель: фон и списки слоев
        egui::Panel::left("layers_panel")
            .resizable(true)
            .default_size(240.0)
            .show(ui, |ui| {
                self.draw_background_section(ui, settings);
                ui.add_space(10.0);

                egui::ScrollArea::vertical().show(ui, |ui| {
                    // Секция Overlay
                    ui.horizontal(|ui| {
                        ui.strong("Scene Overlays");
                        ui.menu_button("+ Add...", |ui| {
                            if ui.small_button("Particles").clicked() {
                                let _ = self.proxy.send_event(UserEvent::AddOverlay(OverlayKind::Particles));
                                ui.close();
                            }
                            if ui.small_button("God Rays").clicked() {
                                let _ = self.proxy.send_event(UserEvent::AddOverlay(OverlayKind::GodRays));
                                ui.close();
                            }
                        });
                    });
                    ui.separator();

                    let (ov_changed, ov_reorder, ov_remove) =
                        self.draw_layer_list(ui, "overlay", overlays, true);
                    changed |= ov_changed;
                    if let Some((from, to)) = ov_reorder {
                        let _ = self.proxy.send_event(UserEvent::ReorderOverlay { from, to });
                    }
                    if let Some(idx) = ov_remove {
                        let _ = self.proxy.send_event(UserEvent::RemoveOverlay(idx));
                    }

                    ui.add_space(12.0);

                    // Секция Post-Processing
                    ui.strong("Post-Processing");
                    ui.separator();

                    let (post_changed, post_reorder, _) =
                        self.draw_layer_list(ui, "post", posts, false);
                    changed |= post_changed;
                    if let Some((from, to)) = post_reorder {
                        let _ = self.proxy.send_event(UserEvent::ReorderPostEffect { from, to });
                    }
                });
            });

        // 2. Правая панель: инспектор выбранного слоя
        egui::CentralPanel::default().show(ui, |ui| {
            changed |= self.draw_inspector(ui, overlays, posts);
        });

        changed
    }

    fn draw_background_section(&self, ui: &mut egui::Ui, settings: &WallpaperSettings) {
        ui.heading("Layers");
        ui.separator();

        let is_picking = self.is_picking_file.load(Ordering::Acquire);
        let btn_text = if is_picking { "Selecting..." } else { "Select Background..." };

        if ui.add_enabled(!is_picking, egui::Button::new(btn_text)).clicked() {
            if !self.is_picking_file.swap(true, Ordering::AcqRel) {
                let flag = self.is_picking_file.clone();
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
                        .pick_file()
                    {
                        let _ = proxy.send_event(UserEvent::BackgroundChanged(path));
                    }
                    flag.store(false, Ordering::Release);
                });
            }
        }

        if let Some(path) = &settings.image_path {
            ui.label(path.file_name().unwrap_or_default().to_string_lossy());
        }
    }

    /// Единый рендер списка слоев с поддержкой Drag & Drop
    fn draw_layer_list<T: LayerItem + ?Sized>(
        &mut self,
        ui: &mut egui::Ui,
        group: &'static str,
        items: &mut [Box<T>],
        can_remove: bool,
    ) -> (bool, Option<(usize, usize)>, Option<usize>) {
        let mut changed = false;
        let mut reorder = None;
        let mut remove = None;

        for i in 0..items.len() {
            let id = items[i].id();
            let is_selected = self.selected_effect_id == Some(id);
            let mut enabled = items[i].is_enabled();

            let row_res = ui
                .horizontal(|ui| {
                    ui.dnd_drag_source(
                        egui::Id::unique((group, id)),
                        DragPayload { group, index: i },

                        |ui| {
                            ui.label("≡");
                        },
                    );

                    if ui.checkbox(&mut enabled, "").changed() {
                        items[i].set_enabled(enabled);
                        changed = true;
                    }

                    if ui.selectable_label(is_selected, items[i].name()).clicked() {
                        self.selected_effect_id = Some(id);
                    }

                    if can_remove && ui.small_button("×").clicked() {
                        remove = Some(i);
                    }
                })
                .response;

            if let Some(payload) = row_res.dnd_hover_payload::<DragPayload>() {
                if payload.group == group && payload.index != i {
                    let rect = row_res.rect;
                    let y = if payload.index < i { rect.bottom() } else { rect.top() };
                    ui.painter().hline(
                        rect.x_range(),
                        y,
                        egui::Stroke::new(2.0, egui::Color32::LIGHT_BLUE),
                    );
                }
            }

            if let Some(payload) = row_res.dnd_release_payload::<DragPayload>() {
                if payload.group == group && payload.index != i {
                    reorder = Some((payload.index, i));
                }
            }
        }

        (changed, reorder, remove)
    }

    fn draw_inspector(
        &self,
        ui: &mut egui::Ui,
        overlays: &mut [Box<dyn OverlayEffect>],
        posts: &mut [Box<dyn PostEffect>],
    ) -> bool {
        let Some(sel_id) = self.selected_effect_id else {
            ui.centered_and_justified(|ui| {
                ui.label("Select a layer on the left to edit its properties");
            });
            return false;
        };

        let mut ctx = UiContext {
            ui,
            proxy: &self.proxy,
            is_picking_file: &self.is_picking_file,
        };

        if let Some(eff) = overlays.iter_mut().find(|e| e.id() == sel_id) {
            ctx.ui.heading(eff.name());
            ctx.ui.separator();
            return eff.draw_ui(&mut ctx);
        }

        if let Some(eff) = posts.iter_mut().find(|e| e.id() == sel_id) {
            ctx.ui.heading(eff.name());
            ctx.ui.separator();
            return eff.draw_ui(&mut ctx);
        }

        false
    }
}
