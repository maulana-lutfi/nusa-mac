//! Inti aplikasi: bentuk notch, animasi, hover, pengingat, dan penjadwalan repaint.
//!
//! Prinsip hemat daya: UI hanya digambar ulang saat ada event (mouse, notifikasi musik,
//! clipboard, baterai) atau saat ada sesuatu yang dijadwalkan (menit berganti, timer,
//! pengingat). Di antara itu proses benar-benar tidur.

use crate::media::{lyrics_key, Lyrics, MediaCmd, Player, PlayerState};
use crate::platform::{Platform, ScreenInfo};
use crate::prayer::{self, DayTimes};
use crate::shared::{spawn_battery, Battery, Shared};
use crate::store::{FocusSession, Settings, StatusRing, Store, Todo, WaterEntry};
use crate::theme::*;
use crate::util::{data_dir, now_ts};
use crate::views;

use chrono::{Local, NaiveDate, TimeZone, Timelike};
use egui::{emath::easing, pos2, vec2, Align2, Color32, CornerRadius, Id, Key, Rect, Sense, Vec2};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const WIN_W: f32 = 640.0;
pub const WIN_H: f32 = 272.0;
pub const EXP_W: f32 = 600.0;
pub const BODY_H: f32 = 206.0;
const EAR: f32 = 62.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Tab {
    Music,
    Focus,
    Water,
    Todo,
    Shelf,
    Prayer,
    Settings,
}

impl Tab {
    pub fn title(self) -> &'static str {
        match self {
            Tab::Music => "Musik",
            Tab::Focus => "Fokus",
            Tab::Water => "Air",
            Tab::Todo => "To-do",
            Tab::Shelf => "Shelf & Clipboard",
            Tab::Prayer => "Waktu Sholat",
            Tab::Settings => "Setelan",
        }
    }
    pub fn icon(self) -> Icon {
        match self {
            Tab::Music => Icon::Music,
            Tab::Focus => Icon::Timer,
            Tab::Water => Icon::Water,
            Tab::Todo => Icon::Todo,
            Tab::Shelf => Icon::Shelf,
            Tab::Prayer => Icon::Prayer,
            Tab::Settings => Icon::Gear,
        }
    }
    pub fn color(self) -> Color32 {
        match self {
            Tab::Music => SPOTIFY,
            Tab::Focus => FOCUS,
            Tab::Water => WATER,
            Tab::Todo => TODO,
            Tab::Shelf => ACCENT,
            Tab::Prayer => PRAYER,
            Tab::Settings => DIM,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PeekKind {
    Track,
    Timer,
    Water,
    Todo,
    Prayer,
    Battery,
    Hello,
}

pub struct Peek {
    pub kind: PeekKind,
    pub title: String,
    pub sub: String,
    pub until: Instant,
    pub tab: Option<Tab>,
    pub color: Color32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Focus,
    ShortBreak,
    LongBreak,
}

impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Phase::Focus => "Fokus",
            Phase::ShortBreak => "Istirahat",
            Phase::LongBreak => "Istirahat panjang",
        }
    }
    pub fn color(self) -> Color32 {
        if self == Phase::Focus {
            FOCUS
        } else {
            BREAK
        }
    }
}

pub struct FocusTimer {
    pub phase: Phase,
    /// Jumlah sesi fokus yang selesai di siklus ini.
    pub done_in_cycle: u32,
    pub running: bool,
    /// Waktu selesai sebagai detik Unix (jam dinding). Bukan `Instant`, karena
    /// `Instant` di macOS berhenti saat Mac tidur sehingga timer akan molor.
    pub ends_at: Option<f64>,
    pub remaining: f64,
    pub total: f64,
}

impl FocusTimer {
    pub fn new(s: &Settings) -> Self {
        let total = s.focus_min.max(1) as f64 * 60.0;
        Self {
            phase: Phase::Focus,
            done_in_cycle: 0,
            running: false,
            ends_at: None,
            remaining: total,
            total,
        }
    }
    pub fn phase_secs(s: &Settings, p: Phase) -> f64 {
        60.0 * match p {
            Phase::Focus => s.focus_min,
            Phase::ShortBreak => s.short_break_min,
            Phase::LongBreak => s.long_break_min,
        }
        .max(1) as f64
    }
    pub fn remaining_now(&self) -> f64 {
        match (self.running, self.ends_at) {
            (true, Some(e)) => (e - wall_secs()).max(0.0),
            _ => self.remaining,
        }
    }
    pub fn start(&mut self) {
        self.ends_at = Some(wall_secs() + self.remaining.max(0.0));
        self.running = true;
    }
    pub fn pause(&mut self) {
        self.remaining = self.remaining_now();
        self.running = false;
        self.ends_at = None;
    }
    /// Pindah ke fase berikutnya (belum berjalan).
    pub fn advance(&mut self, s: &Settings) {
        self.phase = match self.phase {
            Phase::Focus => {
                self.done_in_cycle += 1;
                if self.done_in_cycle >= s.rounds.max(1) {
                    Phase::LongBreak
                } else {
                    Phase::ShortBreak
                }
            }
            Phase::LongBreak => {
                self.done_in_cycle = 0;
                Phase::Focus
            }
            Phase::ShortBreak => Phase::Focus,
        };
        self.total = Self::phase_secs(s, self.phase);
        self.remaining = self.total;
        self.running = false;
        self.ends_at = None;
    }
}

pub enum LyricsState {
    Loading,
    Found(Lyrics),
    Missing,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Ears {
    None,
    Music(Player),
    Focus,
    Prayer(usize, i64),
    Clock,
}

pub struct App {
    pub store: Store,
    pub platform: Platform,
    pub shared: Arc<Shared>,
    pub screen: ScreenInfo,
    pub expanded: bool,
    pub tab: Tab,
    pub peeks: VecDeque<Peek>,
    hover_since: Option<Instant>,
    leave_since: Option<Instant>,
    passthrough: Option<bool>,
    // musik
    pub players: HashMap<Player, PlayerState>,
    media_gen: u64,
    pub pinned_player: Option<Player>,
    pub art_tex: HashMap<Player, (String, egui::TextureHandle)>,
    pub lyrics: HashMap<String, LyricsState>,
    pub show_lyrics: bool,
    last_track_key: HashMap<Player, String>,
    // fokus
    pub focus: FocusTimer,
    // air
    last_water_reminder: i64,
    started_ts: i64,
    started_at: Instant,
    // sholat
    prayer_cache: Option<(NaiveDate, String, DayTimes)>,
    prayer_fired: HashSet<(NaiveDate, usize, u8)>,
    // baterai
    pub battery: Option<Battery>,
    low_batt_fired: u8,
    // maskot
    idle_secs: f64,
    last_idle_check: Option<Instant>,
    mascot_sleeping: bool,
    // input to-do
    pub todo_input: String,
    pub todo_time: String,
    /// To-do yang sedang diedit: (id, teks, jam).
    pub todo_edit: Option<(u64, String, String)>,
    pub settings_msg: Option<String>,
    last_save: Instant,
    wake: f64,
    demo_state: Option<String>,
    configured_fs: Option<bool>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = cc.egui_ctx.clone();
        setup_style(&ctx);
        let shared = Shared::new(ctx.clone());
        let platform = Platform::new(cc, shared.clone());
        let screen = platform.screen();
        let store = Store::load(data_dir().join("data.json"));
        let focus = FocusTimer::new(&store.settings);
        spawn_battery(shared.clone());
        let mut app = Self {
            store,
            platform,
            shared,
            screen,
            expanded: false,
            tab: Tab::Music,
            peeks: VecDeque::new(),
            hover_since: None,
            leave_since: None,
            passthrough: None,
            players: HashMap::new(),
            media_gen: 0,
            pinned_player: None,
            art_tex: HashMap::new(),
            lyrics: HashMap::new(),
            show_lyrics: false,
            last_track_key: HashMap::new(),
            focus,
            last_water_reminder: 0,
            started_ts: now_ts(),
            started_at: Instant::now(),
            prayer_cache: None,
            prayer_fired: HashSet::new(),
            battery: None,
            low_batt_fired: 0,
            idle_secs: 0.0,
            last_idle_check: None,
            mascot_sleeping: false,
            todo_input: String::new(),
            todo_time: String::new(),
            todo_edit: None,
            settings_msg: None,
            last_save: Instant::now(),
            wake: f64::INFINITY,
            demo_state: None,
            configured_fs: None,
        };
        if std::env::var("NUSA_DEMO").is_ok() {
            crate::demo::seed(&mut app);
        }
        app
    }

    // ------------------------------------------------------------------ util

    fn wake_in(&mut self, secs: f64) {
        if secs.is_finite() {
            self.wake = self.wake.min(secs.max(0.016));
        }
    }

    pub fn push_peek(&mut self, kind: PeekKind, title: impl Into<String>, sub: impl Into<String>, secs: f64, tab: Option<Tab>, color: Color32) {
        let peek = Peek {
            kind,
            title: title.into(),
            sub: sub.into(),
            until: Instant::now() + Duration::from_secs_f64(secs),
            tab,
            color,
        };
        if let Some(pos) = self.peeks.iter().position(|p| p.kind == kind) {
            self.peeks[pos] = peek;
        } else {
            self.peeks.push_back(peek);
        }
        self.shared.ctx.request_repaint();
    }

    fn alert(&mut self, kind: PeekKind, title: impl Into<String>, sub: impl Into<String>, tab: Option<Tab>, color: Color32) {
        self.push_peek(kind, title, sub, 6.0, tab, color);
        self.platform.play_sound();
    }

    pub fn active_player(&self) -> Option<Player> {
        if let Some(p) = self.pinned_player {
            if self.players.contains_key(&p) {
                return Some(p);
            }
        }
        Player::ALL
            .into_iter()
            .find(|p| self.players.get(p).is_some_and(|s| s.track.playing))
            .or_else(|| Player::ALL.into_iter().find(|p| self.players.contains_key(p)))
    }

    pub fn media(&self, p: Player, c: MediaCmd) {
        self.platform.media_cmd(p, c);
    }

    pub fn current_lyrics(&self) -> Option<&LyricsState> {
        let p = self.active_player()?;
        let st = self.players.get(&p)?;
        self.lyrics.get(&lyrics_key(&st.track))
    }

    pub fn water_today(&self) -> u32 {
        let today = Local::now().date_naive();
        self.store
            .water
            .iter()
            .filter(|w| ts_date(w.ts) == Some(today))
            .map(|w| w.ml)
            .sum()
    }

    pub fn add_water(&mut self, ml: u32) {
        self.store.water.push(WaterEntry { ts: now_ts(), ml });
        self.store.touch();
    }

    pub fn add_todo(&mut self) {
        let text = self.todo_input.trim().to_string();
        if text.is_empty() {
            return;
        }
        let due = parse_due(&self.todo_time);
        let id = self.store.new_id();
        self.store.todos.push(Todo {
            id,
            text,
            done: false,
            due,
            notified: false,
        });
        self.todo_input.clear();
        self.todo_time.clear();
        self.store.touch();
    }

    pub fn prayer_today(&mut self) -> Option<DayTimes> {
        let s = &self.store.settings;
        if !s.prayer_enabled {
            return None;
        }
        let today = Local::now().date_naive();
        let key = format!("{}|{:.4}|{:.4}", s.prayer_city, s.prayer_lat, s.prayer_lon);
        if let Some((d, k, t)) = &self.prayer_cache {
            if *d == today && *k == key {
                return Some(t.clone());
            }
        }
        let tz = Local::now().offset().local_minus_utc() as f64 / 3600.0;
        let t = prayer::compute(today, s.prayer_lat, s.prayer_lon, tz);
        self.prayer_cache = Some((today, key, t.clone()));
        Some(t)
    }

    /// (indeks sholat berikutnya, detik tersisa).
    pub fn next_prayer(&mut self) -> Option<(usize, i64)> {
        let t = self.prayer_today()?;
        let now = local_hours_now();
        for (i, h) in t.prayers().iter().enumerate() {
            if *h > now {
                return Some((i, ((h - now) * 3600.0) as i64));
            }
        }
        // besok subuh
        let s = &self.store.settings;
        let tz = Local::now().offset().local_minus_utc() as f64 / 3600.0;
        let tomorrow = Local::now().date_naive().succ_opt()?;
        let tt = prayer::compute(tomorrow, s.prayer_lat, s.prayer_lon, tz);
        Some((0, ((24.0 - now + tt.fajr) * 3600.0) as i64))
    }

    // ------------------------------------------------------------------ tick

    fn pull_shared(&mut self, ctx: &egui::Context) {
        let mut clips = Vec::new();
        let mut lyr = Vec::new();
        let mut screen_changed = false;
        let mut battery_new = self.battery;
        if let Ok(mut g) = self.shared.inner.lock() {
            if g.media_gen != self.media_gen {
                self.media_gen = g.media_gen;
                self.players = g.players.clone();
            }
            clips = std::mem::take(&mut g.clip_new);
            lyr = std::mem::take(&mut g.lyrics_results);
            screen_changed = std::mem::take(&mut g.screen_changed);
            battery_new = g.battery;
        }
        if battery_new != self.battery {
            let old = self.battery;
            self.battery = battery_new;
            self.on_battery(old);
        }
        for c in clips {
            self.store.push_clip(c);
        }
        for (k, l) in lyr {
            self.lyrics.insert(
                k,
                match l {
                    Some(l) => LyricsState::Found(l),
                    None => LyricsState::Missing,
                },
            );
        }
        if screen_changed {
            self.screen = self.platform.screen();
            self.platform
                .configure_window(self.store.settings.show_in_fullscreen, vec2(WIN_W, WIN_H));
        }
        // tekstur artwork
        for (p, st) in &self.players {
            let Some(img) = &st.art else { continue };
            let fresh = self.art_tex.get(p).is_none_or(|(k, _)| *k != st.art_key);
            if fresh {
                let tex = ctx.load_texture(
                    format!("art-{p:?}"),
                    (**img).clone(),
                    egui::TextureOptions::LINEAR,
                );
                self.art_tex.insert(*p, (st.art_key.clone(), tex));
            }
        }
        self.art_tex.retain(|p, _| self.players.get(p).is_some_and(|s| s.art.is_some()));
    }

    fn on_battery(&mut self, old: Option<Battery>) {
        let Some(b) = self.battery else { return };
        if let Some(o) = old {
            if o.plugged != b.plugged {
                let (t, c) = if b.plugged {
                    ("Mengisi daya", BREAK)
                } else {
                    ("Pengisi daya dilepas", DIM)
                };
                self.push_peek(PeekKind::Battery, t, format!("{:.0}%", b.percent), 3.0, None, c);
            }
        }
        if b.plugged {
            self.low_batt_fired = 0;
        } else if b.percent <= 10.0 && self.low_batt_fired < 2 {
            self.low_batt_fired = 2;
            self.alert(PeekKind::Battery, "Baterai hampir habis", format!("{:.0}% tersisa", b.percent), None, DANGER);
        } else if b.percent <= 20.0 && self.low_batt_fired < 1 {
            self.low_batt_fired = 1;
            self.alert(PeekKind::Battery, "Baterai lemah", format!("{:.0}% tersisa", b.percent), None, FOCUS);
        }
    }

    fn tick_media(&mut self) {
        let keys: Vec<(Player, String, bool)> = self
            .players
            .iter()
            .map(|(p, s)| {
                let k = if s.track.id.is_empty() { lyrics_key(&s.track) } else { s.track.id.clone() };
                (*p, k, s.track.playing)
            })
            .collect();
        for (p, key, playing) in keys {
            let changed = self.last_track_key.get(&p) != Some(&key);
            if changed {
                self.last_track_key.insert(p, key);
                let is_active = self.active_player() == Some(p);
                if playing && is_active && !self.expanded && self.store.settings.peek_new_track && self.started_at.elapsed() > Duration::from_secs(4) {
                    let t = &self.players[&p].track;
                    let (title, artist) = (t.title.clone(), t.artist.clone());
                    self.push_peek(PeekKind::Track, title, artist, 3.0, Some(Tab::Music), player_color(p));
                }
            }
        }
        self.last_track_key.retain(|p, _| self.players.contains_key(p));
        // lirik untuk lagu aktif
        if self.store.settings.lyrics {
            if let Some(p) = self.active_player() {
                let track = self.players[&p].track.clone();
                let key = lyrics_key(&track);
                if !track.title.is_empty() && !self.lyrics.contains_key(&key) {
                    self.lyrics.insert(key.clone(), LyricsState::Loading);
                    let sh = self.shared.clone();
                    std::thread::spawn(move || {
                        let res = crate::media::fetch_lyrics(&track);
                        sh.update(|g| g.lyrics_results.push((key, res)));
                    });
                }
            }
        }
        if self.lyrics.len() > 40 {
            self.lyrics.clear();
        }
    }

    fn tick_timers(&mut self) {
        let now = now_ts();
        // Fokus
        if self.focus.running {
            let rem = self.focus.remaining_now();
            if rem <= 0.0 {
                let finished = self.focus.phase;
                if finished == Phase::Focus {
                    self.store.focus_history.push(FocusSession {
                        ts: now,
                        minutes: self.store.settings.focus_min,
                    });
                    self.store.touch();
                }
                let s = self.store.settings.clone();
                self.focus.advance(&s);
                let next = self.focus.phase;
                let (title, sub) = match finished {
                    Phase::Focus => ("Sesi fokus selesai", format!("{} {} menit — klik untuk mulai", next.label(), (self.focus.total / 60.0) as u32)),
                    _ => ("Istirahat selesai", "Siap fokus lagi? Klik untuk mulai".to_string()),
                };
                self.alert(PeekKind::Timer, title, sub, Some(Tab::Focus), next.color());
            } else {
                self.wake_in(rem.min(1.0));
            }
        }
        // Air
        let s = self.store.settings.clone();
        if s.water_reminders && s.water_interval_min > 0 {
            let hour = Local::now().hour();
            let last_drink = self.store.water.iter().map(|w| w.ts).max().unwrap_or(0);
            let base = last_drink.max(self.last_water_reminder).max(self.started_ts);
            let due = base + s.water_interval_min as i64 * 60;
            let in_hours = hour >= s.water_start_h && hour < s.water_end_h;
            if in_hours && now >= due && self.water_today() < s.water_goal_ml {
                self.last_water_reminder = now;
                let left = s.water_goal_ml.saturating_sub(self.water_today());
                self.alert(PeekKind::Water, "Waktunya minum", format!("{left} ml lagi menuju target"), Some(Tab::Water), WATER);
            } else {
                // Bangun tepat saat pengingat berikutnya mungkin terjadi, bukan tiap 30 dtk.
                let at_hour = |days: i64, h: u32| {
                    let d = Local::now().date_naive() + chrono::Duration::days(days);
                    d.and_hms_opt(h.min(23), 0, 0)
                        .and_then(|dt| Local.from_local_datetime(&dt).single())
                        .map(|dt| dt.timestamp())
                        .unwrap_or(now + 3600)
                };
                let next = if self.water_today() >= s.water_goal_ml || hour >= s.water_end_h {
                    at_hour(1, s.water_start_h)
                } else if hour < s.water_start_h {
                    at_hour(0, s.water_start_h).max(due)
                } else {
                    due
                };
                self.wake_in((next - now).max(30) as f64);
            }
        }
        // To-do
        let mut fire = Vec::new();
        let mut next_due = i64::MAX;
        for t in self.store.todos.iter_mut() {
            if let Some(d) = t.due {
                if !t.done && !t.notified {
                    if d <= now {
                        t.notified = true;
                        fire.push(t.text.clone());
                    } else {
                        next_due = next_due.min(d);
                    }
                }
            }
        }
        if !fire.is_empty() {
            self.store.touch();
            let sub = if fire.len() > 1 { format!("+{} tugas lain", fire.len() - 1) } else { "Jatuh tempo sekarang".into() };
            self.alert(PeekKind::Todo, fire[0].clone(), sub, Some(Tab::Todo), TODO);
        }
        if next_due != i64::MAX {
            self.wake_in((next_due - now) as f64);
        }
        // Sholat
        if let Some(t) = self.prayer_today() {
            let today = Local::now().date_naive();
            let now_h = local_hours_now();
            let remind = s.prayer_remind_min as f64 / 60.0;
            for (i, h) in t.prayers().iter().enumerate() {
                let name = prayer::NAMES[i];
                if remind > 0.0 && now_h >= h - remind && now_h < *h && self.prayer_fired.insert((today, i, 0)) {
                    let mins = ((h - now_h) * 60.0).ceil() as i64;
                    self.alert(PeekKind::Prayer, format!("{name} {mins} menit lagi"), format!("{} · {}", prayer::fmt_hhmm(*h), s.prayer_city), Some(Tab::Prayer), PRAYER);
                } else if now_h >= *h && now_h < h + 5.0 / 60.0 && self.prayer_fired.insert((today, i, 1)) {
                    self.alert(PeekKind::Prayer, format!("Waktunya {name}"), format!("{} · {}", prayer::fmt_hhmm(*h), s.prayer_city), Some(Tab::Prayer), PRAYER);
                    if s.prayer_pause_music {
                        for (p, st) in &self.players {
                            if st.track.playing {
                                self.platform.media_cmd(*p, MediaCmd::Pause);
                            }
                        }
                    }
                }
                for edge in [h - remind, *h] {
                    if edge > now_h {
                        self.wake_in((edge - now_h) * 3600.0 + 0.5);
                    }
                }
            }
            if self.prayer_fired.len() > 40 {
                self.prayer_fired.retain(|(d, _, _)| *d == today);
            }
        }
        // Peek kedaluwarsa
        let now_i = Instant::now();
        if self.expanded {
            // Tahan alert selama notch terbuka; tampil setelah notch menutup.
            self.peeks.retain(|p| !matches!(p.kind, PeekKind::Track | PeekKind::Battery | PeekKind::Hello));
            for p in self.peeks.iter_mut() {
                p.until = p.until.max(now_i + Duration::from_secs(5));
            }
            return;
        }
        while self.peeks.front().is_some_and(|p| p.until <= now_i) {
            self.peeks.pop_front();
        }
        if let Some(p) = self.peeks.front() {
            let secs = p.until.saturating_duration_since(now_i).as_secs_f64();
            self.wake_in(secs);
        }
    }

    fn tick_idle(&mut self) {
        if !self.store.settings.mascot || self.expanded {
            self.mascot_sleeping = false;
            self.shared.wake_on_any_move.store(false, Ordering::Relaxed);
            return;
        }
        let due = self.last_idle_check.is_none_or(|t| t.elapsed() >= Duration::from_secs(15));
        if due || self.mascot_sleeping {
            self.idle_secs = self.platform.idle_seconds();
            self.last_idle_check = Some(Instant::now());
        }
        let sleeping = self.idle_secs >= 600.0;
        if self.mascot_sleeping && self.idle_secs < 3.0 {
            self.mascot_sleeping = false;
            self.push_peek(PeekKind::Hello, "Halo lagi!", "Selamat datang kembali", 2.5, None, ACCENT);
        } else if sleeping {
            self.mascot_sleeping = true;
        }
        self.shared.wake_on_any_move.store(self.mascot_sleeping, Ordering::Relaxed);
        self.wake_in(15.0);
    }

    // -------------------------------------------------------------- geometri

    fn ears(&mut self) -> Ears {
        if self.focus.running {
            return Ears::Focus;
        }
        if let Some(p) = self.active_player() {
            if self.players[&p].track.playing {
                return Ears::Music(p);
            }
        }
        let remind = self.store.settings.prayer_remind_min as i64 * 60;
        if let Some((i, left)) = self.next_prayer() {
            if left <= remind.max(60) {
                return Ears::Prayer(i, left);
            }
        }
        let s2 = &self.store.settings;
        if let Some(p) = self.active_player() {
            if !s2.minimal {
                return Ears::Music(p);
            }
        }
        if s2.show_clock_idle && !s2.minimal {
            return Ears::Clock;
        }
        Ears::None
    }

    fn collapsed_size(&self, ears: Ears) -> Vec2 {
        let s = self.screen;
        let center = if s.has_notch { s.notch_w } else { 76.0 };
        match ears {
            Ears::None if s.has_notch => vec2(s.notch_w, s.notch_h),
            Ears::None => vec2(130.0, 5.0),
            _ => vec2(center + 2.0 * EAR, s.notch_h),
        }
    }

    fn peek_size(&self, collapsed: Vec2) -> Vec2 {
        vec2(collapsed.x.max(390.0), self.screen.notch_h + 56.0)
    }

    fn expanded_size(&self) -> Vec2 {
        vec2(EXP_W, self.screen.notch_h + BODY_H)
    }

    fn centered(size: Vec2) -> Rect {
        Rect::from_min_size(pos2((WIN_W - size.x) / 2.0, 0.0), size)
    }

    // ------------------------------------------------------------------ hover

    fn update_hover(&mut self, ctx: &egui::Context, hot: Rect, peek_active: bool) {
        let cursor = self.platform.cursor_pos(ctx);
        let exp_rect = Self::centered(self.expanded_size()).expand(16.0);
        let dragging_files = ctx.input(|i| !i.raw.hovered_files.is_empty());
        let in_hot = cursor.is_some_and(|p| hot.contains(p));
        let in_exp = cursor.is_some_and(|p| exp_rect.contains(p));
        let now = Instant::now();

        if let Some(st) = self.demo_state.clone() {
            self.expanded = st == "expanded";
            return;
        }

        // Esc saat mengedit to-do = batal edit, bukan menutup notch.
        if ctx.input(|i| i.key_pressed(Key::Escape)) && self.expanded && self.todo_edit.is_none() {
            self.collapse();
        }

        if !self.expanded {
            self.leave_since = None;
            let clicked = in_hot && ctx.input(|i| i.pointer.primary_clicked());
            if dragging_files || clicked {
                self.expand(if dragging_files { Some(Tab::Shelf) } else { None });
            } else if in_hot && !peek_active {
                let since = *self.hover_since.get_or_insert(now);
                let need = Duration::from_millis(140);
                if since.elapsed() >= need {
                    self.expand(None);
                } else {
                    self.wake_in((need - since.elapsed()).as_secs_f64());
                }
            } else {
                self.hover_since = None;
            }
            // file diseret dari Finder ke notch yang masih tertutup
            if in_hot && self.platform.mouse_button_down() {
                self.wake_in(0.1);
            }
        } else {
            self.hover_since = None;
            let typing = ctx.memory(|m| m.focused().is_some()) && ctx.input(|i| i.viewport().focused != Some(false));
            if in_exp || dragging_files || typing {
                self.leave_since = None;
            } else {
                let since = *self.leave_since.get_or_insert(now);
                let need = Duration::from_millis(350);
                if since.elapsed() >= need {
                    self.collapse();
                } else {
                    self.wake_in((need - since.elapsed()).as_secs_f64());
                }
            }
        }

        let want_hit = self.expanded || in_hot || dragging_files;
        if self.platform.supports_passthrough() && self.passthrough != Some(!want_hit) {
            ctx.send_viewport_cmd(egui::ViewportCommand::MousePassthrough(!want_hit));
            self.passthrough = Some(!want_hit);
        }
        self.platform.set_hot_rect(if self.expanded { exp_rect } else { hot });
    }

    pub fn expand(&mut self, tab: Option<Tab>) {
        if let Some(t) = tab {
            self.tab = t;
        } else if let Some(p) = self.peeks.front() {
            if let Some(t) = p.tab {
                self.tab = t;
            }
        }
        if self.tab == Tab::Prayer && !self.store.settings.prayer_enabled {
            self.tab = Tab::Music;
        }
        self.peeks.clear();
        self.expanded = true;
        self.hover_since = None;
    }

    pub fn collapse(&mut self) {
        self.expanded = false;
        self.leave_since = None;
        self.shared.ctx.memory_mut(|m| {
            if let Some(id) = m.focused() {
                m.surrender_focus(id);
            }
        });
        self.platform.deactivate();
    }

    // ------------------------------------------------------------ menggambar

    fn paint_ears(&mut self, ui: &mut egui::Ui, shape: Rect, ears: Ears, alpha: f32) {
        if alpha <= 0.01 || ears == Ears::None {
            return;
        }
        let p = ui.painter().clone().with_clip_rect(shape);
        let p = p.with_layer_id(ui.layer_id());
        let h = shape.height();
        let left = Rect::from_min_size(shape.min, vec2(EAR, h));
        let right = Rect::from_min_size(pos2(shape.max.x - EAR, shape.min.y), vec2(EAR, h));
        let fade = |c: Color32| c.gamma_multiply(alpha);
        let time = ui.input(|i| i.time);
        match ears {
            Ears::Music(pl) => {
                let st = &self.players[&pl];
                let art = Rect::from_center_size(left.center() + vec2(4.0, 0.0), Vec2::splat((h - 10.0).min(22.0)));
                if let Some((_, tex)) = self.art_tex.get(&pl) {
                    p.image(tex.id(), art, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), fade(Color32::WHITE));
                } else {
                    p.rect_filled(art, CornerRadius::same(5), fade(player_color(pl).gamma_multiply(0.35)));
                    paint_icon(&p, art.shrink(4.0), Icon::Music, fade(TEXT));
                }
                // equalizer
                let playing = st.track.playing;
                let anim = playing && self.store.settings.eq_animation;
                let col = fade(player_color(pl));
                let base = right.center() - vec2(10.0 + 4.0, 0.0);
                for i in 0..4 {
                    let hh = if anim {
                        let v = ((time * (5.0 + i as f64 * 1.7) + i as f64 * 1.3).sin() * 0.5 + 0.5) as f32;
                        4.0 + v * 10.0
                    } else if playing {
                        [8.0, 12.0, 6.0, 10.0][i]
                    } else {
                        3.0
                    };
                    let x = base.x + i as f32 * 7.0;
                    p.rect_filled(
                        Rect::from_center_size(pos2(x, base.y), vec2(3.5, hh)),
                        CornerRadius::same(2),
                        col,
                    );
                }
                if anim {
                    self.wake_in(0.09);
                }
            }
            Ears::Focus => {
                let c = self.focus.phase.color();
                let breathe = ((time * std::f64::consts::TAU / 4.0).sin() * 0.5 + 0.5) as f32;
                p.circle_filled(left.center() + vec2(4.0, 0.0), 4.0 + 2.5 * breathe, fade(c.gamma_multiply(0.35 + 0.65 * breathe)));
                let rem = self.focus.remaining_now();
                text(&p, right.center() - vec2(4.0, 0.0), Align2::CENTER_CENTER, &crate::util::fmt_mmss(rem), 13.0, fade(TEXT));
                self.wake_in(0.25);
            }
            Ears::Prayer(_i, left_s) => {
                paint_icon(&p, Rect::from_center_size(left.center() + vec2(4.0, 0.0), Vec2::splat(16.0)), Icon::Prayer, fade(PRAYER));
                let m = ((left_s + 59) / 60).max(1);
                text(&p, right.center() - vec2(4.0, 0.0), Align2::CENTER_CENTER, &format!("{m} m"), 13.0, fade(PRAYER));
                self.wake_in(((left_s % 60) + 1) as f64);
            }
            Ears::Clock => {
                // cincin status (air / baterai)
                let ring_c = left.center() + vec2(4.0, 0.0);
                match self.store.settings.status_ring {
                    StatusRing::Water => {
                        let f = self.water_today() as f32 / self.store.settings.water_goal_ml.max(1) as f32;
                        ring(&p, ring_c, 7.0, 2.4, f, fade(WATER));
                    }
                    StatusRing::Battery => {
                        if let Some(b) = self.battery {
                            let c = if b.percent <= 20.0 && !b.plugged { DANGER } else if b.plugged { BREAK } else { TEXT };
                            ring(&p, ring_c, 7.0, 2.4, b.percent / 100.0, fade(c));
                        }
                    }
                    StatusRing::Off => {}
                }
                let idle = self.store.settings.mascot && self.idle_secs >= 60.0;
                if idle {
                    self.paint_mascot(&p, right.center() - vec2(4.0, 0.0), alpha, time);
                } else {
                    let now = Local::now();
                    text(&p, right.center() - vec2(4.0, 0.0), Align2::CENTER_CENTER, &now.format("%H:%M").to_string(), 13.0, fade(TEXT));
                    self.wake_in((60 - now.second()) as f64 + 0.05);
                }
            }
            Ears::None => {}
        }
    }

    fn paint_mascot(&mut self, p: &egui::Painter, c: egui::Pos2, alpha: f32, time: f64) {
        let col = TEXT.gamma_multiply(alpha);
        if self.mascot_sleeping {
            for dx in [-6.0, 6.0] {
                arc(p, c + vec2(dx, -1.0), 3.0, 0.15 * std::f32::consts::PI, 0.85 * std::f32::consts::PI, egui::Stroke::new(1.6, col));
            }
            text(p, c + vec2(15.0, -6.0), Align2::CENTER_CENTER, "z", 9.0, DIM.gamma_multiply(alpha));
            return;
        }
        let period = 4.3;
        let phase = time % period;
        let closed = phase < 0.13;
        let eye_h = if closed { 1.8 } else { 9.0 };
        for dx in [-6.0, 6.0] {
            p.rect_filled(Rect::from_center_size(c + vec2(dx, 0.0), vec2(6.0, eye_h)), CornerRadius::same(3), col);
        }
        self.wake_in(if closed { 0.13 - phase } else { period - phase });
    }

    fn paint_peek(&mut self, ui: &mut egui::Ui, shape: Rect, alpha: f32) {
        let Some(peek) = self.peeks.front() else { return };
        let (kind, title, sub, color) = (peek.kind, peek.title.clone(), peek.sub.clone(), peek.color);
        let p = ui.painter().clone().with_clip_rect(shape);
        let fade = |c: Color32| c.gamma_multiply(alpha);
        let body = Rect::from_min_max(pos2(shape.min.x + 18.0, self.screen.notch_h + 4.0), pos2(shape.max.x - 18.0, shape.max.y - 8.0));
        let icon_r = Rect::from_min_size(pos2(body.min.x, body.center().y - 18.0), Vec2::splat(36.0));
        match kind {
            PeekKind::Track => {
                let pl = self.active_player();
                if let Some((_, tex)) = pl.and_then(|pl| self.art_tex.get(&pl)) {
                    p.image(tex.id(), icon_r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), fade(Color32::WHITE));
                } else {
                    p.rect_filled(icon_r, CornerRadius::same(8), fade(color.gamma_multiply(0.3)));
                    paint_icon(&p, icon_r.shrink(9.0), Icon::Music, fade(color));
                }
            }
            PeekKind::Hello => {
                p.rect_filled(icon_r, CornerRadius::same(18), fade(CARD_HI));
                for dx in [-5.0, 5.0] {
                    p.rect_filled(Rect::from_center_size(icon_r.center() + vec2(dx, 0.0), vec2(5.0, 8.0)), CornerRadius::same(3), fade(TEXT));
                }
            }
            _ => {
                let icon = match kind {
                    PeekKind::Timer => Icon::Timer,
                    PeekKind::Water => Icon::Water,
                    PeekKind::Todo => Icon::Todo,
                    PeekKind::Prayer => Icon::Prayer,
                    _ => Icon::Bolt,
                };
                p.circle_filled(icon_r.center(), 18.0, fade(color.gamma_multiply(0.22)));
                paint_icon(&p, icon_r.shrink(9.0), icon, fade(color));
            }
        }
        let tx = icon_r.max.x + 12.0;
        let max_w = body.max.x - tx;
        text_trunc(&p, pos2(tx, body.center().y - 9.0), Align2::LEFT_CENTER, &title, 14.0, fade(TEXT), max_w, true);
        text_trunc(&p, pos2(tx, body.center().y + 10.0), Align2::LEFT_CENTER, &sub, 12.0, fade(DIM), max_w, false);
    }

    fn paint_expanded(&mut self, ui: &mut egui::Ui, shape: Rect, alpha: f32) {
        let nh = self.screen.notch_h;
        let notch_half = if self.screen.has_notch { self.screen.notch_w / 2.0 } else { 40.0 };
        let cx = shape.center().x;
        let p = ui.painter().clone();
        let fade = |c: Color32| c.gamma_multiply(alpha);

        // pita atas: judul (kiri notch) & jam + baterai (kanan notch)
        let band_y = nh / 2.0;
        text_trunc(&p, pos2(shape.min.x + 22.0, band_y), Align2::LEFT_CENTER, self.tab.title(), 13.0, fade(TEXT), cx - notch_half - shape.min.x - 30.0, true);
        let now = Local::now();
        let mut x = shape.max.x - 22.0;
        let r = text(&p, pos2(x, band_y), Align2::RIGHT_CENTER, &now.format("%H:%M").to_string(), 13.0, fade(TEXT));
        x = r.min.x - 12.0;
        if let Some(b) = self.battery {
            let c = if b.percent <= 20.0 && !b.plugged { DANGER } else { DIM };
            let r = text(&p, pos2(x, band_y), Align2::RIGHT_CENTER, &format!("{:.0}%", b.percent), 12.0, fade(c));
            if b.plugged {
                paint_icon(&p, Rect::from_center_size(pos2(r.min.x - 8.0, band_y), Vec2::splat(12.0)), Icon::Bolt, fade(BREAK));
            }
        }
        self.wake_in((60 - now.second()) as f64 + 0.05);

        // rel tab kiri
        let body = Rect::from_min_max(pos2(shape.min.x + 12.0, nh + 4.0), pos2(shape.max.x - 16.0, shape.max.y - 14.0));
        let mut tabs = vec![Tab::Music, Tab::Focus, Tab::Water, Tab::Todo, Tab::Shelf];
        if self.store.settings.prayer_enabled {
            tabs.push(Tab::Prayer);
        }
        tabs.push(Tab::Settings);
        let rail = Rect::from_min_size(body.min, vec2(32.0, body.height()));
        let step = (rail.height() / tabs.len() as f32).min(30.0);
        let mut rail_ui = ui.new_child(egui::UiBuilder::new().max_rect(rail));
        rail_ui.set_opacity(alpha);
        rail_ui.spacing_mut().item_spacing = vec2(0.0, step - 28.0_f32.min(step));
        for t in tabs {
            let size = 28.0_f32.min(step);
            let (r, resp) = rail_ui.allocate_exact_size(vec2(32.0, size), Sense::click());
            let active = self.tab == t;
            let painter = rail_ui.painter();
            if active {
                painter.rect_filled(r.shrink2(vec2(2.0, 1.0)), CornerRadius::same(8), CARD_HI);
            } else if resp.hovered() {
                painter.rect_filled(r.shrink2(vec2(2.0, 1.0)), CornerRadius::same(8), CARD);
            }
            let col = if active { t.color() } else { DIM };
            paint_icon(painter, Rect::from_center_size(r.center(), Vec2::splat(15.0)), t.icon(), col);
            if resp.on_hover_text(t.title()).clicked() {
                self.tab = t;
            }
        }

        // konten
        let content = Rect::from_min_max(pos2(rail.max.x + 10.0, body.min.y), body.max);
        let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(content).layout(egui::Layout::top_down(egui::Align::Min)));
        cui.set_clip_rect(content.expand(2.0));
        cui.set_opacity(alpha);
        match self.tab {
            Tab::Music => views::music::show(self, &mut cui, content),
            Tab::Focus => views::focus::show(self, &mut cui, content),
            Tab::Water => views::water::show(self, &mut cui, content),
            Tab::Todo => views::todo::show(self, &mut cui, content),
            Tab::Shelf => views::shelf::show(self, &mut cui, content),
            Tab::Prayer => views::prayer::show(self, &mut cui, content),
            Tab::Settings => views::settings::show(self, &mut cui, content),
        }
    }

    pub fn request_wake(&mut self, secs: f64) {
        self.wake_in(secs);
    }

    fn handle_drops(&mut self, ctx: &egui::Context) {
        let dropped: Vec<String> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_string_lossy().into_owned())
                .filter(|p| !p.is_empty())
                .collect()
        });
        if !dropped.is_empty() {
            for d in dropped {
                self.store.add_shelf(d);
            }
            self.tab = Tab::Shelf;
            self.expanded = true;
        }
    }
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        if std::env::var("NUSA_BG").is_ok() {
            // latar "wallpaper" untuk preview di Linux
            return Color32::from_rgb(58, 76, 104).to_normalized_gamma_f32();
        }
        [0.0, 0.0, 0.0, 0.0]
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.wake = f64::INFINITY;

        if self.configured_fs != Some(self.store.settings.show_in_fullscreen) {
            self.configured_fs = Some(self.store.settings.show_in_fullscreen);
            self.platform
                .configure_window(self.store.settings.show_in_fullscreen, vec2(WIN_W, WIN_H));
        }

        self.shared
            .clipboard_enabled
            .store(self.store.settings.clipboard_enabled, Ordering::Relaxed);
        self.shared
            .browser_enabled
            .store(self.store.settings.browser_media, Ordering::Relaxed);

        self.pull_shared(&ctx);
        self.tick_media();
        self.tick_timers();
        self.tick_idle();
        self.handle_drops(&ctx);

        // Geometri
        let ears = self.ears();
        let collapsed = self.collapsed_size(ears);
        let peek_active = !self.peeks.is_empty() && !self.expanded;
        let peek_sz = self.peek_size(collapsed);
        let hot_base = if peek_active { Self::centered(peek_sz) } else { Self::centered(collapsed) };
        let hot = Rect::from_min_max(hot_base.min, pos2(hot_base.max.x, hot_base.max.y.max(self.screen.notch_h) + 6.0)).expand2(vec2(6.0, 0.0));
        self.update_hover(&ctx, hot, peek_active);

        let t = ctx.animate_bool_with_time_and_easing(Id::unique("exp"), self.expanded, 0.34, easing::cubic_out);
        let pk = ctx.animate_bool_with_time_and_easing(Id::unique("peek"), peek_active, 0.28, easing::cubic_out);
        let base = collapsed + (peek_sz - collapsed) * pk;
        let size = base + (self.expanded_size() - base) * t;
        let shape = Self::centered(size);

        // Bentuk notch
        let painter = ui.painter();
        let r_bottom = egui::lerp(10.0..=18.0, pk);
        let r_bottom = egui::lerp(r_bottom..=28.0, t).min(shape.height() / 2.0);
        painter.rect_filled(
            shape,
            CornerRadius { nw: 0, ne: 0, sw: r_bottom as u8, se: r_bottom as u8 },
            BG,
        );
        let sr = 7.0 * t.max(pk);
        shoulder(painter, shape.left_top(), sr, true, BG);
        shoulder(painter, shape.right_top(), sr, false, BG);

        // Klik pada notch tertutup / peek
        let resp = ui.interact(shape, Id::unique("notch-body"), Sense::click());
        if !self.expanded && resp.clicked() {
            self.expand(None);
        }

        // Konten
        let ears_alpha = (1.0 - t.max(pk) * 2.5).clamp(0.0, 1.0);
        self.paint_ears(ui, shape, ears, ears_alpha);
        let peek_alpha = ((pk - 0.5) * 2.0).clamp(0.0, 1.0) * (1.0 - t * 3.0).clamp(0.0, 1.0);
        if peek_alpha > 0.0 {
            self.paint_peek(ui, shape, peek_alpha);
        }
        let exp_alpha = ((t - 0.55) / 0.45).clamp(0.0, 1.0);
        if exp_alpha > 0.0 {
            self.paint_expanded(ui, shape, exp_alpha);
        }
        let drop_hint = ctx.input(|i| !i.raw.hovered_files.is_empty());
        if drop_hint && exp_alpha > 0.5 {
            ui.painter().rect_stroke(shape.shrink(6.0), CornerRadius::same(22), egui::Stroke::new(2.0, ACCENT), egui::StrokeKind::Inside);
        }

        // Simpan & jadwalkan bangun berikutnya
        if self.store.dirty && self.last_save.elapsed() > Duration::from_millis(600) {
            self.store.save();
            self.last_save = Instant::now();
        } else if self.store.dirty {
            self.wake_in(0.7);
        }
        if self.wake.is_finite() {
            ctx.request_repaint_after(Duration::from_secs_f64(self.wake));
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.store.save();
    }
}

// ---------------------------------------------------------------------------

pub fn player_color(p: Player) -> Color32 {
    match p {
        Player::Spotify => SPOTIFY,
        Player::Music => MUSIC,
        Player::Browser => YOUTUBE,
    }
}

/// Detik Unix saat ini (jam dinding, tetap maju saat Mac tidur).
pub fn wall_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

pub fn ts_date(ts: i64) -> Option<NaiveDate> {
    Local.timestamp_opt(ts, 0).single().map(|d| d.date_naive())
}

pub fn local_hours_now() -> f64 {
    let n = Local::now();
    n.hour() as f64 + n.minute() as f64 / 60.0 + n.second() as f64 / 3600.0
}

/// "14:30", "14.30", "9" → timestamp hari ini (atau besok jika sudah lewat).
pub fn parse_due(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (h, m) = match s.split_once([':', '.']) {
        Some((h, m)) => (h.trim().parse::<u32>().ok()?, m.trim().parse::<u32>().ok()?),
        None => (s.parse::<u32>().ok()?, 0),
    };
    if h > 23 || m > 59 {
        return None;
    }
    let now = Local::now();
    let today = now.date_naive().and_hms_opt(h, m, 0)?;
    let mut dt = Local.from_local_datetime(&today).single()?;
    if dt <= now {
        dt += chrono::Duration::days(1);
    }
    Some(dt.timestamp())
}

impl App {
    pub fn set_demo_state(&mut self, s: Option<String>) {
        self.demo_state = s;
    }
}

fn setup_style(ctx: &egui::Context) {
    // Font sistem macOS (SF Pro) bila ada — tidak menambah ukuran binary.
    let mut fonts = egui::FontDefinitions::default();
    for path in ["/System/Library/Fonts/SFNS.ttf", "/System/Library/Fonts/Helvetica.ttc"] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert("system".into(), Arc::new(egui::FontData::from_owned(bytes)));
            if let Some(f) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                f.insert(0, "system".into());
            }
            break;
        }
    }
    ctx.set_fonts(fonts);
    ctx.set_theme(egui::Theme::Dark);
    ctx.all_styles_mut(|s| {
        s.visuals = egui::Visuals::dark();
        s.visuals.panel_fill = Color32::TRANSPARENT;
        s.visuals.window_fill = CARD;
        s.visuals.extreme_bg_color = CARD_HI;
        s.visuals.override_text_color = Some(TEXT);
        s.visuals.selection.bg_fill = ACCENT.gamma_multiply(0.5);
        s.visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT);
        for w in [
            &mut s.visuals.widgets.inactive,
            &mut s.visuals.widgets.hovered,
            &mut s.visuals.widgets.active,
            &mut s.visuals.widgets.open,
            &mut s.visuals.widgets.noninteractive,
        ] {
            w.corner_radius = CornerRadius::same(8);
        }
        s.visuals.widgets.inactive.weak_bg_fill = CARD_HI;
        s.visuals.widgets.inactive.bg_fill = CARD_HI;
        s.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(48, 48, 54);
        s.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, CARD_HI);
        s.spacing.item_spacing = vec2(8.0, 6.0);
        s.spacing.button_padding = vec2(10.0, 4.0);
        s.spacing.interact_size.y = 24.0;
        s.spacing.scroll.bar_width = 4.0;
        s.spacing.scroll.floating = true;
        s.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(13.0));
        s.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(12.5));
        s.text_styles.insert(egui::TextStyle::Small, egui::FontId::proportional(11.0));
        s.animation_time = 0.18;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_siklus_pomodoro() {
        let s = Settings { focus_min: 25, short_break_min: 5, long_break_min: 15, rounds: 2, ..Default::default() };
        let mut t = FocusTimer::new(&s);
        assert_eq!(t.remaining_now(), 1500.0);
        t.start();
        // berbasis jam dinding: sisa waktu ≈ total
        assert!((t.remaining_now() - 1500.0).abs() < 1.0);
        t.ends_at = Some(wall_secs() - 1.0); // anggap sudah lewat (mis. setelah Mac tidur)
        assert_eq!(t.remaining_now(), 0.0);
        t.advance(&s);
        assert_eq!((t.phase, t.done_in_cycle, t.running), (Phase::ShortBreak, 1, false));
        t.advance(&s);
        assert_eq!(t.phase, Phase::Focus);
        t.advance(&s);
        assert_eq!(t.phase, Phase::LongBreak, "setelah 2 ronde → istirahat panjang");
        t.advance(&s);
        assert_eq!((t.phase, t.done_in_cycle), (Phase::Focus, 0));
        t.start();
        t.pause();
        assert!(!t.running && t.remaining > 1499.0);
    }

    #[test]
    fn parse_jam_todo() {
        let now = Local::now().timestamp();
        for s in ["14:30", "14.30", "9", " 07:05 "] {
            let ts = parse_due(s).unwrap_or_else(|| panic!("gagal parse {s}"));
            assert!(ts > now && ts <= now + 86400, "{s} harus dalam 24 jam ke depan");
        }
        for s in ["", "25:00", "12:60", "abc"] {
            assert_eq!(parse_due(s), None, "{s}");
        }
    }
}
