use crate::app::{player_color, App, LyricsState};
use crate::media::{MediaCmd, Player};
use crate::theme::*;
use crate::util::fmt_mmss;
use egui::{pos2, vec2, Align2, Color32, CornerRadius, Id, Rect, Sense, Ui, Vec2};
use std::time::Instant;

pub fn show(app: &mut App, ui: &mut Ui, rect: Rect) {
    let Some(pl) = app.active_player() else {
        super::empty_state(
            ui.painter(),
            rect,
            Icon::Music,
            SPOTIFY,
            "Belum ada musik",
            "Putar lagu di Spotify atau Apple Music — Nusa akan menampilkannya di sini.",
        );
        return;
    };
    let st = app.players[&pl].clone();
    let pos = st.position_now();
    let dur = st.track.duration.max(1.0);
    if st.track.playing {
        app.request_wake(1.0 - pos.fract() + 0.02);
    }
    let lyrics_mode = app.show_lyrics && app.store.settings.lyrics;
    let p = ui.painter().clone();
    let color = player_color(pl);
    let both = app.players.len() > 1;

    let col_w = if lyrics_mode { 214.0 } else { rect.width() };
    let left = Rect::from_min_size(rect.min, vec2(col_w, rect.height()));

    // Sampul album — klik untuk ganti pemutar.
    let art_size = if lyrics_mode { 58.0 } else { 120.0 };
    let art = Rect::from_min_size(left.min + vec2(0.0, if lyrics_mode { 4.0 } else { 8.0 }), Vec2::splat(art_size));
    let art_resp = ui.interact(art, Id::unique("art"), Sense::click());
    if let Some((_, tex)) = app.art_tex.get(&pl) {
        egui::Image::new(egui::load::SizedTexture::new(tex.id(), art.size()))
            .corner_radius(CornerRadius::same(if lyrics_mode { 8 } else { 14 }))
            .paint_at(ui, art);
    } else {
        p.rect_filled(art, CornerRadius::same(12), color.gamma_multiply(0.25));
        paint_icon(&p, art.shrink(art_size * 0.3), Icon::Music, color);
    }
    if both {
        let resp = art_resp.on_hover_text("Ganti pemutar");
        if resp.clicked() {
            let other = Player::ALL.into_iter().find(|x| *x != pl && app.players.contains_key(x));
            app.pinned_player = other;
        }
    }

    // Info lagu
    let (tx, ty, tw) = if lyrics_mode {
        (art.max.x + 12.0, art.min.y + 2.0, left.max.x - art.max.x - 12.0)
    } else {
        (art.max.x + 18.0, art.min.y + 4.0, rect.max.x - art.max.x - 18.0)
    };
    text_trunc(&p, pos2(tx, ty), Align2::LEFT_TOP, &st.track.title, if lyrics_mode { 14.0 } else { 17.0 }, TEXT, tw, true);
    text_trunc(&p, pos2(tx, ty + if lyrics_mode { 20.0 } else { 25.0 }), Align2::LEFT_TOP, &st.track.artist, 12.5, DIM, tw, false);
    let src_y = ty + if lyrics_mode { 38.0 } else { 46.0 };
    p.circle_filled(pos2(tx + 4.0, src_y + 6.0), 3.5, color);
    let name = if st.track.source.is_empty() { pl.name().to_string() } else { st.track.source.clone() };
    let src = if both { format!("{name} · klik sampul untuk ganti") } else { name };
    let controllable = !st.track.no_control;
    let has_timeline = controllable && st.track.duration > 0.5;
    text_trunc(&p, pos2(tx + 12.0, src_y), Align2::LEFT_TOP, &src, 11.0, FAINT.gamma_multiply(1.6), tw - 12.0, false);

    // Progress + seek
    let (bx0, bx1, by) = if lyrics_mode {
        (left.min.x, left.max.x, art.max.y + 18.0)
    } else {
        (tx, rect.max.x, ty + 76.0)
    };
    if has_timeline {
        let bar_hit = Rect::from_min_max(pos2(bx0, by - 7.0), pos2(bx1, by + 7.0));
        let bar_resp = ui.interact(bar_hit, Id::unique("seek"), Sense::click_and_drag());
        let mut frac = (pos / dur) as f32;
        if let Some(ptr) = bar_resp.interact_pointer_pos() {
            if bar_resp.dragged() || bar_resp.is_pointer_button_down_on() {
                frac = ((ptr.x - bx0) / (bx1 - bx0)).clamp(0.0, 1.0);
            }
        }
        if bar_resp.clicked() || bar_resp.drag_stopped() {
            if let Some(ptr) = bar_resp.interact_pointer_pos() {
                let f = ((ptr.x - bx0) / (bx1 - bx0)).clamp(0.0, 1.0) as f64;
                seek(app, pl, f * dur);
                frac = f as f32;
            }
        }
        let thick = if bar_resp.hovered() || bar_resp.dragged() { 6.0 } else { 4.0 };
        progress_bar(&p, Rect::from_min_max(pos2(bx0, by - thick / 2.0), pos2(bx1, by + thick / 2.0)), frac, TEXT);
        text(&p, pos2(bx0, by + 8.0), Align2::LEFT_TOP, &fmt_mmss(frac as f64 * dur), 11.0, DIM);
        text(&p, pos2(bx1, by + 8.0), Align2::RIGHT_TOP, &format!("-{}", fmt_mmss(dur - frac as f64 * dur)), 11.0, DIM);
    } else if controllable {
        // siaran langsung / durasi tidak diketahui
        let col = if st.track.playing { YOUTUBE } else { DIM };
        p.circle_filled(pos2(bx0 + 4.0, by), 3.5, col);
        text(&p, pos2(bx0 + 12.0, by), Align2::LEFT_CENTER, "LIVE", 11.0, col);
    }

    if !controllable {
        // Hanya judul tab yang terbaca: jelaskan cara mengaktifkan kontrol.
        let hint = Rect::from_min_max(pos2(bx0, by - 8.0), pos2(bx1, rect.max.y));
        let galley = p.layout(
            "Hanya judul yang terbaca. Untuk kontrol, progres & sampul, aktifkan \"Allow JavaScript from Apple Events\" di menu Developer browser (Chrome: View > Developer; Safari: Settings > Developer).".to_string(),
            egui::FontId::proportional(11.5),
            DIM,
            hint.width(),
        );
        p.galley(hint.min, galley, DIM);
        return;
    }

    // Kontrol
    let cy = by + 46.0;
    let ctr_x = (bx0 + bx1) / 2.0;
    let mut ctl = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_center_size(pos2(ctr_x, cy), vec2(150.0, 42.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
    ctl.spacing_mut().item_spacing.x = 10.0;
    if icon_button(&mut ctl, Icon::Prev, 32.0, TEXT).clicked() {
        app.media(pl, MediaCmd::Prev);
    }
    let pp = if st.track.playing { Icon::Pause } else { Icon::Play };
    if icon_button(&mut ctl, pp, 42.0, TEXT).clicked() {
        app.media(pl, MediaCmd::PlayPause);
        if let Some(s) = app.players.get_mut(&pl) {
            s.track.position = s.position_now();
            s.at = Instant::now();
            s.track.playing = !s.track.playing;
        }
    }
    if icon_button(&mut ctl, Icon::Next, 32.0, TEXT).clicked() {
        app.media(pl, MediaCmd::Next);
    }

    if app.store.settings.lyrics {
        let lr = Rect::from_center_size(pos2(bx1 - 14.0, cy), Vec2::splat(30.0));
        let mut lui = ui.new_child(egui::UiBuilder::new().max_rect(lr));
        let c = if lyrics_mode { ACCENT } else { DIM };
        if icon_button(&mut lui, Icon::Lyrics, 30.0, c).on_hover_text("Lirik").clicked() {
            app.show_lyrics = !app.show_lyrics;
        }
    }

    if lyrics_mode {
        let lyr_rect = Rect::from_min_max(pos2(left.max.x + 18.0, rect.min.y), rect.max);
        p.line_segment(
            [pos2(left.max.x + 9.0, rect.min.y + 6.0), pos2(left.max.x + 9.0, rect.max.y - 6.0)],
            egui::Stroke::new(1.0, CARD_HI),
        );
        lyrics(app, ui, lyr_rect, pl, pos);
        // sinkron ulang posisi berkala selama lirik tampil
        if app_resync_due(app) {
            app.platform.refresh_media(pl);
        }
    }
}

fn app_resync_due(app: &mut App) -> bool {
    let id = Id::unique("lyrics-resync");
    let now = app.shared.ctx.input(|i| i.time);
    let last: f64 = app.shared.ctx.data(|d| d.get_temp(id)).unwrap_or(0.0);
    if now - last > 10.0 {
        app.shared.ctx.data_mut(|d| d.insert_temp(id, now));
        last > 0.0
    } else {
        false
    }
}

fn seek(app: &mut App, pl: Player, to: f64) {
    app.media(pl, MediaCmd::Seek(to));
    if let Some(s) = app.players.get_mut(&pl) {
        s.track.position = to;
        s.at = Instant::now();
    }
}

fn lyrics(app: &mut App, ui: &mut Ui, rect: Rect, pl: Player, pos: f64) {
    let p = ui.painter().clone();
    let state = app.current_lyrics();
    let msg = match state {
        None | Some(LyricsState::Loading) => Some("Mencari lirik…"),
        Some(LyricsState::Missing) => Some("Lirik tidak ditemukan untuk lagu ini"),
        Some(LyricsState::Found(l)) if l.synced.is_empty() => None,
        _ => None,
    };
    if let Some(m) = msg {
        text(&p, rect.center(), Align2::CENTER_CENTER, m, 13.0, DIM);
        return;
    }
    let Some(LyricsState::Found(l)) = state else { return };
    if l.synced.is_empty() {
        // hanya lirik biasa
        let plain = l.plain.clone().unwrap_or_default();
        let mut sui = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        egui::ScrollArea::vertical().max_height(rect.height()).show(&mut sui, |ui| {
            ui.label(egui::RichText::new(plain).color(DIM).size(12.5));
        });
        return;
    }
    let lines = l.synced.clone();
    let cur = l.current_index(pos);
    let next_in = l.next_change_after(pos);
    let line_h = 27.0;
    let center = cur.unwrap_or(0) as isize;
    let first = (center - 2).max(0) as usize;
    let mut seek_to = None;
    for (row, i) in (first..lines.len()).enumerate() {
        let y = rect.min.y + 8.0 + row as f32 * line_h;
        if y + line_h > rect.max.y + 4.0 {
            break;
        }
        let (t, ref txt) = lines[i];
        let is_cur = Some(i) == cur;
        let r = Rect::from_min_size(pos2(rect.min.x, y), vec2(rect.width(), line_h));
        let resp = ui.interact(r, Id::unique(("lyric", i)), Sense::click());
        if resp.hovered() {
            p.rect_filled(r, CornerRadius::same(6), CARD);
        }
        let (size, col) = if is_cur {
            (15.0, TEXT)
        } else if cur.is_some_and(|c| i < c) {
            (13.0, FAINT.gamma_multiply(1.5))
        } else {
            (13.0, DIM)
        };
        let shown = if txt.is_empty() { "♪" } else { txt.as_str() };
        text_trunc(&p, pos2(r.min.x + 6.0, r.center().y), Align2::LEFT_CENTER, shown, size, col, r.width() - 12.0, is_cur);
        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            seek_to = Some(t);
        }
    }
    if let Some(t) = seek_to {
        seek(app, pl, t);
    }
    if let Some(dt) = next_in {
        if app.players.get(&pl).is_some_and(|s| s.track.playing) {
            app.request_wake(dt + 0.02);
        }
    }
    let _ = Color32::WHITE;
}
