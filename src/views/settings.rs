use crate::app::{App, FocusTimer};
use crate::prayer::CITIES;
use crate::store::StatusRing;
use crate::theme::*;
use egui::{DragValue, RichText, Ui};

fn section(ui: &mut Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(title).size(11.5).color(ACCENT));
}

pub fn show(app: &mut App, ui: &mut Ui, rect: egui::Rect) {
    let before = app.store.settings.clone();
    let mut login_change = None;
    let mut quit = false;

    egui::ScrollArea::vertical()
        .id_salt("settings")
        .max_height(rect.height())
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            let s = &mut app.store.settings;
            ui.columns(2, |cols| {
                let ui = &mut cols[0];
                section(ui, "Tampilan");
                ui.checkbox(&mut s.show_clock_idle, "Jam saat notch diam");
                ui.checkbox(&mut s.minimal, "Notch minimal");
                ui.checkbox(&mut s.mascot, "Maskot saat Mac idle");
                ui.checkbox(&mut s.eq_animation, "Animasi equalizer");
                ui.checkbox(&mut s.peek_new_track, "Intip lagu baru");
                ui.checkbox(&mut s.show_in_fullscreen, "Tampil di app layar penuh");
                ui.horizontal(|ui| {
                    ui.label("Cincin status");
                    egui::ComboBox::from_id_salt("ring")
                        .width(80.0)
                        .selected_text(match s.status_ring {
                            StatusRing::Water => "Air",
                            StatusRing::Battery => "Baterai",
                            StatusRing::Off => "Mati",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut s.status_ring, StatusRing::Water, "Air");
                            ui.selectable_value(&mut s.status_ring, StatusRing::Battery, "Baterai");
                            ui.selectable_value(&mut s.status_ring, StatusRing::Off, "Mati");
                        });
                });

                section(ui, "Musik");
                ui.checkbox(&mut s.lyrics, "Lirik tersinkron (LRCLIB)");
                ui.checkbox(&mut s.browser_media, "Musik dari browser")
                    .on_hover_text("YouTube, YouTube Music, SoundCloud, Spotify Web & Apple Music Web di Chrome, Arc, Edge, Brave, Safari.\nUntuk kontrol: aktifkan \"Allow JavaScript from Apple Events\" di menu Developer browser.");

                section(ui, "Fokus (menit)");
                ui.horizontal(|ui| {
                    ui.add(DragValue::new(&mut s.focus_min).range(1..=180).prefix("Fokus "));
                    ui.add(DragValue::new(&mut s.short_break_min).range(1..=60).prefix("Rehat "));
                });
                ui.horizontal(|ui| {
                    ui.add(DragValue::new(&mut s.long_break_min).range(1..=90).prefix("Panjang "));
                    ui.add(DragValue::new(&mut s.rounds).range(1..=12).prefix("Ronde "));
                });

                section(ui, "Sistem");
                let mut login = s.launch_at_login;
                if ui.checkbox(&mut login, "Buka saat login").changed() {
                    login_change = Some(login);
                }
                if let Some(m) = &app.settings_msg {
                    ui.label(RichText::new(m).size(11.0).color(DANGER));
                }
                if ui.button("Keluar Nusa").clicked() {
                    quit = true;
                }

                let ui = &mut cols[1];
                section(ui, "Air");
                ui.checkbox(&mut s.water_reminders, "Pengingat minum");
                ui.horizontal(|ui| {
                    ui.add(DragValue::new(&mut s.water_bottle_ml).range(100..=2000).speed(10).prefix("Botol ").suffix(" ml"));
                });
                ui.horizontal(|ui| {
                    ui.add(DragValue::new(&mut s.water_goal_ml).range(500..=6000).speed(50).prefix("Target ").suffix(" ml"));
                });
                ui.horizontal(|ui| {
                    ui.add(DragValue::new(&mut s.water_interval_min).range(15..=240).speed(5).prefix("Tiap ").suffix(" mnt"));
                });
                ui.horizontal(|ui| {
                    ui.add(DragValue::new(&mut s.water_start_h).range(0..=23).prefix("Dari ").suffix(".00"));
                    ui.add(DragValue::new(&mut s.water_end_h).range(1..=24).prefix("s/d ").suffix(".00"));
                });

                section(ui, "Waktu sholat");
                ui.checkbox(&mut s.prayer_enabled, "Aktif");
                if s.prayer_enabled {
                    let custom = !CITIES.iter().any(|c| c.name == s.prayer_city);
                    egui::ComboBox::from_id_salt("city")
                        .width(150.0)
                        .selected_text(&s.prayer_city)
                        .height(160.0)
                        .show_ui(ui, |ui| {
                            for c in CITIES {
                                if ui.selectable_label(s.prayer_city == c.name, c.name).clicked() {
                                    s.prayer_city = c.name.to_string();
                                    s.prayer_lat = c.lat;
                                    s.prayer_lon = c.lon;
                                }
                            }
                            if ui.selectable_label(custom, "Lainnya (koordinat)…").clicked() {
                                s.prayer_city = "Lokasi saya".into();
                            }
                        });
                    if custom {
                        ui.horizontal(|ui| {
                            ui.add(DragValue::new(&mut s.prayer_lat).range(-66.0..=66.0).speed(0.01).max_decimals(4).prefix("Lat "));
                            ui.add(DragValue::new(&mut s.prayer_lon).range(-180.0..=180.0).speed(0.01).max_decimals(4).prefix("Lon "));
                        });
                    }
                    ui.add(DragValue::new(&mut s.prayer_remind_min).range(0..=60).prefix("Ingatkan ").suffix(" mnt sebelum"));
                    ui.checkbox(&mut s.prayer_pause_music, "Jeda musik saat adzan");
                }

                section(ui, "Clipboard");
                ui.checkbox(&mut s.clipboard_enabled, "Riwayat clipboard");
                ui.add(DragValue::new(&mut s.clipboard_max).range(1..=100).prefix("Maks ").suffix(" item"));
                ui.checkbox(&mut s.clipboard_persist, "Simpan ke disk");
            });
            ui.add_space(6.0);
            ui.label(
                RichText::new("Semua data tersimpan lokal di Mac ini. Lirik: hanya judul, artis & durasi yang dikirim ke LRCLIB.")
                    .size(10.5)
                    .color(DIM),
            );
        });

    if let Some(on) = login_change {
        match app.platform.set_launch_at_login(on) {
            Ok(()) => {
                app.store.settings.launch_at_login = on;
                app.settings_msg = None;
            }
            Err(e) => app.settings_msg = Some(e),
        }
    }
    let s = app.store.settings.clone();
    if s.water_end_h <= s.water_start_h {
        app.store.settings.water_end_h = (s.water_start_h + 1).min(24);
    }
    let changed = format!("{:?}", before) != format!("{:?}", app.store.settings);
    if changed {
        let timing_changed = before.focus_min != s.focus_min
            || before.short_break_min != s.short_break_min
            || before.long_break_min != s.long_break_min
            || before.rounds != s.rounds;
        if timing_changed && !app.focus.running {
            app.focus = FocusTimer::new(&s);
        }
        app.store.clipboard.truncate(s.clipboard_max.max(1));
        app.store.touch();
    }
    if quit {
        app.store.save();
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
    }
}
