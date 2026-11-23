use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use winit::event_loop::EventLoopProxy;
use crate::app::UserEvent;

/// Контекст, который UI передает эффекту для отрисовки его контролов
pub struct UiContext<'a> {
    pub ui: &'a mut egui::Ui,
    pub proxy: &'a EventLoopProxy<UserEvent>,
    pub is_picking_file: &'a Arc<AtomicBool>,
}

impl<'a> UiContext<'a> {
    pub fn texture_picker_button(&mut self, label: &str, effect_id: u64) {
        let is_picking = self.is_picking_file.load(Ordering::Relaxed);
        if self.ui.add_enabled(!is_picking, egui::Button::new(label)).clicked() {
            let proxy = self.proxy.clone();
            let flag = self.is_picking_file.clone();
            flag.store(true, Ordering::Relaxed);

            std::thread::spawn(move || {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
                    .pick_file()
                {
                    let _ = proxy.send_event(UserEvent::EffectTextureChanged { effect_id, path });
                }
                flag.store(false, Ordering::Relaxed);
            });
        }
    }
}

/// Общий интерфейс для любого слоя в редакторе UI
pub trait LayerItem {
    fn id(&self) -> u64;
    fn name(&self) -> &str;
    fn is_enabled(&self) -> bool;
    fn set_enabled(&mut self, enabled: bool);
    fn draw_ui(&mut self, ctx: &mut UiContext) -> bool;
}