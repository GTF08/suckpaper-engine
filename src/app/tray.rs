use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};
use winit::event_loop::EventLoopProxy;

use crate::app::UserEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    OpenSettings,
    TogglePause,
    Exit,
}

pub struct AppTray {
    _tray_icon: TrayIcon,
}

impl AppTray {
    pub fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
        let menu = Menu::new();

        let settings_item = MenuItem::new("Настройки", true, None);
        let pause_item = MenuItem::new("Пауза / Возобновить", true, None);
        let exit_item = MenuItem::new("Выход", true, None);

        let settings_id = settings_item.id().clone();
        let pause_id = pause_item.id().clone();
        let exit_id = exit_item.id().clone();

        menu.append_items(&[
            &settings_item,
            &pause_item,
            &PredefinedMenuItem::separator(),
            &exit_item,
        ])
        .expect("Не удалось собрать меню трея");

        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let action = if event.id == settings_id {
                Some(TrayAction::OpenSettings)
            } else if event.id == pause_id {
                Some(TrayAction::TogglePause)
            } else if event.id == exit_id {
                Some(TrayAction::Exit)
            } else {
                None
            };

            if let Some(action) = action {
                let _ = proxy.send_event(UserEvent::Tray(action));
            }
        }));

        let icon = Self::create_default_icon();

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Rust Wallpaper Engine")
            .with_icon(icon)
            .build()
            .expect("Не удалось создать иконку в трее");

        Self {
            _tray_icon: tray_icon
        }
    }

    fn create_default_icon() -> Icon {
        let width = 16;
        let height = 16;
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);

        for y in 0..height {
            for x in 0..width {
                let r = (x * 16) as u8;
                let g = (y * 16) as u8;
                let b = 220;
                let a = 255;
                rgba.extend_from_slice(&[r, g, b, a]);
            }
        }

        Icon::from_rgba(rgba, width, height).expect("Ошибка создания иконки")
    }
}