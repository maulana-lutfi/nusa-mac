//! Mode demo (NUSA_DEMO=1): mengisi data contoh untuk preview/screenshot.
//! Tidak pernah menulis ke data asli.
//!
//! NUSA_TAB   = music|lyrics|focus|water|todo|shelf|prayer|settings
//! NUSA_STATE = collapsed|expanded|peek
//! NUSA_EARS  = music|focus|clock|none   (untuk state collapsed)
//! NUSA_BROWSER = 1|nojs                  (contoh musik dari tab YouTube)

use crate::app::{App, PeekKind, Tab};
use crate::media::{lyrics_key, Lyrics, Player, PlayerState, Track};
use crate::shared::Battery;
use crate::store::{ClipItem, FocusSession, Store, Todo, WaterEntry};
use crate::theme::*;
use crate::util::now_ts;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn gradient_art() -> egui::ColorImage {
    let n = 160;
    let mut px = Vec::with_capacity(n * n);
    for y in 0..n {
        for x in 0..n {
            let fx = x as f32 / n as f32;
            let fy = y as f32 / n as f32;
            let sun = ((fx - 0.62).powi(2) + (fy - 0.55).powi(2)).sqrt();
            let (mut r, mut g, mut b) = (
                250.0 - 120.0 * fy,
                120.0 + 40.0 * fx - 60.0 * fy,
                90.0 + 120.0 * fy,
            );
            if sun < 0.16 {
                r = 255.0;
                g = 214.0;
                b = 120.0;
            }
            if fy > 0.7 {
                let w = ((fx * 40.0 + fy * 13.0).sin() * 0.5 + 0.5) * 30.0;
                r = 30.0 + w;
                g = 70.0 + w;
                b = 120.0 + w;
            }
            px.push(egui::Color32::from_rgb(r as u8, g as u8, b as u8));
        }
    }
    egui::ColorImage::new([n, n], px)
}

pub fn seed(app: &mut App) {
    // path = None → demo tidak pernah menyimpan ke disk
    let mut store = Store { path: None, ..Default::default() };
    let now = now_ts();
    let day = 86400;

    store.todos = vec![
        Todo { id: 1, text: "Kirim proposal landing page ke klien".into(), due: Some(now + 3600 + 1200), ..Default::default() },
        Todo { id: 2, text: "Review animasi hero section".into(), due: Some(now - 600), notified: true, ..Default::default() },
        Todo { id: 3, text: "Export aset Figma @2x".into(), ..Default::default() },
        Todo { id: 4, text: "Standup tim jam 10".into(), done: true, ..Default::default() },
    ];
    store.next_id = 10;
    for (i, ml) in [300, 600, 150].iter().enumerate() {
        store.water.push(WaterEntry { ts: now - 3600 * (3 - i as i64), ml: *ml });
    }
    for d in 1..7 {
        store.water.push(WaterEntry { ts: now - d * day, ml: [1800, 2100, 1500, 2400, 900, 2000][(d - 1) as usize] });
        store.focus_history.push(FocusSession { ts: now - d * day, minutes: [75, 100, 50, 125, 25, 0][(d - 1) as usize] });
    }
    store.focus_history.push(FocusSession { ts: now - 1800, minutes: 25 });
    store.focus_history.push(FocusSession { ts: now - 5400, minutes: 25 });

    let dir = std::env::temp_dir().join("nusa-demo");
    let _ = std::fs::create_dir_all(&dir);
    for f in ["proposal-landing-page.pdf", "hero-final@2x.png", "brief-klien.docx", "invoice-0921.xlsx", "motion-spec.fig"] {
        let p = dir.join(f);
        let _ = std::fs::write(&p, b"demo");
        store.add_shelf(p.to_string_lossy().into_owned());
    }
    store.clipboard = vec![
        ClipItem { text: "https://figma.com/file/aB3x/landing-v4".into(), ts: now },
        ClipItem { text: "Halo Kak, berikut revisi kedua untuk section pricing…".into(), ts: now },
        ClipItem { text: "#0F172A".into(), ts: now },
        ClipItem { text: "cubic-bezier(0.22, 1, 0.36, 1)".into(), ts: now },
    ];
    store.dirty = false;
    app.store = store;

    // Musik
    let track = Track {
        id: "demo-1".into(),
        title: "Kala Senja di Nusa".into(),
        artist: "Orkes Ombak".into(),
        album: "Pesisir".into(),
        duration: 214.0,
        position: 66.0,
        playing: std::env::var("NUSA_PAUSED").is_err(),
        ..Default::default()
    };
    // NUSA_BROWSER=1 → contoh pemutar YouTube di browser; NUSA_BROWSER=nojs → JavaScript belum diizinkan
    let browser = std::env::var("NUSA_BROWSER").ok();
    let (player, track) = match browser.as_deref() {
        Some(mode) => (
            Player::Browser,
            Track {
                id: "https://www.youtube.com/watch?v=demo".into(),
                artist: if mode == "nojs" { String::new() } else { "Orkes Ombak".into() },
                source: "YouTube · Chrome".into(),
                no_control: mode == "nojs",
                duration: if mode == "nojs" { 0.0 } else { track.duration },
                playing: mode != "nojs" && track.playing,
                ..track
            },
        ),
        None => (Player::Spotify, track),
    };
    let key = lyrics_key(&track);
    app.shared.set_player(
        player,
        Some(PlayerState { track, at: Instant::now(), art: Some(Arc::new(gradient_art())), art_key: "demo".into() }),
    );
    app.lyrics.insert(
        key,
        crate::app::LyricsState::Found(Lyrics {
            synced: vec![
                (52.0, "Angin pelan menyapa dermaga".into()),
                (58.5, "Perahu kecil pulang ke rumah".into()),
                (64.0, "Langit jingga menulis namamu".into()),
                (70.5, "Di riak air yang tenang".into()),
                (77.0, "Kita diam, tapi hati bicara".into()),
                (83.5, "Senja pun tahu kita tak sendiri".into()),
                (90.0, "".into()),
            ],
            plain: None,
        }),
    );

    app.shared.update(|g| g.battery = Some(Battery { percent: 76.0, plugged: true, charging: true }));

    let tab = std::env::var("NUSA_TAB").unwrap_or_default();
    app.tab = match tab.as_str() {
        "focus" => Tab::Focus,
        "water" => Tab::Water,
        "todo" => Tab::Todo,
        "shelf" => Tab::Shelf,
        "prayer" => Tab::Prayer,
        "settings" => Tab::Settings,
        _ => Tab::Music,
    };
    app.show_lyrics = tab == "lyrics";

    if std::env::var("NUSA_FOCUS").is_ok() || std::env::var("NUSA_EARS").as_deref() == Ok("focus") {
        app.store.settings.focus_min = 25;
        app.focus.done_in_cycle = 2;
        app.focus.remaining = 14.0 * 60.0 + 32.0;
        app.focus.start();
    }
    match std::env::var("NUSA_EARS").as_deref() {
        Ok("clock") | Ok("none") => {
            app.shared.set_player(Player::Spotify, None);
            if std::env::var("NUSA_EARS").as_deref() == Ok("none") {
                app.store.settings.show_clock_idle = false;
            }
        }
        _ => {}
    }
    let state = std::env::var("NUSA_STATE").unwrap_or_else(|_| "expanded".into());
    if state == "peek" {
        let kind = std::env::var("NUSA_PEEK").unwrap_or_default();
        match kind.as_str() {
            "water" => app.push_peek(PeekKind::Water, "Waktunya minum", "950 ml lagi menuju target", 3600.0, Some(Tab::Water), WATER),
            "prayer" => app.push_peek(PeekKind::Prayer, "Ashar 10 menit lagi", "15:03 · Jakarta", 3600.0, Some(Tab::Prayer), PRAYER),
            _ => app.push_peek(PeekKind::Track, "Kala Senja di Nusa", "Orkes Ombak", 3600.0, Some(Tab::Music), SPOTIFY),
        }
        app.set_demo_state(Some("peek".into()));
    } else if state != "auto" {
        app.set_demo_state(Some(state));
    }
    let _ = Duration::ZERO;
}
