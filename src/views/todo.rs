use crate::app::{parse_due, App};
use crate::store::Todo;
use crate::theme::*;
use chrono::{Local, TimeZone};
use egui::{pos2, vec2, Align2, CornerRadius, Id, Rect, Sense, Ui};

enum Action {
    Toggle(u64),
    Delete(u64),
    Move(u64, i32),
    StartEdit(u64),
    SaveEdit,
    CancelEdit,
}

pub fn show(app: &mut App, ui: &mut Ui, rect: Rect) {
    // Baris input
    let input_rect = Rect::from_min_size(rect.min, vec2(rect.width(), 28.0));
    let mut iui = ui.new_child(egui::UiBuilder::new().max_rect(input_rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
    let te = egui::TextEdit::singleline(&mut app.todo_input)
        .hint_text("Tambah tugas…")
        .desired_width(rect.width() - 110.0)
        .margin(vec2(8.0, 4.0));
    let r1 = iui.add(te);
    let tt = egui::TextEdit::singleline(&mut app.todo_time)
        .hint_text("jam")
        .desired_width(44.0)
        .margin(vec2(6.0, 4.0));
    let r2 = iui.add(tt).on_hover_text("Tenggat opsional, mis. 14:30");
    let enter = (r1.lost_focus() || r2.lost_focus()) && iui.input(|i| i.key_pressed(egui::Key::Enter));
    let add = icon_button(&mut iui, Icon::Plus, 28.0, TODO).on_hover_text("Tambah").clicked();
    if enter || add {
        app.add_todo();
        r1.request_focus();
    }

    // Baris info
    let p = ui.painter().clone();
    let active = app.store.todos.iter().filter(|t| !t.done).count();
    let done = app.store.todos.len() - active;
    let info_y = rect.min.y + 42.0;
    text(&p, pos2(rect.min.x + 2.0, info_y), Align2::LEFT_CENTER, &format!("{active} aktif"), 11.5, DIM);
    let sort_label = if app.store.settings.todo_sort_by_time { "Urut: waktu" } else { "Urut: manual" };
    let sr = Rect::from_min_size(pos2(rect.min.x + 70.0, info_y - 9.0), vec2(90.0, 18.0));
    let sresp = ui.interact(sr, Id::unique("sort"), Sense::click());
    text(&p, sr.left_center(), Align2::LEFT_CENTER, sort_label, 11.5, if sresp.hovered() { TEXT } else { ACCENT });
    if sresp.clicked() {
        app.store.settings.todo_sort_by_time = !app.store.settings.todo_sort_by_time;
        app.store.touch();
    }
    if done > 0 {
        let cr = Rect::from_min_size(pos2(rect.max.x - 100.0, info_y - 9.0), vec2(100.0, 18.0));
        let cresp = ui.interact(cr, Id::unique("clear-done"), Sense::click());
        text(&p, cr.right_center(), Align2::RIGHT_CENTER, &format!("Hapus {done} selesai"), 11.5, if cresp.hovered() { TEXT } else { DIM });
        if cresp.clicked() {
            app.store.todos.retain(|t| !t.done);
            app.store.touch();
        }
    }

    // Daftar
    let list_rect = Rect::from_min_max(pos2(rect.min.x, info_y + 12.0), rect.max);
    let mut order: Vec<usize> = (0..app.store.todos.len()).collect();
    if app.store.settings.todo_sort_by_time {
        order.sort_by_key(|&i| {
            let t = &app.store.todos[i];
            (t.done, t.due.unwrap_or(i64::MAX))
        });
    } else {
        order.sort_by_key(|&i| app.store.todos[i].done);
    }
    if order.is_empty() {
        text(&p, list_rect.center(), Align2::CENTER_CENTER, "Belum ada tugas. Ketik di atas lalu tekan Enter.", 12.0, DIM);
        return;
    }
    let now = Local::now().timestamp();
    let manual = !app.store.settings.todo_sort_by_time;
    let editing_id = app.todo_edit.as_ref().map(|e| e.0);
    let focus_id = Id::unique("todo-edit-focus");
    let mut actions = Vec::new();
    let mut lui = ui.new_child(egui::UiBuilder::new().max_rect(list_rect));
    egui::ScrollArea::vertical()
        .max_height(list_rect.height())
        .auto_shrink([false, false])
        .show(&mut lui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            for &i in &order {
                let t = &app.store.todos[i];

                // ---- Mode edit
                if editing_id == Some(t.id) {
                    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::hover());
                    ui.painter().rect_filled(row, CornerRadius::same(6), CARD);
                    let mut eui = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(row.shrink2(vec2(4.0, 2.0)))
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );
                    eui.spacing_mut().item_spacing.x = 4.0;
                    let Some(edit) = app.todo_edit.as_mut() else { continue };
                    let text_w = eui.available_width() - 44.0 - 4.0 - 2.0 * 26.0;
                    let rt = eui.add(egui::TextEdit::singleline(&mut edit.1).desired_width(text_w).margin(vec2(6.0, 3.0)));
                    let rtime = eui
                        .add(egui::TextEdit::singleline(&mut edit.2).hint_text("jam").desired_width(44.0).margin(vec2(5.0, 3.0)))
                        .on_hover_text("Kosongkan untuk menghapus tenggat");
                    // fokus otomatis ke kolom teks saat edit dimulai
                    if ui.ctx().data_mut(|d| d.remove_temp::<bool>(focus_id)).unwrap_or(false) {
                        rt.request_focus();
                    }
                    let save = icon_button(&mut eui, Icon::Check, 26.0, BREAK).on_hover_text("Simpan (Enter)").clicked();
                    let cancel = icon_button(&mut eui, Icon::Close, 26.0, DIM).on_hover_text("Batal (Esc)").clicked();
                    let (enter, esc) = eui.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
                    let typing_here = rt.has_focus() || rt.lost_focus() || rtime.has_focus() || rtime.lost_focus();
                    if save || (enter && typing_here) {
                        actions.push(Action::SaveEdit);
                    } else if cancel || esc {
                        actions.push(Action::CancelEdit);
                    }
                    continue;
                }

                // ---- Baris biasa
                let (row, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::hover());
                let p = ui.painter();
                if resp.hovered() {
                    p.rect_filled(row, CornerRadius::same(6), CARD);
                }
                // centang
                let cb = Rect::from_center_size(pos2(row.min.x + 12.0, row.center().y), egui::Vec2::splat(16.0));
                let cresp = ui.interact(cb.expand(3.0), Id::unique(("todo-cb", t.id)), Sense::click());
                if t.done {
                    p.circle_filled(cb.center(), 8.0, TODO);
                    paint_icon(p, cb.shrink(3.0), Icon::Check, BG);
                } else {
                    p.circle_stroke(cb.center(), 7.5, egui::Stroke::new(1.5, if cresp.hovered() { TODO } else { DIM }));
                }
                if cresp.clicked() {
                    actions.push(Action::Toggle(t.id));
                }
                let n_btn = if manual { 4.0 } else { 2.0 };
                let btn_w = n_btn * 22.0;
                let text_area = Rect::from_min_max(pos2(cb.max.x + 6.0, row.min.y), pos2(row.max.x - 4.0, row.max.y));
                let tresp = ui.interact(text_area, Id::unique(("todo-text", t.id)), Sense::click());
                let hovered = resp.hovered() || cresp.hovered() || tresp.hovered();
                let mut right = row.max.x - 6.0;
                if hovered {
                    right -= btn_w + 2.0;
                }
                if let Some(d) = t.due {
                    let overdue = d <= now && !t.done;
                    let label = due_label(d);
                    let col = if overdue { DANGER } else { DIM };
                    let r = text(p, pos2(right, row.center().y), Align2::RIGHT_CENTER, &label, 11.0, col);
                    right = r.min.x - 8.0;
                }
                let tx = cb.max.x + 10.0;
                let tr = text_trunc(p, pos2(tx, row.center().y), Align2::LEFT_CENTER, &t.text, 13.0, if t.done { FAINT.gamma_multiply(1.6) } else { TEXT }, right - tx, false);
                if t.done {
                    p.line_segment([pos2(tr.min.x, tr.center().y), pos2(tr.max.x, tr.center().y)], egui::Stroke::new(1.0, FAINT.gamma_multiply(1.6)));
                }
                if tresp.on_hover_text("Klik ganda untuk edit").double_clicked() {
                    actions.push(Action::StartEdit(t.id));
                }
                if hovered {
                    let br = Rect::from_min_max(pos2(row.max.x - btn_w - 2.0, row.min.y + 1.0), row.max);
                    let mut bui = ui.new_child(egui::UiBuilder::new().max_rect(br).layout(egui::Layout::left_to_right(egui::Align::Center)));
                    bui.spacing_mut().item_spacing.x = 0.0;
                    if manual {
                        if icon_button(&mut bui, Icon::Up, 22.0, DIM).clicked() {
                            actions.push(Action::Move(t.id, -1));
                        }
                        if icon_button(&mut bui, Icon::Down, 22.0, DIM).clicked() {
                            actions.push(Action::Move(t.id, 1));
                        }
                    }
                    if icon_button(&mut bui, Icon::Pencil, 22.0, DIM).on_hover_text("Edit").clicked() {
                        actions.push(Action::StartEdit(t.id));
                    }
                    if icon_button(&mut bui, Icon::Close, 22.0, DIM).on_hover_text("Hapus").clicked() {
                        actions.push(Action::Delete(t.id));
                    }
                }
            }
        });

    for a in actions {
        match a {
            Action::Toggle(id) => {
                if let Some(t) = app.store.todos.iter_mut().find(|t| t.id == id) {
                    t.done = !t.done;
                }
            }
            Action::Delete(id) => {
                app.store.todos.retain(|t| t.id != id);
                if app.todo_edit.as_ref().is_some_and(|e| e.0 == id) {
                    app.todo_edit = None;
                }
            }
            Action::Move(id, dir) => {
                let todos = &mut app.store.todos;
                if let Some(i) = todos.iter().position(|t| t.id == id) {
                    let j = i as i32 + dir;
                    if j >= 0 && (j as usize) < todos.len() {
                        todos.swap(i, j as usize);
                    }
                }
            }
            Action::StartEdit(id) => {
                if let Some(t) = app.store.todos.iter().find(|t| t.id == id) {
                    let time = t.due.map(hhmm).unwrap_or_default();
                    app.todo_edit = Some((id, t.text.clone(), time));
                    ui.ctx().data_mut(|d| d.insert_temp(focus_id, true));
                }
                continue;
            }
            Action::SaveEdit => {
                if let Some((id, text, time)) = app.todo_edit.take() {
                    if let Some(t) = app.store.todos.iter_mut().find(|t| t.id == id) {
                        apply_edit(t, &text, &time);
                    }
                }
            }
            Action::CancelEdit => {
                app.todo_edit = None;
                continue;
            }
        }
        app.store.touch();
    }
}

/// Simpan hasil edit. Teks kosong = tidak berubah. Jam kosong = hapus tenggat.
/// Jam tidak valid = tenggat lama dipertahankan.
pub fn apply_edit(t: &mut Todo, text: &str, time: &str) {
    let text = text.trim();
    if !text.is_empty() {
        t.text = text.to_string();
    }
    let new_due = if time.trim().is_empty() {
        None
    } else if t.due.map(hhmm).as_deref() == Some(time.trim()) {
        t.due // jam tidak diubah → jangan geser ke besok
    } else {
        parse_due(time).or(t.due)
    };
    if new_due != t.due {
        t.due = new_due;
        t.notified = false;
    }
}

fn hhmm(ts: i64) -> String {
    Local
        .timestamp_opt(ts, 0)
        .single()
        .map(|d| d.format("%H:%M").to_string())
        .unwrap_or_default()
}

fn due_label(ts: i64) -> String {
    let Some(dt) = Local.timestamp_opt(ts, 0).single() else { return String::new() };
    let today = Local::now().date_naive();
    let d = dt.date_naive();
    let hm = dt.format("%H:%M").to_string();
    if d == today {
        hm
    } else if d == today.succ_opt().unwrap_or(today) {
        format!("besok {hm}")
    } else {
        dt.format("%d/%m %H:%M").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_todo() {
        let due = parse_due("14:30");
        let mut t = Todo { id: 1, text: "Lama".into(), due, notified: true, ..Default::default() };
        // ubah teks saja, jam sama → tenggat & status notifikasi tetap
        apply_edit(&mut t, "  Kirim proposal ", "14:30");
        assert_eq!((t.text.as_str(), t.due, t.notified), ("Kirim proposal", due, true));
        // teks kosong tidak menghapus teks
        apply_edit(&mut t, "   ", "14:30");
        assert_eq!(t.text, "Kirim proposal");
        // jam tidak valid → tenggat lama dipertahankan
        apply_edit(&mut t, "Kirim proposal", "99:99");
        assert_eq!(t.due, due);
        // jam baru → tenggat berubah & notifikasi di-reset
        apply_edit(&mut t, "Kirim proposal", "09:15");
        assert_eq!(t.due, parse_due("09:15"));
        assert!(!t.notified);
        // jam dikosongkan → tenggat dihapus
        apply_edit(&mut t, "Kirim proposal", "");
        assert_eq!(t.due, None);
    }
}
