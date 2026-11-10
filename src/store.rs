//! Data lokal: settings, to-do, log air, shelf, riwayat fokus, clipboard.
//! Disimpan sebagai JSON di ~/Library/Application Support/Nusa/data.json.
//! Tanpa akun, tanpa analytics.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    // Tampilan
    pub show_clock_idle: bool,
    pub minimal: bool,
    pub mascot: bool,
    pub eq_animation: bool,
    pub peek_new_track: bool,
    pub show_in_fullscreen: bool,
    pub launch_at_login: bool,
    pub status_ring: StatusRing,
    // Musik
    pub lyrics: bool,
    pub browser_media: bool,
    // Fokus (menit)
    pub focus_min: u32,
    pub short_break_min: u32,
    pub long_break_min: u32,
    pub rounds: u32,
    // Air
    pub water_reminders: bool,
    pub water_bottle_ml: u32,
    pub water_goal_ml: u32,
    pub water_interval_min: u32,
    pub water_start_h: u32,
    pub water_end_h: u32,
    // Sholat
    pub prayer_enabled: bool,
    pub prayer_city: String,
    pub prayer_lat: f64,
    pub prayer_lon: f64,
    pub prayer_remind_min: u32,
    pub prayer_pause_music: bool,
    // Clipboard
    pub clipboard_enabled: bool,
    pub clipboard_max: usize,
    pub clipboard_persist: bool,
    // To-do
    pub todo_sort_by_time: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusRing {
    Water,
    Battery,
    /// Nilai tidak dikenal → Off (supaya file data lama/baru tetap bisa dibaca).
    #[serde(other)]
    Off,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            show_clock_idle: true,
            minimal: false,
            mascot: true,
            eq_animation: true,
            peek_new_track: true,
            show_in_fullscreen: false,
            launch_at_login: false,
            status_ring: StatusRing::Water,
            lyrics: true,
            browser_media: true,
            focus_min: 25,
            short_break_min: 5,
            long_break_min: 15,
            rounds: 4,
            water_reminders: true,
            water_bottle_ml: 600,
            water_goal_ml: 2000,
            water_interval_min: 60,
            water_start_h: 8,
            water_end_h: 21,
            prayer_enabled: true,
            prayer_city: "Jakarta".into(),
            prayer_lat: -6.2088,
            prayer_lon: 106.8456,
            prayer_remind_min: 10,
            prayer_pause_music: true,
            clipboard_enabled: true,
            clipboard_max: 20,
            clipboard_persist: false,
            todo_sort_by_time: false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Todo {
    pub id: u64,
    pub text: String,
    pub done: bool,
    /// Unix timestamp (detik) tenggat, opsional.
    pub due: Option<i64>,
    pub notified: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WaterEntry {
    pub ts: i64,
    pub ml: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ShelfItem {
    pub path: String,
    pub added: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FocusSession {
    pub ts: i64,
    pub minutes: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ClipItem {
    pub text: String,
    pub ts: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Store {
    pub settings: Settings,
    pub todos: Vec<Todo>,
    pub water: Vec<WaterEntry>,
    pub shelf: Vec<ShelfItem>,
    pub focus_history: Vec<FocusSession>,
    pub clipboard: Vec<ClipItem>,
    pub next_id: u64,
    #[serde(skip)]
    pub dirty: bool,
    #[serde(skip)]
    pub path: Option<PathBuf>,
}

impl Store {
    pub fn load(path: PathBuf) -> Self {
        let mut s: Store = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        if !s.settings.clipboard_persist {
            s.clipboard.clear();
        }
        s.path = Some(path);
        s.prune();
        s
    }

    pub fn save(&mut self) {
        let Some(path) = self.path.clone() else { return };
        let mut copy = self.clone();
        if !copy.settings.clipboard_persist {
            copy.clipboard.clear();
        }
        if let Ok(json) = serde_json::to_string_pretty(&copy) {
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, json).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
        self.dirty = false;
    }

    pub fn touch(&mut self) {
        self.dirty = true;
    }

    pub fn new_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    /// Buang data lama supaya file tetap kecil.
    fn prune(&mut self) {
        let cutoff = crate::util::now_ts() - 60 * 60 * 24 * 60; // 60 hari
        self.water.retain(|w| w.ts >= cutoff);
        self.focus_history.retain(|f| f.ts >= cutoff);
    }

    pub fn push_clip(&mut self, text: String) {
        if text.trim().is_empty() {
            return;
        }
        self.clipboard.retain(|c| c.text != text);
        self.clipboard.insert(
            0,
            ClipItem {
                text,
                ts: crate::util::now_ts(),
            },
        );
        let max = self.settings.clipboard_max.max(1);
        self.clipboard.truncate(max);
        if self.settings.clipboard_persist {
            self.touch();
        }
    }

    pub fn add_shelf(&mut self, path: String) {
        if self.shelf.iter().any(|s| s.path == path) {
            return;
        }
        self.shelf.push(ShelfItem {
            path,
            added: crate::util::now_ts(),
        });
        self.touch();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_and_defaults() {
        let dir = std::env::temp_dir().join(format!("nusa-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("data.json");
        let mut s = Store::load(p.clone());
        assert_eq!(s.settings.focus_min, 25);
        let id = s.new_id();
        s.todos.push(Todo {
            id,
            text: "Kirim invoice".into(),
            ..Default::default()
        });
        s.push_clip("rahasia".into());
        s.save();
        let s2 = Store::load(p.clone());
        assert_eq!(s2.todos.len(), 1);
        assert!(s2.clipboard.is_empty(), "clipboard tidak disimpan by default");
        // file lama tanpa field baru, field yang sudah dihapus, atau varian lama tetap bisa dibaca
        std::fs::write(&p, r#"{"settings":{"focus_min":50,"status_ring":"Gelombang","clipboard_max":5},"clipboard":[{"text":"x","ts":1}]}"#).unwrap();
        let s3 = Store::load(p);
        assert_eq!(s3.settings.focus_min, 50);
        assert_eq!(s3.settings.water_goal_ml, 2000);
        assert_eq!(s3.settings.status_ring, StatusRing::Off);
        assert_eq!(s3.settings.clipboard_max, 5);
        assert!(s3.clipboard.is_empty());
    }
}
