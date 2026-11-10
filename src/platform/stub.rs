//! Implementasi pengganti (Linux) supaya UI bisa dijalankan & dites di luar macOS.

use super::{DragPayload, ScreenInfo};
use crate::media::{MediaCmd, Player};
use crate::shared::Shared;
use std::sync::Arc;

pub struct Platform {
    #[allow(dead_code)]
    shared: Arc<Shared>,
}

/// Posisi awal window (koordinat layar, titik kiri-atas).
pub fn initial_position(_w: f32) -> Option<[f32; 2]> {
    None
}

impl Platform {
    pub fn new(_cc: &eframe::CreationContext<'_>, shared: Arc<Shared>) -> Self {
        Self { shared }
    }

    pub fn screen(&self) -> ScreenInfo {
        if std::env::var("NUSA_NO_NOTCH").is_ok() {
            ScreenInfo { has_notch: false, notch_w: 0.0, notch_h: 24.0 }
        } else {
            ScreenInfo { has_notch: true, notch_w: 186.0, notch_h: 32.0 }
        }
    }

    pub fn configure_window(&self, _show_in_fullscreen: bool, _size: egui::Vec2) {}
    pub fn supports_passthrough(&self) -> bool {
        false
    }
    pub fn set_hot_rect(&self, _r: egui::Rect) {}
    pub fn cursor_pos(&self, ctx: &egui::Context) -> Option<egui::Pos2> {
        ctx.input(|i| i.pointer.hover_pos())
    }
    pub fn mouse_button_down(&self) -> bool {
        false
    }
    pub fn media_cmd(&self, _p: Player, _c: MediaCmd) {}
    pub fn refresh_media(&self, _p: Player) {}
    pub fn idle_seconds(&self) -> f64 {
        std::env::var("NUSA_IDLE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0)
    }
    pub fn copy_text(&self, ctx: &egui::Context, s: &str) {
        ctx.copy_text(s.to_string());
    }
    pub fn copy_files(&self, ctx: &egui::Context, paths: &[String]) {
        ctx.copy_text(paths.join("\n"));
    }
    pub fn start_drag(&self, _payload: DragPayload) -> bool {
        false
    }
    pub fn open(&self, path: &str) {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
    pub fn reveal(&self, path: &str) {
        let dir = std::path::Path::new(path)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
    }
    pub fn play_sound(&self) {}
    pub fn deactivate(&self) {}
    pub fn set_launch_at_login(&self, _on: bool) -> Result<(), String> {
        Err("Hanya tersedia di macOS".into())
    }
}
