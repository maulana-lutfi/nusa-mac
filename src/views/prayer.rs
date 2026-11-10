use crate::app::{local_hours_now, App};
use crate::prayer::{fmt_hhmm, NAMES};
use crate::theme::*;
use crate::util::fmt_hm_duration;
use egui::{pos2, vec2, Align2, CornerRadius, Rect, Shape, Stroke, Ui};
use std::f32::consts::PI;

pub fn show(app: &mut App, ui: &mut Ui, rect: Rect) {
    let p = ui.painter().clone();
    let Some(t) = app.prayer_today() else {
        text(&p, rect.center(), Align2::CENTER_CENTER, "Waktu sholat dimatikan di Setelan", 12.5, DIM);
        return;
    };
    let next = app.next_prayer();
    let now = local_hours_now() as f32;

    // Busur matahari dari terbit ke terbenam
    let arc_r = Rect::from_min_max(rect.min + vec2(10.0, 30.0), pos2(rect.max.x - 10.0, rect.min.y + 100.0));
    let (rise, set) = (t.sunrise as f32, t.sunset as f32);
    let at = |h: f32| {
        let f = ((h - rise) / (set - rise)).clamp(0.0, 1.0);
        pos2(
            arc_r.min.x + arc_r.width() * f,
            arc_r.max.y - (f * PI).sin() * arc_r.height(),
        )
    };
    let pts: Vec<_> = (0..=48).map(|i| at(rise + (set - rise) * i as f32 / 48.0)).collect();
    p.extend(Shape::dashed_line(&pts, Stroke::new(1.4, FAINT.gamma_multiply(1.4)), 4.0, 4.0));
    p.line_segment([pos2(rect.min.x, arc_r.max.y), pos2(rect.max.x, arc_r.max.y)], Stroke::new(1.0, CARD_HI));
    // titik Dzuhur & Ashar di busur
    for h in [t.dhuhr as f32, t.asr as f32] {
        p.circle_filled(at(h), 3.0, PRAYER.gamma_multiply(0.5));
    }
    if now >= rise && now <= set {
        let sp = at(now);
        p.circle_filled(sp, 11.0, PRAYER.gamma_multiply(0.18));
        p.circle_filled(sp, 6.5, PRAYER);
    } else {
        paint_icon(&p, Rect::from_center_size(pos2(arc_r.max.x - 10.0, arc_r.min.y + 4.0), egui::Vec2::splat(16.0)), Icon::Prayer, PRAYER);
    }
    text(&p, pos2(arc_r.min.x, arc_r.max.y + 4.0), Align2::LEFT_TOP, &format!("Terbit {}", fmt_hhmm(t.sunrise)), 10.5, DIM);
    text(&p, pos2(arc_r.max.x, arc_r.max.y + 4.0), Align2::RIGHT_TOP, &format!("Terbenam {}", fmt_hhmm(t.sunset)), 10.5, DIM);

    // judul: kota + hitung mundur
    text(&p, rect.min + vec2(2.0, 2.0), Align2::LEFT_TOP, &app.store.settings.prayer_city, 12.5, TEXT);
    if let Some((i, left)) = next {
        text(
            &p,
            pos2(rect.max.x - 2.0, rect.min.y + 2.0),
            Align2::RIGHT_TOP,
            &format!("{} dalam {}", NAMES[i], fmt_hm_duration(left)),
            12.5,
            PRAYER,
        );
        app.request_wake(((left % 60) + 1) as f64);
    }

    // lima waktu
    let row = Rect::from_min_max(pos2(rect.min.x, rect.max.y - 52.0), rect.max);
    let w = row.width() / 5.0;
    for (i, h) in t.prayers().iter().enumerate() {
        let cell = Rect::from_min_size(pos2(row.min.x + w * i as f32, row.min.y), vec2(w, row.height())).shrink2(vec2(3.0, 0.0));
        let is_next = next.is_some_and(|(n, _)| n == i);
        let passed = (*h as f32) < now && !is_next;
        if is_next {
            p.rect_filled(cell, CornerRadius::same(10), PRAYER.gamma_multiply(0.14));
        } else {
            p.rect_filled(cell, CornerRadius::same(10), CARD);
        }
        let name_c = if is_next { PRAYER } else { DIM };
        let time_c = if passed { DIM } else { TEXT };
        text(&p, cell.center() - vec2(0.0, 10.0), Align2::CENTER_CENTER, NAMES[i], 11.0, name_c);
        strong(&p, cell.center() + vec2(0.0, 9.0), Align2::CENTER_CENTER, &fmt_hhmm(*h), 15.0, time_c);
    }
}
