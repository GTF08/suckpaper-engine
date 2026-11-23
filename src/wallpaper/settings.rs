use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct WallpaperSettings {
    pub image_path: Option<PathBuf>,
}