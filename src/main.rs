//! Nusa — notch hidup untuk macOS.
//! Musik + lirik, fokus, air, to-do, shelf, clipboard, waktu sholat, baterai, maskot.

mod app;
mod browser;
mod demo;
mod media;
mod platform;
mod prayer;
mod shared;
mod store;
mod theme;
mod util;
mod views;

use app::{WIN_H, WIN_W};

fn main() -> eframe::Result<()> {
    // Satu instance saja.
    let lock_path = util::data_dir().join(".lock");
    let lock = std::fs::File::create(&lock_path).ok();
    if std::env::var("NUSA_DEMO").is_err() {
        if let Some(f) = &lock {
            if f.try_lock().is_err() {
                eprintln!("Nusa sudah berjalan.");
                return Ok(());
            }
        }
    }

    let viewport = egui::ViewportBuilder::default()
        .with_title(util::APP_NAME)
        .with_app_id(util::BUNDLE_ID)
        .with_inner_size([WIN_W, WIN_H])
        .with_position(platform::initial_position(WIN_W).unwrap_or([0.0, 0.0]))
        .with_decorations(false)
        .with_transparent(true)
        .with_resizable(false)
        .with_has_shadow(false)
        .with_taskbar(false)
        .with_active(false)
        .with_always_on_top()
        .with_drag_and_drop(true);

    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Glow,
        multisampling: 0,
        depth_buffer: 0,
        stencil_buffer: 0,
        centered: false,
        persist_window: false,
        system_font_fallback: false,
        ..Default::default()
    };

    let res = eframe::run_native(
        util::APP_NAME,
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    );
    drop(lock);
    res
}
