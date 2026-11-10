use super::fmt_thousands;
use crate::app::{ts_date, App};
use crate::theme::*;
use egui::{pos2, vec2, Align2, Rect, Ui};

pub fn show(app: &mut App, ui: &mut Ui, rect: Rect) {
    let p = ui.painter().clone();
    let s = app.store.settings.clone();
    let total = app.water_today();
    let goal = s.water_goal_ml.max(1);
    let frac = total as f32 / goal as f32;

    let c = pos2(rect.min.x + 76.0, rect.center().y - 4.0);
    ring(&p, c, 64.0, 7.0, frac, WATER);
    paint_icon(&p, Rect::from_center_size(c - vec2(0.0, 30.0), egui::Vec2::splat(14.0)), Icon::Water, WATER);
    strong(&p, c - vec2(0.0, 4.0), Align2::CENTER_CENTER, &fmt_thousands(total), 26.0, TEXT);
    text(&p, c + vec2(0.0, 20.0), Align2::CENTER_CENTER, &format!("dari {} ml", fmt_thousands(goal)), 11.5, DIM);

    let x0 = c.x + 92.0;
    text(&p, pos2(x0, rect.min.y + 8.0), Align2::LEFT_TOP, &format!("Botol {} ml", s.water_bottle_ml), 11.0, DIM);
    let btn_rect = Rect::from_min_size(pos2(x0, rect.min.y + 26.0), vec2(rect.max.x - x0, 28.0));
    let mut bui = ui.new_child(egui::UiBuilder::new().max_rect(btn_rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
    let b = s.water_bottle_ml;
    for (label, ml) in [("+¼", b / 4), ("+½", b / 2), ("+1 botol", b)] {
        if pill(&mut bui, label, WATER, label == "+1 botol").on_hover_text(format!("{ml} ml")).clicked() {
            app.add_water(ml);
        }
    }
    let today = chrono::Local::now().date_naive();
    let has_today = app.store.water.last().is_some_and(|w| ts_date(w.ts) == Some(today));
    if has_today && pill(&mut bui, "Batal", DIM, false).clicked() {
        app.store.water.pop();
        app.store.touch();
    }

    // status pengingat
    let status = if !s.water_reminders {
        "Pengingat mati".to_string()
    } else if total >= goal {
        "Target hari ini tercapai".to_string()
    } else {
        format!("Diingatkan tiap {} menit, {:02}.00–{:02}.00", s.water_interval_min, s.water_start_h, s.water_end_h)
    };
    text(&p, pos2(x0, rect.min.y + 66.0), Align2::LEFT_TOP, &status, 11.5, if total >= goal { BREAK } else { DIM });

    let days = super::last7();
    let vals: Vec<(f32, &str)> = days
        .iter()
        .map(|(d, l)| {
            let ml: u32 = app.store.water.iter().filter(|w| ts_date(w.ts) == Some(*d)).map(|w| w.ml).sum();
            (ml as f32, *l)
        })
        .collect();
    let chart = Rect::from_min_max(pos2(x0, rect.min.y + 92.0), pos2(rect.max.x.min(x0 + 260.0), rect.max.y - 2.0));
    super::bar_chart(&p, chart, &vals, Some(goal as f32), WATER);
}
