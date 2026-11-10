use crate::app::{ts_date, App, FocusTimer, Phase};
use crate::theme::*;
use crate::util::fmt_mmss;
use egui::{pos2, vec2, Align2, Rect, Ui};

pub fn show(app: &mut App, ui: &mut Ui, rect: Rect) {
    let p = ui.painter().clone();
    if app.focus.running {
        let rem = app.focus.remaining_now();
        app.request_wake(rem.fract() + 0.02);
    }
    let f = &app.focus;
    let rem = f.remaining_now();
    let frac = (1.0 - rem / f.total.max(1.0)) as f32;
    let color = f.phase.color();

    // Cincin waktu
    let c = pos2(rect.min.x + 76.0, rect.center().y - 4.0);
    ring(&p, c, 64.0, 7.0, frac, color);
    strong(&p, c - vec2(0.0, 6.0), Align2::CENTER_CENTER, &fmt_mmss(rem), 28.0, TEXT);
    text(&p, c + vec2(0.0, 20.0), Align2::CENTER_CENTER, f.phase.label(), 12.0, color);

    let x0 = c.x + 92.0;
    let s = app.store.settings.clone();

    // Ronde
    text(&p, pos2(x0, rect.min.y + 8.0), Align2::LEFT_TOP, "Ronde", 11.0, DIM);
    let rounds = s.rounds.max(1);
    for i in 0..rounds {
        let dc = pos2(x0 + 6.0 + i as f32 * 20.0, rect.min.y + 34.0);
        let done = i < f.done_in_cycle;
        let current = i == f.done_in_cycle && f.phase == Phase::Focus;
        if done {
            p.circle_filled(dc, 6.0, FOCUS);
        } else if current {
            p.circle_stroke(dc, 6.0, egui::Stroke::new(2.0, FOCUS));
            p.circle_filled(dc, 6.0 * frac.clamp(0.0, 1.0), FOCUS.gamma_multiply(0.6));
        } else {
            p.circle_stroke(dc, 6.0, egui::Stroke::new(1.5, FAINT));
        }
    }
    let lb = pos2(x0 + rounds as f32 * 20.0 + 6.0, rect.min.y + 34.0);
    let long = f.phase == Phase::LongBreak;
    p.circle_filled(lb, 4.0, if long { BREAK } else { FAINT });

    // Tombol
    let btn_rect = Rect::from_min_size(pos2(x0, rect.min.y + 54.0), vec2(rect.max.x - x0, 28.0));
    let mut bui = ui.new_child(egui::UiBuilder::new().max_rect(btn_rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
    let running = app.focus.running;
    let label = if running { "Jeda" } else if (app.focus.remaining - app.focus.total).abs() < 0.5 { "Mulai" } else { "Lanjut" };
    if pill(&mut bui, label, color, true).clicked() {
        if running {
            app.focus.pause();
        } else {
            app.focus.start();
        }
    }
    if pill(&mut bui, "Reset", DIM, false).clicked() {
        app.focus = FocusTimer::new(&s);
    }
    if pill(&mut bui, "Lewati", DIM, false).clicked() {
        app.focus.advance(&s);
    }

    // Statistik
    let today = chrono::Local::now().date_naive();
    let hist = &app.store.focus_history;
    let today_min: u32 = hist.iter().filter(|h| ts_date(h.ts) == Some(today)).map(|h| h.minutes).sum();
    let today_n = hist.iter().filter(|h| ts_date(h.ts) == Some(today)).count();
    text(
        &p,
        pos2(x0, rect.min.y + 96.0),
        Align2::LEFT_TOP,
        &format!("Hari ini {today_min} menit · {today_n} sesi"),
        12.0,
        TEXT,
    );
    let days = super::last7();
    let vals: Vec<(f32, &str)> = days
        .iter()
        .map(|(d, l)| {
            let m: u32 = hist.iter().filter(|h| ts_date(h.ts) == Some(*d)).map(|h| h.minutes).sum();
            (m as f32, *l)
        })
        .collect();
    let chart = Rect::from_min_max(pos2(x0, rect.min.y + 118.0), pos2(rect.max.x.min(x0 + 230.0), rect.max.y - 2.0));
    super::bar_chart(&p, chart, &vals, None, FOCUS);
    text(
        &p,
        pos2(rect.max.x, rect.max.y - 2.0),
        Align2::RIGHT_BOTTOM,
        &format!("{} / {} / {} mnt", s.focus_min, s.short_break_min, s.long_break_min),
        10.5,
        FAINT.gamma_multiply(1.6),
    );
}
