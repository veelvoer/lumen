//! The settings screen. Every change is saved at once and applied live.

use crate::android;
use crate::app::{Lumen, Screen};
use crate::config::{Clock, DockStyle, DrawerStyle, Mode, Preset, Search};
use crate::theme::Palette;
use crate::update;
use eframe::egui::{self, vec2, Align, Color32, CornerRadius, Frame, Layout, RichText, Stroke, Ui};

fn card(ui: &mut Ui, pal: &Palette, title: &str, add: impl FnOnce(&mut Ui)) {
    ui.add_space(6.0);
    ui.label(RichText::new(title.to_uppercase()).size(12.0).color(pal.accent).strong());
    Frame::NONE.fill(pal.card).corner_radius(18).inner_margin(14).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui);
    });
}

fn row(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.with_layout(Layout::right_to_left(Align::Center), add);
    });
}

fn choice<T: PartialEq + Copy>(ui: &mut Ui, pal: &Palette, cur: &mut T, options: &[(T, &str)]) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        for (v, name) in options {
            let on = *cur == *v;
            let b = egui::Button::new(RichText::new(*name).color(if on { pal.on_accent } else { pal.text })).fill(if on { pal.accent } else { pal.card }).corner_radius(12);
            if ui.add(b).clicked() && !on {
                *cur = *v;
                changed = true;
            }
        }
    });
    changed
}

pub fn show(l: &mut Lumen, root: &mut Ui) {
    let ctx = &root.ctx().clone();
    let pal = l.pal;
    let k = ctx.pixels_per_point();
    let ins = android::insets();
    let (top, bottom) = (ins.top / k, ins.bottom / k);
    let mut changed = false;
    let mut preset: Option<Preset> = None;

    egui::CentralPanel::default().frame(Frame::NONE.fill(pal.sheet)).show(root, |ui| {
        ui.add_space(top + 6.0);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            if ui.add(egui::Button::new(RichText::new("‹  Back").size(16.0)).fill(Color32::TRANSPARENT)).clicked() {
                l.screen = Screen::Home;
            }
        });
        egui::ScrollArea::vertical().auto_shrink(false).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
            ui.add_space(4.0);
            Frame::NONE.inner_margin(egui::Margin::symmetric(16, 0)).show(ui, |ui| {
                ui.label(RichText::new("Lumen").size(30.0).strong());

                card(ui, &pal, "Look", |ui| {
                    ui.label(RichText::new("Pick a preset, then change anything you like.").color(pal.dim).size(13.0));
                    ui.horizontal(|ui| {
                        for (p, name) in [(Preset::Nothing, "Nothing"), (Preset::Pixel, "Pixel"), (Preset::Ios, "iOS")] {
                            let on = l.cfg.preset == p;
                            let b = egui::Button::new(RichText::new(name).size(16.0).strong().color(if on { pal.on_accent } else { pal.text })).fill(if on { pal.accent } else { pal.card }).corner_radius(14);
                            if ui.add_sized([(ui.available_width() - 16.0) / 3.0, 54.0], b).clicked() {
                                preset = Some(p);
                            }
                        }
                    });
                    if l.cfg.preset == Preset::Custom {
                        ui.label(RichText::new("Custom").color(pal.dim).size(13.0));
                    }
                    ui.add_space(4.0);
                    changed |= choice(ui, &pal, &mut l.cfg.mode, &[(Mode::Dark, "Dark"), (Mode::Light, "Light")]);
                    row(ui, "Accent color", |ui| {
                        changed |= ui.color_edit_button_srgb(&mut l.cfg.accent).changed();
                    });
                    row(ui, "Wallpaper dimming", |ui| {
                        changed |= ui.add(egui::Slider::new(&mut l.cfg.wallpaper_dim, 0.0..=0.8).show_value(false)).changed();
                    });
                    row(ui, "Text size", |ui| {
                        changed |= ui.add(egui::Slider::new(&mut l.cfg.font_scale, 0.8..=1.3).show_value(false)).changed();
                    });
                });

                card(ui, &pal, "Icons", |ui| {
                    row(ui, "Size", |ui| {
                        changed |= ui.add(egui::Slider::new(&mut l.cfg.icon_size, 36.0..=76.0).show_value(false)).changed();
                    });
                    row(ui, "Roundness", |ui| {
                        changed |= ui.add(egui::Slider::new(&mut l.cfg.icon_radius, 0.0..=1.0).show_value(false)).changed();
                    });
                    changed |= ui.checkbox(&mut l.cfg.labels, "Show names").changed();
                    changed |= ui.checkbox(&mut l.cfg.mono_icons, "Single-color icons (when the app supports it)").changed();
                });

                card(ui, &pal, "Home screen", |ui| {
                    row(ui, "Columns", |ui| {
                        changed |= ui.add(egui::Slider::new(&mut l.cfg.cols, 3..=6).show_value(false)).changed();
                    });
                    row(ui, "Rows", |ui| {
                        changed |= ui.add(egui::Slider::new(&mut l.cfg.rows, 3..=8).show_value(false)).changed();
                    });
                    ui.label("Clock");
                    changed |= choice(ui, &pal, &mut l.cfg.clock, &[(Clock::Off, "Off"), (Clock::Glance, "Date"), (Clock::Digital, "Digital"), (Clock::DotMatrix, "Dot matrix")]);
                    ui.label("Search bar");
                    changed |= choice(ui, &pal, &mut l.cfg.search, &[(Search::Off, "Off"), (Search::Top, "Top"), (Search::Bottom, "Bottom")]);
                    changed |= ui.checkbox(&mut l.cfg.page_dots, "Page dots").changed();
                });

                card(ui, &pal, "Dock", |ui| {
                    changed |= choice(ui, &pal, &mut l.cfg.dock, &[(DockStyle::Glass, "Glass"), (DockStyle::Solid, "Solid"), (DockStyle::Bare, "None")]);
                    row(ui, "Corner radius", |ui| {
                        changed |= ui.add(egui::Slider::new(&mut l.cfg.dock_radius, 0.0..=48.0).show_value(false)).changed();
                    });
                });

                card(ui, &pal, "App drawer", |ui| {
                    changed |= choice(ui, &pal, &mut l.cfg.drawer, &[(DrawerStyle::Grid, "Grid"), (DrawerStyle::List, "List")]);
                    if !l.cfg.hidden.is_empty() && ui.button(format!("Show {} hidden apps again", l.cfg.hidden.len())).clicked() {
                        l.cfg.hidden.clear();
                        l.dirty = true;
                    }
                });

                card(ui, &pal, "Phone", |ui| {
                    let home = l.default_home;
                    ui.label(if home { "Lumen is your home app." } else { "Lumen is not your home app yet." });
                    if !home && ui.button("Make Lumen the home app").clicked() {
                        android::pick_default_home();
                    }
                    if ui.button("Change wallpaper").clicked() {
                        android::open_wallpaper_picker();
                    }
                    if ui.button("Phone settings").clicked() {
                        android::open_system_settings();
                    }
                });

                card(ui, &pal, "Updates", |ui| {
                    let st = l.upd.state();
                    ui.label(format!("Lumen {}", l.version));
                    let status = match (&st.available, st.busy, st.checked) {
                        (_, true, _) if st.progress.is_some() => format!("Downloading… {}%", st.progress.unwrap_or(0)),
                        (_, true, _) => "Looking…".to_string(),
                        (Some(r), _, _) => format!("Version {} is ready", r.version),
                        (None, _, true) => "This is the newest version".to_string(),
                        _ => String::new(),
                    };
                    if !status.is_empty() {
                        ui.label(RichText::new(status).color(pal.dim));
                    }
                    if let Some(r) = &st.available {
                        for line in update::plain(&r.notes).iter().take(14) {
                            ui.label(RichText::new(line).size(13.0).color(pal.dim));
                        }
                    }
                    if !st.message.is_empty() {
                        ui.label(RichText::new(&st.message).color(pal.accent));
                    }
                    ui.horizontal(|ui| {
                        let c2 = ctx.clone();
                        if st.available.is_some() {
                            if ui.add_enabled(!st.busy, egui::Button::new(RichText::new("Update").color(pal.on_accent)).fill(pal.accent)).clicked() {
                                l.upd.install(move || c2.request_repaint());
                            }
                        } else if ui.add_enabled(!st.busy, egui::Button::new("Check for updates")).clicked() {
                            l.upd.check(move || c2.request_repaint());
                        }
                    });
                    if changed_toggle(ui, &mut l.cfg.auto_update, "Look for updates on start") {
                        l.dirty = true;
                    }
                    if st.busy {
                        ctx.request_repaint_after(std::time::Duration::from_millis(200));
                    }
                });

                card(ui, &pal, "About", |ui| {
                    ui.label("Lumen is written in Rust. Open source.");
                    if ui.add(egui::Button::new(RichText::new("github.com/veelvoer/lumen").color(pal.accent)).fill(Color32::TRANSPARENT).stroke(Stroke::NONE)).clicked() {
                        android::open_url("https://github.com/veelvoer/lumen");
                    }
                });
                ui.add_space(bottom + 40.0);
            });
        });
    });
    let _ = (vec2(0.0, 0.0), CornerRadius::ZERO);

    if let Some(p) = preset {
        l.cfg.apply_preset(p);
        changed = true;
    } else if changed && l.cfg.preset != Preset::Custom {
        l.cfg.preset = Preset::Custom;
    }
    if changed {
        l.dirty = true;
        l.restyle(ctx);
    }
}

fn changed_toggle(ui: &mut Ui, v: &mut bool, label: &str) -> bool {
    ui.checkbox(v, label).changed()
}
