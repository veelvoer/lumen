//! Quick-action menu, rename dialog, widget picker and toast.

use crate::app::*;
use crate::cards;
use crate::config::{Item, Preset, WidgetKind};
use crate::fonts;
use crate::phosphor as ph;
use crate::platform;
use crate::widgets::{self, cr};
use egui::{pos2, vec2, Align2, Area, Color32, Context, Frame, Id, Order, Rect, Sense, Stroke, UiBuilder};

#[derive(Clone, Debug)]
enum Act {
    AppInfo(String),
    RemoveKey(String),
    AddHome(String),
    AddDock(String),
    Hide(String),
    Rename(String),
    Uninstall(String),
    EditHome,
    Settings,
    Wallpaper,
    Widgets,
    PhoneSettings,
    RenameFolder(u32),
    DissolveFolder(u32),
}

impl Lumen {
    fn menu_items(&self, target: &Target) -> Vec<(&'static str, &'static str, Act, bool)> {
        let ios = self.cfg.preset == Preset::Ios;
        match target {
            Target::Home => {
                let mut v = vec![];
                if ios {
                    v.push((ph::PENCIL_SIMPLE, "Edit Home Screen", Act::EditHome, false));
                }
                v.push((ph::SQUARES_FOUR, "Widgets", Act::Widgets, false));
                v.push((ph::IMAGE, "Wallpaper", Act::Wallpaper, false));
                v.push((ph::GEAR, "Lumen settings", Act::Settings, false));
                v.push((ph::DEVICE_MOBILE, "Phone settings", Act::PhoneSettings, false));
                v
            }
            Target::App { pkg, on_home } => {
                let mut v = vec![];
                if *on_home {
                    if ios {
                        v.push((ph::PENCIL_SIMPLE, "Edit Home Screen", Act::EditHome, false));
                    }
                    v.push((ph::X, if ios { "Remove from Home Screen" } else { "Remove" }, Act::RemoveKey(format!("app:{pkg}")), false));
                } else {
                    v.push((ph::PLUS, "Add to home", Act::AddHome(pkg.clone()), false));
                    if self.cfg.dock_items.len() < self.cfg.dock_slots {
                        v.push((ph::ARROW_LINE_DOWN, "Add to dock", Act::AddDock(pkg.clone()), false));
                    }
                }
                if pkg == platform::PACKAGE {
                    v.push((ph::GEAR, "Lumen settings", Act::Settings, false));
                } else {
                    v.push((ph::INFO, "App info", Act::AppInfo(pkg.clone()), false));
                }
                v.push((ph::PENCIL_SIMPLE, "Rename", Act::Rename(pkg.clone()), false));
                let own = pkg == platform::PACKAGE;
                if !*on_home && !own {
                    v.push((ph::EYE_SLASH, "Hide", Act::Hide(pkg.clone()), false));
                }
                if !own {
                    v.push((ph::TRASH, "Uninstall", Act::Uninstall(pkg.clone()), true));
                }
                v
            }
            Target::Folder(id) => {
                let mut v = vec![];
                if ios {
                    v.push((ph::PENCIL_SIMPLE, "Edit Home Screen", Act::EditHome, false));
                }
                v.push((ph::PENCIL_SIMPLE, "Rename", Act::RenameFolder(*id), false));
                v.push((ph::X, "Remove folder", Act::DissolveFolder(*id), true));
                v
            }
            Target::Widget(id) => vec![(ph::X, "Remove widget", Act::RemoveKey(format!("widget:{id}")), true)],
        }
    }

    pub fn menu_ui(&mut self, ctx: &Context, screen: Rect) {
        let Some(menu) = self.menu.clone() else { return };
        let pal = self.pal;
        let cfg = self.cfg.clone();
        let ios = cfg.preset == Preset::Ios;
        let nothing = cfg.preset == Preset::Nothing;
        let items = self.menu_items(&menu.target);
        let mut close = false;
        let mut act: Option<Act> = None;

        // A tap anywhere else closes it. While the finger is still down after a long press the drag takes over.
        let bg = Area::new(Id::new("menu-bg")).order(Order::Tooltip).fixed_pos(screen.min).show(ctx, |ui| ui.allocate_rect(screen, Sense::click()));
        let lifted = matches!(self.touch, Touch::Lifted { .. } | Touch::Drag(_));
        if bg.inner.clicked() && !lifted && ctx.input(|i| i.time) > self.suppress_click {
            close = true;
        }

        let width = if ios { 250.0 } else { 232.0 };
        let row_h = if ios { 46.0 } else { 50.0 };
        let height = items.len() as f32 * row_h + 12.0;
        let a = menu.anchor;
        let below = a.bottom() + 14.0 + height < screen.bottom() - 40.0;
        let y = if below { a.bottom() + 14.0 } else { (a.top() - 14.0 - height).max(screen.top() + 40.0) };
        let x = (a.center().x - width / 2.0).clamp(12.0, screen.right() - width - 12.0);
        let radius = if ios { 16.0 } else if nothing { 14.0 } else { 26.0 };
        // grows out of the icon it belongs to
        let mt = self.menu_t.x.clamp(0.0, 1.15);
        let sc = 0.7 + 0.3 * mt;
        let pivot = pos2(a.center().x.clamp(x, x + width), if below { y } else { y + height });
        ctx.set_transform_layer(egui::LayerId::new(Order::Tooltip, Id::new("menu")), egui::emath::TSTransform::new(pivot.to_vec2() * (1.0 - sc), sc));
        Area::new(Id::new("menu")).order(Order::Tooltip).fixed_pos(pos2(x, y)).show(ctx, |ui| {
            ui.set_opacity(mt.min(1.0));
            Frame::NONE
                .fill(pal.menu_bg)
                .corner_radius(cr(radius))
                .stroke(Stroke::new(1.0, pal.glass_line))
                .inner_margin(6)
                .shadow(egui::epaint::Shadow { offset: [0, 8], blur: 28, spread: 0, color: Color32::from_black_alpha(100) })
                .show(ui, |ui| {
                    ui.set_width(width - 12.0);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for (i, (glyph, label, a, danger)) in items.iter().enumerate() {
                        let (rect, resp) = ui.allocate_exact_size(vec2(width - 12.0, row_h), Sense::click());
                        let col = if *danger { Color32::from_rgb(235, 87, 87) } else { pal.text };
                        let p = ui.painter();
                        if resp.is_pointer_button_down_on() {
                            p.rect_filled(rect, cr(radius - 8.0), pal.card);
                        }
                        let font = fonts::body(&cfg, 15.5, false);
                        if ios {
                            p.text(rect.left_center() + vec2(14.0, 0.0), Align2::LEFT_CENTER, label, font, col);
                            widgets::glyph_at(p, rect.right_center() - vec2(14.0, 0.0), Align2::RIGHT_CENTER, glyph, 19.0, col);
                            if i + 1 < items.len() {
                                p.line_segment([rect.left_bottom() + vec2(0.0, 0.0), rect.right_bottom()], Stroke::new(1.0, pal.glass_line));
                            }
                        } else {
                            widgets::glyph_at(p, rect.left_center() + vec2(18.0, 0.0), Align2::LEFT_CENTER, glyph, 20.0, if *danger { col } else { pal.accent });
                            p.text(rect.left_center() + vec2(52.0, 0.0), Align2::LEFT_CENTER, label, font, col);
                        }
                        if resp.clicked() && !lifted {
                            act = Some(a.clone());
                        }
                    }
                });
        });

        if let Some(a) = act {
            close = true;
            self.run_menu_act(ctx, a);
        }
        if close {
            self.menu = None;
            if matches!(self.touch, Touch::Lifted { .. }) {
                self.touch = Touch::Idle;
            }
        }
    }

    fn run_menu_act(&mut self, ctx: &Context, a: Act) {
        self.dirty = true;
        match a {
            Act::AppInfo(p) => crate::bridge::bg(move || platform::app_info(&p)),
            Act::Uninstall(p) => crate::bridge::bg(move || platform::uninstall(&p)),
            Act::RemoveKey(k) => self.remove_item(&k),
            Act::AddHome(p) => {
                let pi = (self.page.round() as usize).min(self.cfg.pages.len().saturating_sub(1));
                let item = Item::App(p.clone());
                self.cfg.remove_app(&p);
                if let Some((c, r)) = self.cfg.free_spot(pi, 1, 1) {
                    self.cfg.place(pi, c, r, item);
                } else {
                    self.cfg.insert_auto(item);
                }
                self.close_drawer(ctx);
                self.toast(ctx, "Added to home");
            }
            Act::AddDock(p) => {
                self.cfg.remove_app(&p);
                self.cfg.dock_items.push(Item::App(p));
                self.close_drawer(ctx);
            }
            Act::Hide(p) => {
                self.cfg.remove_app(&p);
                self.cfg.hidden.push(p);
                self.toast(ctx, "Hidden. Bring it back in Settings > App drawer");
            }
            Act::Rename(p) => {
                let text = self.label_of(&p);
                self.dialog = Some(Dialog::RenameApp { pkg: p, text });
            }
            Act::RenameFolder(id) => {
                let name = self.cfg.pages.iter().flat_map(|p| p.items.iter().map(|x| &x.item)).chain(self.cfg.dock_items.iter()).find_map(|i| match i {
                    Item::Folder { id: fid, name, .. } if *fid == id => Some(name.clone()),
                    _ => None,
                });
                self.dialog = Some(Dialog::RenameFolder { id, text: name.unwrap_or_default() });
            }
            Act::DissolveFolder(id) => {
                let key = format!("folder:{id}");
                if let Some(Item::Folder { apps, .. }) = self.cfg.remove_key(&key) {
                    for a in apps {
                        self.cfg.insert_auto(Item::App(a));
                    }
                }
            }
            Act::EditHome => self.edit = true,
            Act::Settings => self.open_settings(),
            Act::Wallpaper => platform::open_wallpaper_picker(),
            Act::Widgets => self.widget_picker = true,
            Act::PhoneSettings => platform::open_system_settings(),
        }
    }

    pub fn dialog_ui(&mut self, ctx: &Context, screen: Rect) {
        let Some(mut dlg) = self.dialog.clone() else { return };
        let pal = self.pal;
        let cfg = self.cfg.clone();
        let (title, mut text) = match &dlg {
            Dialog::RenameApp { text, .. } => ("Rename app", text.clone()),
            Dialog::RenameFolder { text, .. } => ("Rename folder", text.clone()),
        };
        let mut save = false;
        let mut cancel = false;
        Area::new(Id::new("dialog-bg")).order(Order::Tooltip).fixed_pos(screen.min).show(ctx, |ui| {
            ui.painter().rect_filled(screen, 0, Color32::from_black_alpha(140));
            ui.allocate_rect(screen, Sense::click_and_drag());
        });
        let w = (screen.width() - 56.0).min(360.0);
        Area::new(Id::new("dialog")).order(Order::Tooltip).fixed_pos(pos2(screen.center().x - w / 2.0, screen.top() + screen.height() * 0.22)).show(ctx, |ui| {
            Frame::NONE.fill(pal.menu_bg).corner_radius(cr(28.0)).inner_margin(22).show(ui, |ui| {
                ui.set_width(w - 44.0);
                ui.label(egui::RichText::new(title).font(fonts::body(&cfg, 20.0, true)).color(pal.text));
                ui.add_space(10.0);
                let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(f32::INFINITY).font(fonts::body(&cfg, 17.0, false)).margin(vec2(14.0, 12.0)));
                if !r.has_focus() && !ui.memory(|m| m.has_focus(r.id)) {
                    r.request_focus();
                }
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new(egui::RichText::new("Save").color(pal.on_accent)).fill(pal.accent).corner_radius(cr(20.0))).clicked() {
                            save = true;
                        }
                        if ui.add(egui::Button::new(egui::RichText::new("Cancel").color(pal.accent)).fill(Color32::TRANSPARENT)).clicked() {
                            cancel = true;
                        }
                    });
                });
            });
        });
        match &mut dlg {
            Dialog::RenameApp { text: t, .. } | Dialog::RenameFolder { text: t, .. } => *t = text.clone(),
        }
        if save {
            match dlg {
                Dialog::RenameApp { pkg, text } => {
                    if text.trim().is_empty() {
                        self.cfg.names.remove(&pkg);
                    } else {
                        self.cfg.names.insert(pkg, text.trim().to_string());
                    }
                }
                Dialog::RenameFolder { id, text } => {
                    let set = |it: &mut Item| {
                        if let Item::Folder { id: fid, name, .. } = it {
                            if *fid == id {
                                *name = text.trim().to_string();
                            }
                        }
                    };
                    for pg in self.cfg.pages.iter_mut() {
                        for pl in pg.items.iter_mut() {
                            set(&mut pl.item);
                        }
                    }
                    for it in self.cfg.dock_items.iter_mut() {
                        set(it);
                    }
                }
            }
            self.dirty = true;
            self.dialog = None;
            ctx.memory_mut(|m| m.stop_text_input());
        } else if cancel {
            self.dialog = None;
            ctx.memory_mut(|m| m.stop_text_input());
        } else {
            self.dialog = Some(dlg);
        }
    }

    pub fn widget_picker_ui(&mut self, ctx: &Context, screen: Rect) {
        let pal = self.pal;
        let cfg = self.cfg.clone();
        let geo = self.geo(ctx, screen);
        let live_clock = self.clock.clone();
        let battery = self.battery;
        let mut add: Option<WidgetKind> = None;
        let mut close = false;
        let mut picker_sc = std::mem::take(&mut self.picker_scroller);
        let k = ctx.pixels_per_point();
        let it = self.insets.top / k;
        let ib = self.insets.bottom / k;
        Area::new(Id::new("widgets")).order(Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
            ui.painter().rect_filled(screen, 0, Color32::from_black_alpha(120));
            let sheet = Rect::from_min_max(pos2(screen.left(), screen.top() + it + 60.0), screen.max);
            if ui.interact(Rect::from_min_max(screen.min, pos2(screen.right(), sheet.top())), Id::new("widgets-bg"), Sense::click()).clicked() {
                close = true;
            }
            ui.painter().rect_filled(sheet, egui::CornerRadius { nw: 28, ne: 28, sw: 0, se: 0 }, pal.sheet);
            ui.painter().text(pos2(sheet.left() + 24.0, sheet.top() + 30.0), Align2::LEFT_CENTER, "Widgets", fonts::body(&cfg, 22.0, true), pal.text);
            let body = Rect::from_min_max(pos2(sheet.left() + 12.0, sheet.top() + 64.0), pos2(sheet.right() - 12.0, sheet.bottom() - ib));
            ui.scope_builder(UiBuilder::new().max_rect(body), |ui| {
                crate::scroll::show(ui, &mut picker_sc, "picker", body, true, |ui| {
                    for kind in WidgetKind::ALL {
                        let (w, h) = kind.span(cfg.cols);
                        let size = vec2(w as f32 * geo.cell.x, h as f32 * geo.cell.y);
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new(kind.name()).font(fonts::body(&cfg, 14.0, true)).color(pal.dim));
                        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
                        let backdrop = if pal.light { Color32::from_gray(120) } else { Color32::from_gray(40) };
                        ui.painter().rect_filled(rect.expand(4.0), cr(26.0), backdrop.gamma_multiply(0.35));
                        let mut preview_cfg = cfg.clone();
                        preview_cfg.font_scale = cfg.font_scale;
                        let live = cards::Live { clock: &live_clock, battery };
                        let mut ppal = pal;
                        ppal.home_text = pal.text;
                        cards::paint(&ui.painter_at(rect.expand(4.0)), rect, kind, &preview_cfg, &ppal, &live);
                        if resp.clicked() {
                            add = Some(kind);
                        }
                        ui.add_space(8.0);
                    }
                    ui.add_space(30.0);
                });
            });
        });
        self.picker_scroller = picker_sc;
        if let Some(kind) = add {
            let id = self.cfg.new_id();
            let pi = (self.page.round() as usize).min(self.cfg.pages.len().saturating_sub(1));
            let item = Item::Widget { id, kind };
            let (w, h) = item.span(self.cfg.cols);
            if let Some((c, r)) = self.cfg.free_spot(pi, w.min(self.cfg.cols), h.min(self.cfg.rows)) {
                self.cfg.place(pi, c, r, item);
            } else {
                self.cfg.insert_auto(item);
            }
            self.widget_picker = false;
            self.edit = false;
            self.dirty = true;
            self.toast(ctx, "Widget added");
        }
        if close {
            self.widget_picker = false;
        }
    }

    pub fn toast_ui(&mut self, ctx: &Context, screen: Rect) {
        let Some((text, until)) = self.toast.clone() else { return };
        let now = ctx.input(|i| i.time);
        if now > until {
            self.toast = None;
            return;
        }
        let pal = self.pal;
        let cfg = self.cfg.clone();
        let k = ctx.pixels_per_point();
        let ib = self.insets.bottom / k;
        Area::new(Id::new("toast")).order(Order::Tooltip).fixed_pos(pos2(screen.center().x, screen.bottom() - ib - 120.0)).pivot(Align2::CENTER_CENTER).show(ctx, |ui| {
            Frame::NONE.fill(Color32::from_black_alpha(220)).corner_radius(cr(22.0)).inner_margin(egui::Margin::symmetric(18, 12)).show(ui, |ui| {
                ui.label(egui::RichText::new(text).font(fonts::body(&cfg, 14.5, false)).color(Color32::WHITE));
            });
        });
        let _ = pal;
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}
