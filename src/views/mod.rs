pub mod focus;
pub mod music;
pub mod prayer;
pub mod settings;
pub mod shelf;
pub mod todo;
pub mod water;

use crate::theme::*;
use chrono::{Datelike, Local};
use egui::{pos2, vec2, Align2, Color32, CornerRadius, Painter, Rect, Stroke};

pub const DAYS: [&str; 7] = ["Sen", "Sel", "Rab", "Kam", "Jum", "Sab", "Min"];

/// 7 hari terakhir (lama → hari ini) dengan label hari.
pub fn last7() -> Vec<(chrono::NaiveDate, &'static str)> {
    let today = Local::now().date_naive();
    (0..7)
        .rev()
        .map(|i| {
            let d = today - chrono::Duration::days(i);
            (d, DAYS[d.weekday().num_days_from_monday() as usize])
        })
        .collect()
}

/// Grafik batang mini dengan garis target opsional.
pub fn bar_chart(p: &Painter, rect: Rect, values: &[(f32, &str)], goal: Option<f32>, color: Color32) {
    let n = values.len().max(1) as f32;
    let max = values
        .iter()
        .map(|v| v.0)
        .fold(goal.unwrap_or(0.0), f32::max)
        .max(1.0);
    let label_h = 13.0;
    let chart = Rect::from_min_max(rect.min, pos2(rect.max.x, rect.max.y - label_h));
    let slot = chart.width() / n;
    let bw = (slot * 0.5).min(14.0);
    if let Some(g) = goal {
        let y = chart.max.y - chart.height() * (g / max);
        p.extend(egui::Shape::dashed_line(
            &[pos2(chart.min.x, y), pos2(chart.max.x, y)],
            Stroke::new(1.0, FAINT),
            3.0,
            3.0,
        ));
    }
    for (i, (v, label)) in values.iter().enumerate() {
        let cx = chart.min.x + slot * (i as f32 + 0.5);
        let h = (chart.height() * (v / max)).max(2.0);
        let last = i + 1 == values.len();
        let c = if last { color } else { color.gamma_multiply(0.45) };
        p.rect_filled(
            Rect::from_min_max(pos2(cx - bw / 2.0, chart.max.y - h), pos2(cx + bw / 2.0, chart.max.y)),
            CornerRadius::same(3),
            c,
        );
        text(p, pos2(cx, rect.max.y), Align2::CENTER_BOTTOM, label, 10.0, if last { TEXT } else { DIM });
    }
}

/// 1250 → "1.250"
pub fn fmt_thousands(n: u32) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(ch);
    }
    out
}

pub fn empty_state(p: &Painter, rect: Rect, icon: Icon, color: Color32, title: &str, sub: &str) {
    let c = rect.center();
    p.circle_filled(c - vec2(0.0, 30.0), 22.0, color.gamma_multiply(0.15));
    paint_icon(p, Rect::from_center_size(c - vec2(0.0, 30.0), egui::Vec2::splat(22.0)), icon, color);
    strong(p, c + vec2(0.0, 6.0), Align2::CENTER_CENTER, title, 14.0, TEXT);
    let galley = p.layout(
        sub.to_string(),
        egui::FontId::proportional(12.0),
        DIM,
        rect.width() * 0.8,
    );
    let pos = pos2(c.x - galley.size().x / 2.0, c.y + 18.0);
    p.galley(pos, galley, DIM);
}
