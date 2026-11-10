//! State yang ditulis oleh thread/observer latar dan dibaca UI setiap frame.
//! Setiap perubahan memanggil `request_repaint`, jadi UI tidur total saat tidak ada apa-apa.

use crate::media::{Lyrics, Player, PlayerState};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Battery {
    pub percent: f32,
    pub plugged: bool,
    pub charging: bool,
}

#[derive(Default)]
pub struct SharedInner {
    pub players: HashMap<Player, PlayerState>,
    pub media_gen: u64,
    pub clip_new: Vec<String>,
    pub battery: Option<Battery>,
    pub lyrics_results: Vec<(String, Option<Lyrics>)>,
    pub screen_changed: bool,
}

pub struct Shared {
    pub ctx: egui::Context,
    pub inner: Mutex<SharedInner>,
    /// Bangunkan UI pada gerakan mouse apa pun (dipakai saat maskot tidur).
    pub wake_on_any_move: AtomicBool,
    pub clipboard_enabled: AtomicBool,
    pub browser_enabled: AtomicBool,
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
impl Shared {
    pub fn new(ctx: egui::Context) -> Arc<Self> {
        Arc::new(Self {
            ctx,
            inner: Mutex::new(SharedInner::default()),
            wake_on_any_move: AtomicBool::new(false),
            clipboard_enabled: AtomicBool::new(true),
            browser_enabled: AtomicBool::new(true),
        })
    }

    pub fn update(&self, f: impl FnOnce(&mut SharedInner)) {
        if let Ok(mut g) = self.inner.lock() {
            f(&mut g);
        }
        self.ctx.request_repaint();
    }

    pub fn set_player(&self, p: Player, st: Option<PlayerState>) {
        self.update(|g| {
            match st {
                Some(s) => {
                    g.players.insert(p, s);
                }
                None => {
                    g.players.remove(&p);
                }
            }
            g.media_gen += 1;
        });
    }

    pub fn get_player(&self, p: Player) -> Option<PlayerState> {
        self.inner.lock().ok()?.players.get(&p).cloned()
    }

    pub fn wake_on_move(&self) -> bool {
        self.wake_on_any_move.load(Ordering::Relaxed)
    }
}

/// Polling baterai ringan (IOKit lewat starship-battery) tiap 20 detik di thread sendiri.
/// Mac desktop tanpa baterai: thread langsung berhenti.
pub fn spawn_battery(shared: Arc<Shared>) {
    std::thread::Builder::new()
        .name("battery".into())
        .spawn(move || {
            let Ok(manager) = starship_battery::Manager::new() else { return };
            let mut last: Option<Battery> = None;
            loop {
                let b = manager
                    .batteries()
                    .ok()
                    .and_then(|mut it| it.next())
                    .and_then(|r| r.ok());
                let Some(b) = b else { return };
                let state = b.state();
                let cur = Battery {
                    percent: (b.state_of_charge().value * 100.0).round(),
                    plugged: state != starship_battery::State::Discharging,
                    charging: state == starship_battery::State::Charging,
                };
                if last != Some(cur) {
                    last = Some(cur);
                    shared.update(|g| g.battery = Some(cur));
                }
                std::thread::sleep(Duration::from_secs(20));
            }
        })
        .ok();
}
