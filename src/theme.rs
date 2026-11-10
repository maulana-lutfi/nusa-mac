//! Warna, ikon vektor (digambar langsung, tanpa file gambar), dan helper teks.

use egui::{
    text::{LayoutJob, TextFormat, TextWrapping},
    Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Response, Sense, Shape, Stroke,
    Ui, Vec2,
};
use std::f32::consts::{PI, TAU};

pub const BG: Color32 = Color32::from_rgb(0, 0, 0);
pub const CARD: Color32 = Color32::from_rgb(22, 22, 25);
pub const CARD_HI: Color32 = Color32::from_rgb(34, 34, 38);
pub const TEXT: Color32 = Color32::from_rgb(242, 242, 245);
pub const DIM: Color32 = Color32::from_rgb(142, 142, 150);
pub const FAINT: Color32 = Color32::from_rgb(72, 72, 78);
pub const ACCENT: Color32 = Color32::from_rgb(64, 214, 196); // teal "nusa"
pub const SPOTIFY: Color32 = Color32::from_rgb(30, 215, 96);
pub const MUSIC: Color32 = Color32::from_rgb(250, 45, 72);
pub const YOUTUBE: Color32 = Color32::from_rgb(255, 78, 69);
pub const WATER: Color32 = Color32::from_rgb(74, 168, 255);
pub const FOCUS: Color32 = Color32::from_rgb(255, 159, 10);
pub const BREAK: Color32 = Color32::from_rgb(48, 209, 88);
pub const PRAYER: Color32 = Color32::from_rgb(255, 214, 10);
pub const TODO: Color32 = Color32::from_rgb(191, 90, 242);
pub const DANGER: Color32 = Color32::from_rgb(255, 69, 58);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)]
pub enum Icon {
    Music,
    Timer,
    Water,
    Todo,
    Shelf,
    Prayer,
    Gear,
    Play,
    Pause,
    Next,
    Prev,
    Lyrics,
    Plus,
    Close,
    Up,
    Down,
    Folder,
    Copy,
    Check,
    Pencil,
    Bolt,
    Reset,
    Skip,
    Clipboard,
    Undo,
}

fn line(p: &Painter, pts: &[Pos2], s: Stroke) {
    p.add(Shape::line(pts.to_vec(), s));
}

pub fn arc_points(c: Pos2, r: f32, a0: f32, a1: f32) -> Vec<Pos2> {
    let n = (((a1 - a0).abs() * r) / 2.5).ceil().max(6.0) as usize;
    (0..=n)
        .map(|i| {
            let a = a0 + (a1 - a0) * (i as f32 / n as f32);
            Pos2::new(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect()
}

pub fn arc(p: &Painter, c: Pos2, r: f32, a0: f32, a1: f32, s: Stroke) {
    p.add(Shape::line(arc_points(c, r, a0, a1), s));
}

/// Cincin progres (0..1), mulai dari atas searah jarum jam.
pub fn ring(p: &Painter, c: Pos2, r: f32, width: f32, frac: f32, color: Color32) {
    p.circle_stroke(c, r, Stroke::new(width, color.gamma_multiply(0.22)));
    let f = frac.clamp(0.0, 1.0);
    if f > 0.001 {
        arc(p, c, r, -PI / 2.0, -PI / 2.0 + TAU * f, Stroke::new(width, color));
    }
}

pub fn paint_icon(p: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let c = rect.center();
    let s = rect.width().min(rect.height());
    let u = s / 16.0; // unit grid 16x16
    let pt = |x: f32, y: f32| Pos2::new(c.x + (x - 8.0) * u, c.y + (y - 8.0) * u);
    let st = Stroke::new((1.5 * u).max(1.2), color);
    match icon {
        Icon::Music => {
            p.circle_filled(pt(5.0, 12.0), 2.2 * u, color);
            p.circle_filled(pt(12.0, 10.5), 2.2 * u, color);
            line(p, &[pt(7.0, 12.0), pt(7.0, 3.5)], st);
            line(p, &[pt(14.0, 10.5), pt(14.0, 2.0)], st);
            line(p, &[pt(7.0, 3.5), pt(14.0, 2.0)], Stroke::new(2.2 * u, color));
        }
        Icon::Timer => {
            p.circle_stroke(pt(8.0, 9.0), 5.5 * u, st);
            line(p, &[pt(8.0, 9.0), pt(8.0, 5.8)], st);
            line(p, &[pt(8.0, 9.0), pt(10.3, 10.3)], st);
            line(p, &[pt(6.5, 1.8), pt(9.5, 1.8)], st);
        }
        Icon::Water => {
            let mut pts = vec![pt(8.0, 1.5)];
            pts.extend(arc_points(pt(8.0, 10.0), 4.6 * u, -0.25 * PI, 1.25 * PI));
            p.add(Shape::convex_polygon(pts, color, Stroke::NONE));
        }
        Icon::Todo => {
            p.rect_stroke(
                Rect::from_min_max(pt(2.5, 2.5), pt(13.5, 13.5)),
                CornerRadius::same((3.0 * u) as u8),
                st,
                egui::StrokeKind::Middle,
            );
            line(p, &[pt(5.0, 8.2), pt(7.2, 10.5), pt(11.2, 5.6)], st);
        }
        Icon::Shelf => {
            line(p, &[pt(2.0, 8.0), pt(2.0, 13.5), pt(14.0, 13.5), pt(14.0, 8.0)], st);
            line(p, &[pt(5.0, 10.0), pt(11.0, 10.0)], st);
            line(p, &[pt(8.0, 1.5), pt(8.0, 7.0)], st);
            line(p, &[pt(5.5, 4.8), pt(8.0, 7.2), pt(10.5, 4.8)], st);
        }
        Icon::Prayer => {
            // bulan sabit
            p.circle_filled(pt(8.0, 8.0), 6.0 * u, color);
            p.circle_filled(pt(10.5, 6.2), 5.0 * u, BG);
            p.circle_filled(pt(12.2, 11.6), 1.0 * u, color);
        }
        Icon::Gear => {
            for i in 0..8 {
                let a = i as f32 * TAU / 8.0;
                let (sn, cs) = a.sin_cos();
                line(
                    p,
                    &[
                        pt(8.0 + 4.0 * cs, 8.0 + 4.0 * sn),
                        pt(8.0 + 6.6 * cs, 8.0 + 6.6 * sn),
                    ],
                    Stroke::new(2.4 * u, color),
                );
            }
            p.circle_stroke(pt(8.0, 8.0), 4.2 * u, st);
            p.circle_filled(pt(8.0, 8.0), 1.6 * u, color);
        }
        Icon::Play => {
            p.add(Shape::convex_polygon(
                vec![pt(4.5, 2.5), pt(13.5, 8.0), pt(4.5, 13.5)],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Pause => {
            let r = CornerRadius::same((u) as u8);
            p.rect_filled(Rect::from_min_max(pt(4.0, 2.5), pt(7.0, 13.5)), r, color);
            p.rect_filled(Rect::from_min_max(pt(9.0, 2.5), pt(12.0, 13.5)), r, color);
        }
        Icon::Next | Icon::Prev => {
            let m = |x: f32| if icon == Icon::Prev { 16.0 - x } else { x };
            p.add(Shape::convex_polygon(
                vec![pt(m(2.0), 3.0), pt(m(8.5), 8.0), pt(m(2.0), 13.0)],
                color,
                Stroke::NONE,
            ));
            p.add(Shape::convex_polygon(
                vec![pt(m(7.5), 3.0), pt(m(14.0), 8.0), pt(m(7.5), 13.0)],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Lyrics => {
            line(p, &[pt(2.0, 4.0), pt(14.0, 4.0)], st);
            line(p, &[pt(2.0, 8.0), pt(11.0, 8.0)], st);
            line(p, &[pt(2.0, 12.0), pt(8.0, 12.0)], st);
            p.circle_filled(pt(12.5, 12.0), 1.8 * u, color);
        }
        Icon::Plus => {
            line(p, &[pt(8.0, 2.5), pt(8.0, 13.5)], st);
            line(p, &[pt(2.5, 8.0), pt(13.5, 8.0)], st);
        }
        Icon::Close => {
            line(p, &[pt(3.5, 3.5), pt(12.5, 12.5)], st);
            line(p, &[pt(12.5, 3.5), pt(3.5, 12.5)], st);
        }
        Icon::Up => line(p, &[pt(3.5, 10.5), pt(8.0, 5.5), pt(12.5, 10.5)], st),
        Icon::Down => line(p, &[pt(3.5, 5.5), pt(8.0, 10.5), pt(12.5, 5.5)], st),
        Icon::Folder => {
            line(
                p,
                &[
                    pt(1.5, 4.0),
                    pt(6.0, 4.0),
                    pt(7.5, 5.5),
                    pt(14.5, 5.5),
                    pt(14.5, 13.0),
                    pt(1.5, 13.0),
                    pt(1.5, 4.0),
                ],
                st,
            );
        }
        Icon::Copy => {
            p.rect_stroke(
                Rect::from_min_max(pt(5.5, 5.5), pt(13.5, 14.0)),
                CornerRadius::same((1.5 * u) as u8),
                st,
                egui::StrokeKind::Middle,
            );
            line(p, &[pt(2.5, 11.0), pt(2.5, 2.0), pt(10.0, 2.0)], st);
        }
        Icon::Check => line(p, &[pt(3.0, 8.5), pt(6.5, 12.0), pt(13.0, 4.0)], st),
        Icon::Pencil => {
            line(p, &[pt(3.0, 13.0), pt(3.6, 10.2), pt(11.0, 2.8), pt(13.2, 5.0), pt(5.8, 12.4), pt(3.0, 13.0)], st);
            line(p, &[pt(9.4, 4.4), pt(11.6, 6.6)], st);
        }
        Icon::Bolt => {
            p.add(Shape::convex_polygon(
                vec![pt(9.5, 1.0), pt(3.5, 9.0), pt(8.0, 9.0)],
                color,
                Stroke::NONE,
            ));
            p.add(Shape::convex_polygon(
                vec![pt(8.0, 7.0), pt(12.5, 7.0), pt(6.5, 15.0)],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Reset | Icon::Undo => {
            arc(p, pt(8.0, 8.5), 5.0 * u, -0.9 * PI, 0.75 * PI, st);
            let tip = pt(8.0 + 5.0 * (-0.9 * PI).cos(), 8.5 + 5.0 * (-0.9 * PI).sin());
            line(p, &[Pos2::new(tip.x - 0.5 * u, tip.y - 3.5 * u), tip, Pos2::new(tip.x + 3.2 * u, tip.y + 0.2 * u)], st);
        }
        Icon::Skip => {
            p.add(Shape::convex_polygon(
                vec![pt(3.0, 3.0), pt(10.5, 8.0), pt(3.0, 13.0)],
                color,
                Stroke::NONE,
            ));
            line(p, &[pt(12.5, 3.0), pt(12.5, 13.0)], Stroke::new(2.2 * u, color));
        }
        Icon::Clipboard => {
            p.rect_stroke(
                Rect::from_min_max(pt(3.0, 3.0), pt(13.0, 14.5)),
                CornerRadius::same((2.0 * u) as u8),
                st,
                egui::StrokeKind::Middle,
            );
            p.rect_filled(
                Rect::from_min_max(pt(5.5, 1.5), pt(10.5, 4.5)),
                CornerRadius::same(u as u8),
                color,
            );
            line(p, &[pt(5.5, 8.0), pt(10.5, 8.0)], st);
            line(p, &[pt(5.5, 11.0), pt(9.0, 11.0)], st);
        }
    }
}

/// Tombol ikon bundar dengan efek hover.
pub fn icon_button(ui: &mut Ui, icon: Icon, size: f32, color: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        if resp.hovered() {
            p.circle_filled(rect.center(), size * 0.5, CARD_HI);
        }
        let inner = rect.shrink(size * 0.24);
        let col = if resp.is_pointer_button_down_on() {
            color.gamma_multiply(0.7)
        } else {
            color
        };
        paint_icon(p, inner, icon, col);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Tombol "pil" teks.
pub fn pill(ui: &mut Ui, text: &str, color: Color32, filled: bool) -> Response {
    let font = FontId::proportional(12.0);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font, if filled { BG } else { color });
    let size = Vec2::new(galley.size().x + 18.0, 24.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let mut fill = if filled { color } else { CARD_HI };
        if resp.hovered() {
            fill = if filled {
                color.gamma_multiply(0.85)
            } else {
                Color32::from_rgb(48, 48, 54)
            };
        }
        p.rect_filled(rect, CornerRadius::same(12), fill);
        p.galley(
            rect.center() - galley.size() / 2.0,
            galley,
            TEXT,
        );
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn text(p: &Painter, pos: Pos2, align: Align2, s: &str, size: f32, color: Color32) -> Rect {
    p.text(pos, align, s, FontId::proportional(size), color)
}

/// Teks tebal palsu (digambar dua kali bergeser 0.45px) — font sistem tidak punya bobot bold terpisah.
pub fn strong(p: &Painter, pos: Pos2, align: Align2, s: &str, size: f32, color: Color32) -> Rect {
    p.text(pos + Vec2::new(0.45, 0.0), align, s, FontId::proportional(size), color);
    p.text(pos, align, s, FontId::proportional(size), color)
}

/// Teks satu baris yang dipotong "…" bila melebihi `max_w`.
#[allow(clippy::too_many_arguments)]
pub fn text_trunc(
    p: &Painter,
    pos: Pos2,
    align: Align2,
    s: &str,
    size: f32,
    color: Color32,
    max_w: f32,
    bold: bool,
) -> Rect {
    let mut job = LayoutJob::single_section(
        s.to_string(),
        TextFormat::simple(FontId::proportional(size), color),
    );
    job.wrap = TextWrapping::truncate_at_width(max_w.max(10.0));
    let galley = p.layout_job(job);
    let rect = align.anchor_size(pos, galley.size());
    if bold {
        p.galley(rect.min + Vec2::new(0.45, 0.0), galley.clone(), color);
    }
    p.galley(rect.min, galley, color);
    rect
}

pub fn progress_bar(p: &Painter, rect: Rect, frac: f32, color: Color32) {
    let r = CornerRadius::same((rect.height() / 2.0) as u8);
    p.rect_filled(rect, r, FAINT);
    let mut f = rect;
    f.max.x = rect.min.x + rect.width() * frac.clamp(0.0, 1.0);
    if f.width() > 0.5 {
        p.rect_filled(f, r, color);
    }
}

/// Menggambar "bahu" cekung kecil di pojok atas notch supaya menyatu dengan tepi layar.
pub fn shoulder(p: &Painter, corner: Pos2, r: f32, left: bool, color: Color32) {
    if r < 0.5 {
        return;
    }
    // Pusat lingkaran ada di luar badan notch.
    let dir = if left { -1.0 } else { 1.0 };
    let center = Pos2::new(corner.x + dir * r, corner.y + r);
    let (a0, a1) = if left { (0.0, -PI / 2.0) } else { (PI, 1.5 * PI) };
    let arc = arc_points(center, r, a0, a1);
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(corner, color);
    for q in &arc {
        mesh.colored_vertex(*q, color);
    }
    for i in 1..arc.len() as u32 {
        mesh.add_triangle(0, i, i + 1);
    }
    p.add(Shape::mesh(mesh));
}

pub fn dashed_rect(p: &Painter, rect: Rect, color: Color32) {
    let s = Stroke::new(1.2, color);
    let pts = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
        rect.left_top(),
    ];
    for w in pts.windows(2) {
        p.extend(Shape::dashed_line(&[w[0], w[1]], s, 5.0, 4.0));
    }
}

