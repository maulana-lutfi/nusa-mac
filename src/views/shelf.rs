use crate::app::App;
use crate::platform::DragPayload;
use crate::theme::*;
use egui::{pos2, vec2, Align2, Color32, CornerRadius, Id, Rect, Sense, Ui};
use std::path::Path;

enum Action {
    Open(String),
    Reveal(String),
    CopyFile(String),
    Remove(String),
    Drag(DragPayload),
    CopyText(String),
    RemoveClip(usize),
}

fn ext_color(ext: &str) -> Color32 {
    match ext {
        "pdf" => Color32::from_rgb(235, 87, 87),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "heic" | "svg" => Color32::from_rgb(86, 204, 242),
        "doc" | "docx" | "pages" | "txt" | "md" => Color32::from_rgb(47, 128, 237),
        "xls" | "xlsx" | "csv" | "numbers" => Color32::from_rgb(39, 174, 96),
        "ppt" | "pptx" | "key" => Color32::from_rgb(242, 153, 74),
        "zip" | "dmg" | "rar" | "7z" => Color32::from_rgb(155, 81, 224),
        "fig" | "sketch" | "psd" | "ai" => Color32::from_rgb(162, 89, 255),
        "mp4" | "mov" | "mp3" | "wav" => Color32::from_rgb(242, 201, 76),
        _ => Color32::from_rgb(130, 130, 140),
    }
}

pub fn show(app: &mut App, ui: &mut Ui, rect: Rect) {
    let split = rect.min.x + rect.width() * 0.58;
    let shelf_rect = Rect::from_min_max(rect.min, pos2(split - 8.0, rect.max.y));
    let clip_rect = Rect::from_min_max(pos2(split + 8.0, rect.min.y), rect.max);
    let p = ui.painter().clone();
    p.line_segment([pos2(split, rect.min.y + 4.0), pos2(split, rect.max.y - 4.0)], egui::Stroke::new(1.0, CARD_HI));
    let mut actions: Vec<Action> = Vec::new();

    // ---------------- Shelf
    strong(&p, shelf_rect.left_top() + vec2(2.0, 2.0), Align2::LEFT_TOP, "Shelf", 12.5, TEXT);
    let n = app.store.shelf.len();
    if n > 0 {
        text(&p, shelf_rect.left_top() + vec2(44.0, 3.0), Align2::LEFT_TOP, &n.to_string(), 11.5, DIM);
        let cr = Rect::from_min_size(pos2(shelf_rect.max.x - 64.0, shelf_rect.min.y), vec2(64.0, 18.0));
        let resp = ui.interact(cr, Id::unique("shelf-clear"), Sense::click());
        text(&p, cr.right_top() + vec2(0.0, 3.0), Align2::RIGHT_TOP, "Bersihkan", 11.5, if resp.hovered() { TEXT } else { DIM });
        if resp.clicked() {
            app.store.shelf.clear();
            app.store.touch();
        }
    }
    let list = Rect::from_min_max(shelf_rect.min + vec2(0.0, 24.0), shelf_rect.max);
    if n == 0 {
        dashed_rect(&p, list.shrink(2.0), FAINT.gamma_multiply(1.4));
        paint_icon(&p, Rect::from_center_size(list.center() - vec2(0.0, 14.0), egui::Vec2::splat(20.0)), Icon::Shelf, ACCENT);
        text(&p, list.center() + vec2(0.0, 12.0), Align2::CENTER_CENTER, "Seret file ke notch", 12.5, TEXT);
        text(&p, list.center() + vec2(0.0, 30.0), Align2::CENTER_CENTER, "lalu seret keluar ke app mana pun", 11.0, DIM);
    } else {
        let mut sui = ui.new_child(egui::UiBuilder::new().max_rect(list));
        egui::ScrollArea::vertical()
            .id_salt("shelf-scroll")
            .max_height(list.height())
            .auto_shrink([false, false])
            .show(&mut sui, |ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                let w = (ui.available_width() - 6.0) / 2.0;
                let items = app.store.shelf.clone();
                for pair in items.chunks(2) {
                    ui.horizontal(|ui| {
                        for it in pair {
                            file_chip(ui, &it.path, w, &mut actions);
                        }
                    });
                }
            });
    }

    // ---------------- Clipboard
    strong(&p, clip_rect.left_top() + vec2(2.0, 2.0), Align2::LEFT_TOP, "Clipboard", 12.5, TEXT);
    if !app.store.settings.clipboard_enabled {
        text(&p, clip_rect.center(), Align2::CENTER_CENTER, "Riwayat clipboard mati", 12.0, DIM);
    } else if app.store.clipboard.is_empty() {
        let c = clip_rect.center() + vec2(0.0, 8.0);
        paint_icon(&p, Rect::from_center_size(c - vec2(0.0, 18.0), egui::Vec2::splat(18.0)), Icon::Clipboard, DIM);
        text(&p, c + vec2(0.0, 6.0), Align2::CENTER_CENTER, "Teks yang kamu salin", 11.5, DIM);
        text(&p, c + vec2(0.0, 21.0), Align2::CENTER_CENTER, "akan muncul di sini", 11.5, DIM);
    } else {
        let cr = Rect::from_min_size(pos2(clip_rect.max.x - 44.0, clip_rect.min.y), vec2(44.0, 18.0));
        let resp = ui.interact(cr, Id::unique("clip-clear"), Sense::click());
        text(&p, cr.right_top() + vec2(0.0, 3.0), Align2::RIGHT_TOP, "Hapus", 11.5, if resp.hovered() { TEXT } else { DIM });
        if resp.clicked() {
            app.store.clipboard.clear();
            app.store.touch();
        }
        let list = Rect::from_min_max(clip_rect.min + vec2(0.0, 24.0), clip_rect.max);
        let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(list));
        let copied_id = Id::unique("clip-copied");
        let copied: Option<(usize, f64)> = ui.ctx().data(|d| d.get_temp(copied_id));
        let now = ui.input(|i| i.time);
        egui::ScrollArea::vertical()
            .id_salt("clip-scroll")
            .max_height(list.height())
            .auto_shrink([false, false])
            .show(&mut cui, |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                for (i, c) in app.store.clipboard.iter().enumerate() {
                    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::click_and_drag());
                    let p = ui.painter();
                    p.rect_filled(r, CornerRadius::same(7), if resp.hovered() { CARD_HI } else { CARD });
                    let one_line = c.text.split_whitespace().collect::<Vec<_>>().join(" ");
                    let just = copied.is_some_and(|(ci, t)| ci == i && now - t < 1.2);
                    let right_pad = if resp.hovered() { 26.0 } else { 8.0 };
                    if just {
                        paint_icon(p, Rect::from_center_size(pos2(r.min.x + 14.0, r.center().y), egui::Vec2::splat(12.0)), Icon::Check, BREAK);
                        text(p, pos2(r.min.x + 26.0, r.center().y), Align2::LEFT_CENTER, "Disalin", 12.0, BREAK);
                    } else {
                        text_trunc(p, pos2(r.min.x + 9.0, r.center().y), Align2::LEFT_CENTER, &one_line, 12.0, TEXT, r.width() - 9.0 - right_pad, false);
                    }
                    if resp.hovered() {
                        let xr = Rect::from_center_size(pos2(r.max.x - 13.0, r.center().y), egui::Vec2::splat(20.0));
                        let mut xui = ui.new_child(egui::UiBuilder::new().max_rect(xr));
                        if icon_button(&mut xui, Icon::Close, 20.0, DIM).clicked() {
                            actions.push(Action::RemoveClip(i));
                            continue;
                        }
                    }
                    if resp.drag_started() {
                        actions.push(Action::Drag(DragPayload::Text(c.text.clone())));
                    } else if resp.clicked() {
                        actions.push(Action::CopyText(c.text.clone()));
                        ui.ctx().data_mut(|d| d.insert_temp(copied_id, (i, now)));
                    }
                    resp.on_hover_text("Klik untuk salin · seret ke app lain");
                }
            });
        if copied.is_some_and(|(_, t)| now - t < 1.2) {
            app.request_wake(0.3);
        }
    }

    // ---------------- Terapkan aksi
    let ctx = ui.ctx().clone();
    for a in actions {
        match a {
            Action::Open(p) => app.platform.open(&p),
            Action::Reveal(p) => app.platform.reveal(&p),
            Action::CopyFile(p) => app.platform.copy_files(&ctx, &[p]),
            Action::Remove(p) => {
                app.store.shelf.retain(|s| s.path != p);
                app.store.touch();
            }
            Action::Drag(payload) => {
                app.platform.start_drag(payload);
            }
            Action::CopyText(t) => app.platform.copy_text(&ctx, &t),
            Action::RemoveClip(i) => {
                if i < app.store.clipboard.len() {
                    app.store.clipboard.remove(i);
                    app.store.touch();
                }
            }
        }
    }
}

fn file_chip(ui: &mut Ui, path: &str, w: f32, actions: &mut Vec<Action>) {
    let pth = Path::new(path);
    let name = pth.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_string());
    let ext = pth.extension().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    let exists = pth.exists();
    let (r, resp) = ui.allocate_exact_size(vec2(w, 34.0), Sense::click_and_drag());
    let p = ui.painter();
    p.rect_filled(r, CornerRadius::same(8), if resp.hovered() { CARD_HI } else { CARD });
    let badge = Rect::from_min_size(r.min + vec2(5.0, 5.0), vec2(30.0, 24.0));
    let col = if exists { ext_color(&ext) } else { FAINT };
    p.rect_filled(badge, CornerRadius::same(5), col.gamma_multiply(0.25));
    let label: String = if ext.is_empty() { "FILE".into() } else { ext.to_uppercase().chars().take(4).collect() };
    text(p, badge.center(), Align2::CENTER_CENTER, &label, 8.5, col);
    let tx = badge.max.x + 7.0;
    let name_col = if exists { TEXT } else { DIM };
    let shown = if exists { name.clone() } else { format!("{name} (hilang)") };
    text_trunc(p, pos2(tx, r.center().y), Align2::LEFT_CENTER, &shown, 12.0, name_col, r.max.x - tx - 6.0, false);

    if resp.drag_started() && exists {
        actions.push(Action::Drag(DragPayload::Files(vec![path.to_string()])));
    } else if resp.double_clicked() && exists {
        actions.push(Action::Open(path.to_string()));
    }
    let path_s = path.to_string();
    resp.on_hover_text(format!("{name}\nSeret keluar · klik ganda untuk buka · klik kanan untuk opsi"))
        .context_menu(|ui| {
            if ui.button("Buka").clicked() {
                actions.push(Action::Open(path_s.clone()));
                ui.close();
            }
            if ui.button("Tampilkan di Finder").clicked() {
                actions.push(Action::Reveal(path_s.clone()));
                ui.close();
            }
            if ui.button("Salin").clicked() {
                actions.push(Action::CopyFile(path_s.clone()));
                ui.close();
            }
            ui.separator();
            if ui.button("Hapus dari shelf").clicked() {
                actions.push(Action::Remove(path_s.clone()));
                ui.close();
            }
        });
}
