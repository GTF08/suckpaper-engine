mod app;
mod wallpaper;

pub mod texture;

use app::App;

fn main() {
    App::run().unwrap();
}
