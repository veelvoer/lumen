//! Settings. Every change is saved at once and applied live.

use crate::app::{Lumen, Screen};
use crate::config::*;
use crate::fonts;
use crate::phosphor as ph;
use crate::platform;
use crate::theme::rgb;
use crate::update;
use crate::widgets::{self, cr};
use egui::{pos2, vec2, Align2, Color32, Context, Frame, Rect, RichText, Sense, Stroke, StrokeKind, Ui};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SettingsPage {
    Look,
    Home,
    Icons,
    Dock,
    Drawer,
    Gestures,
    Motion,
    Layout,
    Behavior,
    Performance,
    Badges,
    Backup,
    Updates,
    About,
}

impl SettingsPage {
    fn title(self) -> &'static str {
        match self {
            SettingsPage::Look => "Look and colors",
            SettingsPage::Home => "Home screen",
            SettingsPage::Icons => "Icons",
            SettingsPage::Dock => "Dock",
            SettingsPage::Drawer => "App drawer",
            SettingsPage::Gestures => "Gestures",
            SettingsPage::Motion => "Animations and feel",
            SettingsPage::Layout => "Layout and sizes",
            SettingsPage::Behavior => "Behavior",
            SettingsPage::Performance => "Performance",
            SettingsPage::Badges => "Notification badges",
            SettingsPage::Backup => "Backup and reset",
            SettingsPage::Updates => "Updates",
            SettingsPage::About => "About",
        }
    }
}

struct Ctl {
    ctx: Context,
    pal: crate::theme::Palette,
    fcfg: crate::app::Cfg,
    changed: bool,
    /// a change that does not alter the look (behavior, gestures, performance) keeps the preset's name
    keep_preset: bool,
    /// a page is sliding: taps that would navigate are ignored, so rapid tapping cannot glitch
    busy: bool,
    restyle_preset: bool,
    preset: Option<Preset>,
    toast: Option<String>,
}

impl Ctl {
    fn body(&self, size: f32, medium: bool) -> egui::FontId {
        fonts::body(&self.fcfg, size, medium)
    }
}

// ───────────────────────────── building blocks ─────────────────────────────

fn section(ui: &mut Ui, c: &Ctl, title: &str) {
    ui.add_space(18.0);
    ui.label(RichText::new(title).font(c.body(13.5, true)).color(c.pal.accent));
    ui.add_space(4.0);
}

fn tonal_icon(p: &egui::Painter, center: egui::Pos2, glyph: &str, c: &Ctl) {
    p.circle_filled(center, 21.0, c.pal.accent_soft);
    widgets::glyph_at(p, center, Align2::CENTER_CENTER, glyph, 22.0, if c.pal.light { c.pal.accent } else { c.pal.accent });
}

/// A row you can tap to open a page. Returns true when tapped.
fn nav_row(ui: &mut Ui, c: &Ctl, glyph: &str, title: &str, sub: &str, badge: bool) -> bool {
    let ok = !c.busy;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 68.0), Sense::click());
    let p = ui.painter();
    if resp.is_pointer_button_down_on() {
        p.rect_filled(rect, cr(18.0), c.pal.card);
    }
    tonal_icon(p, rect.left_center() + vec2(30.0, 0.0), glyph, c);
    p.text(rect.left_center() + vec2(66.0, if sub.is_empty() { 0.0 } else { -10.0 }), Align2::LEFT_CENTER, title, c.body(16.5, false), c.pal.text);
    if !sub.is_empty() {
        let font = c.body(13.0, false);
        let shown = widgets::fit(p, sub, &font, rect.width() - 66.0 - 44.0);
        p.text(rect.left_center() + vec2(66.0, 11.0), Align2::LEFT_CENTER, shown, font, c.pal.dim);
    }
    widgets::glyph_at(p, rect.right_center() - vec2(14.0, 0.0), Align2::RIGHT_CENTER, ph::CARET_RIGHT, 18.0, c.pal.dim);
    if badge {
        p.circle_filled(rect.right_center() - vec2(46.0, 0.0), 5.0, c.pal.accent);
    }
    resp.clicked() && ok
}

fn switch_row(ui: &mut Ui, c: &mut Ctl, title: &str, sub: &str, v: &mut bool) {
    let h = if sub.is_empty() { 56.0 } else { 66.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
    if resp.clicked() {
        *v = !*v;
        c.changed = true;
    }
    let t = ui.ctx().animate_bool_with_time(resp.id, *v, 0.15);
    let p = ui.painter();
    p.text(rect.left_center() + vec2(6.0, if sub.is_empty() { 0.0 } else { -10.0 }), Align2::LEFT_CENTER, title, c.body(16.0, false), c.pal.text);
    if !sub.is_empty() {
        p.text(rect.left_center() + vec2(6.0, 11.0), Align2::LEFT_CENTER, sub, c.body(13.0, false), c.pal.dim);
    }
    let track = Rect::from_center_size(rect.right_center() - vec2(34.0, 0.0), vec2(52.0, 30.0));
    let off = if c.pal.light { Color32::from_gray(200) } else { Color32::from_gray(80) };
    let col = Color32::from_rgb(
        (off.r() as f32 + (c.pal.accent.r() as f32 - off.r() as f32) * t) as u8,
        (off.g() as f32 + (c.pal.accent.g() as f32 - off.g() as f32) * t) as u8,
        (off.b() as f32 + (c.pal.accent.b() as f32 - off.b() as f32) * t) as u8,
    );
    p.rect_filled(track, cr(15.0), col);
    let thumb_x = track.left() + 15.0 + t * 22.0;
    p.circle_filled(pos2(thumb_x, track.center().y), 11.0 + 2.0 * t, if t > 0.5 { c.pal.on_accent } else { Color32::WHITE });
}

fn slider_row(ui: &mut Ui, c: &mut Ctl, title: &str, v: &mut f32, range: std::ops::RangeInclusive<f32>, shown: String) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).font(c.body(16.0, false)).color(c.pal.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(shown).font(c.body(14.0, false)).color(c.pal.dim));
        });
    });
    ui.spacing_mut().slider_width = ui.available_width() - 16.0;
    if ui.add(egui::Slider::new(v, range).show_value(false)).changed() {
        c.changed = true;
    }
    ui.add_space(4.0);
}

fn usize_row(ui: &mut Ui, c: &mut Ctl, title: &str, v: &mut usize, range: std::ops::RangeInclusive<usize>) {
    let mut f = *v as f32;
    let shown = format!("{}", *v);
    slider_row(ui, c, title, &mut f, *range.start() as f32..=*range.end() as f32, shown);
    let n = f.round() as usize;
    if n != *v {
        *v = n;
        c.changed = true;
    }
}

fn chips<T: PartialEq + Copy>(ui: &mut Ui, c: &mut Ctl, cur: &mut T, options: &[(T, &str)]) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
        for (v, name) in options {
            let on = *cur == *v;
            let b = egui::Button::new(RichText::new(*name).font(c.body(14.5, false)).color(if on { c.pal.on_accent } else { c.pal.text }))
                .fill(if on { c.pal.accent } else { c.pal.card })
                .corner_radius(cr(20.0))
                .min_size(vec2(0.0, 38.0));
            if ui.add(b).clicked() && !on {
                *cur = *v;
                c.changed = true;
            }
        }
    });
    ui.add_space(6.0);
}

fn label(ui: &mut Ui, c: &Ctl, text: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(text).font(c.body(16.0, false)).color(c.pal.text));
    ui.add_space(2.0);
}

fn note(ui: &mut Ui, c: &Ctl, text: &str) {
    ui.label(RichText::new(text).font(c.body(13.0, false)).color(c.pal.dim));
}

fn button(ui: &mut Ui, c: &Ctl, glyph: &str, text: &str, primary: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 52.0), Sense::click());
    let p = ui.painter();
    let bg = if primary { c.pal.accent } else { c.pal.card };
    let fg = if primary { c.pal.on_accent } else { c.pal.text };
    p.rect_filled(rect, cr(26.0), if resp.is_pointer_button_down_on() { bg.gamma_multiply(0.8) } else { bg });
    widgets::glyph_at(p, rect.left_center() + vec2(22.0, 0.0), Align2::LEFT_CENTER, glyph, 21.0, fg);
    p.text(rect.left_center() + vec2(56.0, 0.0), Align2::LEFT_CENTER, text, c.body(15.5, false), fg);
    ui.add_space(8.0);
    resp.clicked()
}

const SWATCHES: [[u8; 3]; 10] = [
    [138, 180, 248],
    [10, 132, 255],
    [215, 25, 33],
    [255, 112, 67],
    [255, 193, 7],
    [52, 168, 83],
    [0, 188, 212],
    [156, 39, 176],
    [233, 30, 99],
    [158, 158, 158],
];

fn swatches(ui: &mut Ui, c: &mut Ctl, cur: &mut [u8; 3]) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
        for s in SWATCHES {
            let (r, resp) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::click());
            ui.painter().circle_filled(r.center(), 18.0, rgb(s));
            if *cur == s {
                ui.painter().circle_stroke(r.center(), 21.0, Stroke::new(2.5, c.pal.text));
            }
            if resp.clicked() {
                *cur = s;
                c.changed = true;
            }
        }
        if ui.color_edit_button_srgb(cur).changed() {
            c.changed = true;
        }
    });
    ui.add_space(6.0);
}

// ───────────────────────────── entry ─────────────────────────────

pub fn show(l: &mut Lumen, root: &mut Ui) {
    let _ = root;
    let ctx = root.ctx().clone();
    let screen = ctx.content_rect();
    let k = ctx.pixels_per_point();
    let (top, bottom) = (l.insets.top / k, l.insets.bottom / k);
    let mut c = Ctl { ctx: ctx.clone(), pal: l.pal, fcfg: l.cfg.clone(), changed: false, keep_preset: true, busy: false, restyle_preset: false, preset: None, toast: None };
    let before = (l.cfg.cols, l.cfg.rows, l.cfg.dock_slots);

    // how the whole screen arrives: iOS slides in from the right, Pixel rises and fades, Nothing just fades
    let t = l.screen_t.x;
    let tc = t.clamp(0.0, 1.0);
    let (off, alpha) = match l.cfg.preset {
        Preset::Ios => (vec2((1.0 - t) * screen.width(), 0.0), 1.0),
        Preset::Pixel | Preset::Custom => (vec2(0.0, (1.0 - tc) * 56.0), tc),
        Preset::Nothing => (vec2(0.0, 0.0), tc),
    };

    // sub pages push and pop with the same spring
    l.depth.target = l.settings_stack.len() as f32;
    let depth = l.depth.x.max(0.0);
    // pages that are leaving stay drawn until the spring settles; the live stack always wins where it has a page
    if !l.depth.moving() {
        l.settings_view = l.settings_stack.clone();
    }
    let mut view = l.settings_stack.clone();
    for (i, p) in l.settings_view.iter().enumerate() {
        if i >= view.len() {
            view.push(*p);
        }
    }
    l.settings_view = view.clone();
    c.busy = (l.depth.x - l.depth.target).abs() > 0.2 || (l.screen_t.x - l.screen_t.target).abs() > 0.2;
    let (k0, k1) = (depth.floor() as usize, depth.ceil() as usize);
    let mut shown = vec![k0];
    if k1 != k0 {
        shown.push(k1);
    }
    for pk in shown {
        let page = if pk == 0 { None } else { view.get(pk - 1).copied() };
        let dx = if (pk as f32) < depth { (pk as f32 - depth) * screen.width() * 0.3 } else { (pk as f32 - depth) * screen.width() };
        let a = alpha * if (pk as f32) < depth { 1.0 - 0.4 * (depth - pk as f32).min(1.0) } else { 1.0 };
        let origin = screen.min + off + vec2(dx, 0.0);
        let rect = Rect::from_min_size(origin, screen.size());
        let id = egui::Id::new(("lumen-settings", pk));
        egui::Area::new(id).order(egui::Order::Foreground).fixed_pos(origin).constrain(false).show(&ctx, |ui| {
            ui.set_clip_rect(screen);
            ui.set_opacity(a);
            ui.painter().rect_filled(rect, 0, c.pal.sheet);
            ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                ui.add_space(top + 4.0);
                // app bar
                let (bar, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 56.0), Sense::click());
                let p = ui.painter();
                widgets::glyph_at(p, bar.left_center() + vec2(30.0, 0.0), Align2::CENTER_CENTER, ph::ARROW_LEFT, 24.0, c.pal.text);
                p.text(bar.left_center() + vec2(66.0, 0.0), Align2::LEFT_CENTER, page.map(|p| p.title()).unwrap_or("Lumen"), c.body(22.0, true), c.pal.text);
                if resp.clicked() && !c.busy && ui.input(|i| i.pointer.interact_pos().is_some_and(|p| p.x - rect.left() < 120.0)) {
                    if l.settings_stack.pop().is_none() {
                        l.screen = Screen::Home;
                    }
                }
                if l.settings_scrollers.len() <= pk {
                    l.settings_scrollers.resize_with(pk + 1, Default::default);
                }
                let mut sc = std::mem::take(&mut l.settings_scrollers[pk]);
                let body = ui.available_rect_before_wrap();
                crate::scroll::show(ui, &mut sc, ("settings-scroll", pk), body, tc > 0.98, |ui| {
                    Frame::NONE.inner_margin(egui::Margin::symmetric(18, 0)).show(ui, |ui| {
                        c.keep_preset = matches!(page, None | Some(SettingsPage::Performance | SettingsPage::Behavior | SettingsPage::Gestures | SettingsPage::Backup | SettingsPage::Updates | SettingsPage::About));
                        let was_changed = c.changed;
                        match page {
                            None => main_page(ui, l, &mut c),
                            Some(SettingsPage::Look) => look_page(ui, l, &mut c),
                            Some(SettingsPage::Home) => home_page(ui, l, &mut c),
                            Some(SettingsPage::Icons) => icons_page(ui, l, &mut c),
                            Some(SettingsPage::Dock) => dock_page(ui, l, &mut c),
                            Some(SettingsPage::Drawer) => drawer_page(ui, l, &mut c),
                            Some(SettingsPage::Gestures) => gestures_page(ui, l, &mut c),
                            Some(SettingsPage::Motion) => motion_page(ui, l, &mut c),
                            Some(SettingsPage::Layout) => layout_page(ui, l, &mut c),
                            Some(SettingsPage::Behavior) => behavior_page(ui, l, &mut c),
                            Some(SettingsPage::Performance) => performance_page(ui, l, &mut c),
                            Some(SettingsPage::Badges) => badges_page(ui, l, &mut c),
                            Some(SettingsPage::Backup) => backup_page(ui, l, &mut c),
                            Some(SettingsPage::Updates) => updates_page(ui, l, &mut c),
                            Some(SettingsPage::About) => about_page(ui, l, &mut c),
                        }
                        if c.changed && !was_changed && !c.keep_preset {
                            c.restyle_preset = true;
                        }
                        ui.add_space(bottom + 48.0);
                    });
                });
                l.settings_scrollers[pk] = sc;
            });
        });
    }

    if let Some(p) = c.preset {
        l.cfg.apply_preset(p);
        c.changed = true;
    } else if c.restyle_preset && l.cfg.preset != Preset::Custom {
        l.cfg.preset = Preset::Custom;
    }
    if c.changed {
        if (l.cfg.cols, l.cfg.rows, l.cfg.dock_slots) != before {
            let items = l.cfg.reading_order();
            l.cfg.repack(items);
        }
        l.dirty = true;
        l.restyle(&ctx);
    }
    if let Some(t) = c.toast {
        l.toast(&ctx, &t);
    }
}

// ───────────────────────────── pages ─────────────────────────────

fn main_page(ui: &mut Ui, l: &mut Lumen, c: &mut Ctl) {
    if !l.default_home {
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 84.0), Sense::click());
        let p = ui.painter();
        p.rect_filled(rect, cr(24.0), c.pal.accent_soft);
        widgets::glyph_at(p, rect.left_center() + vec2(30.0, 0.0), Align2::CENTER_CENTER, ph::HOUSE_SIMPLE, 26.0, c.pal.accent);
        p.text(rect.left_center() + vec2(64.0, -11.0), Align2::LEFT_CENTER, "Make Lumen your home app", c.body(16.0, true), c.pal.text);
        p.text(rect.left_center() + vec2(64.0, 12.0), Align2::LEFT_CENTER, "Tap here and choose Lumen", c.body(13.0, false), c.pal.dim);
        if resp.clicked() {
            platform::pick_default_home();
        }
        ui.add_space(8.0);
    }

    section(ui, c, "Style");
    preset_cards(ui, l, c);
    ui.add_space(10.0);
    let st = l.upd.state();
    for (glyph, title, sub, page, badge) in [
        (ph::PALETTE, "Look and colors", "Mode, Material You, accent, fonts", SettingsPage::Look, false),
        (ph::HOUSE_SIMPLE, "Home screen", "Grid, search bar, App Library, widgets", SettingsPage::Home, false),
        (ph::SQUARES_FOUR, "Icons", "Size, shape, single-color icons", SettingsPage::Icons, false),
        (ph::ARROW_LINE_DOWN, "Dock", "Style and number of slots", SettingsPage::Dock, false),
        (ph::LIST_BULLETS, "App drawer", "Grid or list, suggestions, hidden apps", SettingsPage::Drawer, false),
        (ph::HAND_SWIPE_RIGHT, "Gestures", "Swipes, double tap, pinch, long press", SettingsPage::Gestures, false),
        (ph::SPARKLE, "Animations and feel", "Spring style, speed, page effects, blur", SettingsPage::Motion, false),
        (ph::LAYOUT, "Layout and sizes", "Margins, dock, labels, widgets, folders", SettingsPage::Layout, false),
        (ph::SLIDERS_HORIZONTAL, "Behavior", "New apps, status bar, rotation, clock format", SettingsPage::Behavior, false),
        (ph::LIGHTNING, "Performance", "Smooth mode for weaker phones, diagnostics", SettingsPage::Performance, false),
        (ph::BELL, "Notification badges", "Dots or counts on icons", SettingsPage::Badges, false),
        (ph::FLOPPY_DISK, "Backup and reset", "Copy your layout, restore it, start over", SettingsPage::Backup, false),
        (ph::DOWNLOAD_SIMPLE, "Updates", &format!("Lumen {}", l.version), SettingsPage::Updates, st.available.is_some()),
        (ph::INFO, "About", "Lumen is open source and written in Rust", SettingsPage::About, false),
    ] {
        if nav_row(ui, c, glyph, title, sub, badge) {
            l.settings_stack.push(page);
        }
    }
}

/// Three little phones that draw the look of each preset.
fn preset_cards(ui: &mut Ui, l: &mut Lumen, c: &mut Ctl) {
    let w = (ui.available_width() - 20.0) / 3.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        for (p, name) in [(Preset::Nothing, "Nothing"), (Preset::Pixel, "Pixel"), (Preset::Ios, "iOS")] {
            ui.vertical(|ui| {
                let (rect, resp) = ui.allocate_exact_size(vec2(w, w * 1.9), Sense::click());
                let on = l.cfg.preset == p;
                paint_preview(ui.painter(), rect, p, &c.pal, &l.cfg);
                if on {
                    ui.painter().rect_stroke(rect.expand(2.0), cr(22.0), Stroke::new(3.0, c.pal.accent), StrokeKind::Outside);
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    ui.label(RichText::new(name).font(c.body(14.5, on)).color(if on { c.pal.accent } else { c.pal.text }));
                });
                if resp.clicked() {
                    c.preset = Some(p);
                }
            });
        }
    });
}

fn paint_preview(p: &egui::Painter, r: Rect, preset: Preset, _pal: &crate::theme::Palette, _cfg: &Config) {
    let (top, bot) = match preset {
        Preset::Nothing => (Color32::from_rgb(20, 20, 22), Color32::from_rgb(0, 0, 0)),
        Preset::Pixel => (Color32::from_rgb(86, 125, 190), Color32::from_rgb(34, 52, 96)),
        _ => (Color32::from_rgb(255, 154, 90), Color32::from_rgb(120, 60, 190)),
    };
    // rounded card with a vertical gradient: round caps, square gradient in between
    let cap = 20.0;
    p.rect_filled(Rect::from_min_max(r.min, pos2(r.right(), r.top() + cap * 2.0)), egui::CornerRadius { nw: 20, ne: 20, sw: 0, se: 0 }, top);
    p.rect_filled(Rect::from_min_max(pos2(r.left(), r.bottom() - cap * 2.0), r.max), egui::CornerRadius { nw: 0, ne: 0, sw: 20, se: 20 }, bot);
    let lerp = |t: f32| Color32::from_rgb((top.r() as f32 + (bot.r() as f32 - top.r() as f32) * t) as u8, (top.g() as f32 + (bot.g() as f32 - top.g() as f32) * t) as u8, (top.b() as f32 + (bot.b() as f32 - top.b() as f32) * t) as u8);
    let (y0, y1) = (r.top() + cap, r.bottom() - cap);
    let (t0, t1) = (cap / r.height(), 1.0 - cap / r.height());
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(pos2(r.left(), y0), lerp(t0));
    mesh.colored_vertex(pos2(r.right(), y0), lerp(t0));
    mesh.colored_vertex(pos2(r.right(), y1), lerp(t1));
    mesh.colored_vertex(pos2(r.left(), y1), lerp(t1));
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(egui::Shape::mesh(mesh));
    let clip = p.with_clip_rect(r);
    let corner = Color32::TRANSPARENT;
    let _ = corner;
    let w = r.width();
    let icon = w * 0.17;
    let pad = w * 0.09;
    let cols = 4;
    let gap = (w - 2.0 * pad - cols as f32 * icon) / (cols as f32 - 1.0);
    let cols_c = [Color32::from_rgb(66, 133, 244), Color32::from_rgb(52, 168, 83), Color32::from_rgb(251, 188, 5), Color32::from_rgb(234, 67, 53)];
    let draw_icons = |row_y: f32, n: usize| {
        for i in 0..n {
            let rect = Rect::from_min_size(pos2(r.left() + pad + i as f32 * (icon + gap), row_y), vec2(icon, icon));
            match preset {
                Preset::Nothing => {
                    clip.circle_filled(rect.center(), icon / 2.0, Color32::from_gray(26));
                    clip.circle_stroke(rect.center(), icon * 0.18, Stroke::new(1.4, Color32::WHITE));
                }
                Preset::Pixel => {
                    clip.circle_filled(rect.center(), icon / 2.0, cols_c[i % 4]);
                }
                _ => {
                    clip.rect_filled(rect, cr(icon * 0.225), cols_c[i % 4]);
                }
            }
        }
    };
    // top: signature widget
    match preset {
        Preset::Nothing => {
            let card = Rect::from_min_size(r.min + vec2(pad, w * 0.2), vec2(w - 2.0 * pad, w * 0.42));
            clip.rect_filled(card, cr(10.0), Color32::from_black_alpha(190));
            crate::widgets::dot_text(&clip, pos2(card.center().x - crate::widgets::dot_text_width("10:24", w * 0.026) / 2.0, card.top() + w * 0.07), "10:24", w * 0.026, Color32::WHITE, Color32::from_rgb(215, 25, 33), false);
        }
        Preset::Pixel => {
            clip.text(r.min + vec2(pad, w * 0.2), Align2::LEFT_TOP, "Sat, Oct 24", egui::FontId::proportional(w * 0.12), Color32::WHITE);
        }
        _ => {
            for i in 0..2 {
                let card = Rect::from_min_size(r.min + vec2(pad + i as f32 * (w * 0.42 + w * 0.06), w * 0.18), vec2(w * 0.42, w * 0.42));
                clip.rect_filled(card, cr(10.0), if i == 0 { Color32::from_white_alpha(230) } else { Color32::from_black_alpha(180) });
            }
        }
    }
    draw_icons(r.top() + w * 0.66, 4);
    draw_icons(r.top() + w * 0.66 + w * 0.25, 4);
    draw_icons(r.top() + w * 0.66 + w * 0.5, 2);
    // dock / search
    match preset {
        Preset::Pixel => {
            draw_icons(r.bottom() - icon * 2.2 - 6.0, 4);
            clip.rect_filled(Rect::from_min_size(pos2(r.left() + pad, r.bottom() - icon * 1.05 - 4.0), vec2(w - 2.0 * pad, icon * 0.9)), cr(icon * 0.45), Color32::from_black_alpha(130));
        }
        Preset::Nothing => draw_icons(r.bottom() - icon * 1.4, 4),
        _ => {
            let dock = Rect::from_min_size(pos2(r.left() + pad * 0.6, r.bottom() - icon * 1.6), vec2(w - 1.2 * pad, icon * 1.25));
            clip.rect_filled(dock, cr(icon * 0.5), Color32::from_white_alpha(70));
            draw_icons(dock.top() + icon * 0.12, 4);
        }
    }
}

fn look_page(ui: &mut Ui, l: &mut Lumen, c: &mut Ctl) {
    section(ui, c, "Theme");
    chips(ui, c, &mut l.cfg.mode, &[(Mode::Auto, "Follow phone"), (Mode::Dark, "Dark"), (Mode::Light, "Light")]);
    let has_dyn = l.phone_colors.is_some();
    let mut dynamic = l.cfg.dynamic && has_dyn;
    switch_row(ui, c, "Material You colors", if has_dyn { "Use the colors of your wallpaper" } else { "Needs Android 12 or newer" }, &mut dynamic);
    if has_dyn {
        l.cfg.dynamic = dynamic;
    }
    if !(l.cfg.dynamic && has_dyn) {
        label(ui, c, "Accent color");
        swatches(ui, c, &mut l.cfg.accent);
    }
    section(ui, c, "Frosted glass (iOS style)");
    switch_row(ui, c, "Frosted glass", "Dock and search blur your actual wallpaper", &mut l.cfg.real_glass);
    if l.cfg.real_glass {
        if l.wallpaper_live {
            note(ui, c, "Not available with a live wallpaper. Pick a normal picture to see it.");
        } else if !l.wallpaper_access {
            note(ui, c, "Android only lets an app read your wallpaper with \"All files access\". Lumen uses it for nothing else, and the picture never leaves your phone.");
            ui.add_space(8.0);
            if button(ui, c, ph::IMAGE, "Allow wallpaper access", true) {
                platform::request_wallpaper_access();
            }
        } else if l.light {
            note(ui, c, "Turned off by light mode (Settings > Performance).");
        } else if l.glass.active() {
            note(ui, c, "On. It shows on the iOS dock and the Search capsule.");
        } else {
            note(ui, c, "Reading your wallpaper…");
        }
    }
    section(ui, c, "Wallpaper");
    switch_row(ui, c, "Paint the wallpaper in Lumen", "Makes coming back from an app instant instead of showing an empty screen for a moment. Needs wallpaper access and a normal picture; takes effect the next time Lumen starts.", &mut l.cfg.own_wallpaper);
    let shown = format!("{}%", (l.cfg.wallpaper_dim * 100.0) as i32);
    slider_row(ui, c, "Dim the wallpaper", &mut l.cfg.wallpaper_dim, 0.0..=0.8, shown);
    switch_row(ui, c, "Scroll the wallpaper with the pages", "", &mut l.cfg.wallpaper_scroll);
    if button(ui, c, ph::IMAGE, "Change wallpaper", false) {
        platform::open_wallpaper_picker();
    }
    section(ui, c, "Text");
    chips(ui, c, &mut l.cfg.font, &[(FontChoice::Preset, "Match the style"), (FontChoice::GoogleSans, "Google Sans"), (FontChoice::Inter, "Inter"), (FontChoice::Mono, "Mono")]);
    let shown = format!("{}%", (l.cfg.font_scale * 100.0) as i32);
    slider_row(ui, c, "Text size", &mut l.cfg.font_scale, 0.8..=1.3, shown);
}

fn home_page(ui: &mut Ui, l: &mut Lumen, c: &mut Ctl) {
    section(ui, c, "Grid");
    usize_row(ui, c, "Columns", &mut l.cfg.cols, 3..=6);
    usize_row(ui, c, "Rows", &mut l.cfg.rows, 3..=8);
    section(ui, c, "Search bar");
    chips(ui, c, &mut l.cfg.search, &[(Search::Off, "Off"), (Search::Top, "Top"), (Search::Bottom, "Bottom")]);
    section(ui, c, "More");
    switch_row(ui, c, "Show app names", "", &mut l.cfg.labels);
    switch_row(ui, c, "Page dots", "", &mut l.cfg.page_dots);
    let mut lib = l.cfg.app_library;
    switch_row(ui, c, "App Library page", "All apps sorted into categories, like iOS", &mut lib);
    l.cfg.app_library = lib;
    switch_row(ui, c, "Lock the layout", "Stops icons from being moved by accident", &mut l.cfg.lock_layout);
    ui.add_space(8.0);
    if button(ui, c, ph::SQUARES_FOUR, "Add a widget", true) {
        l.screen = Screen::Home;
        l.widget_picker = true;
    }
}

fn icons_page(ui: &mut Ui, l: &mut Lumen, c: &mut Ctl) {
    section(ui, c, "Shape");
    chips(ui, c, &mut l.cfg.shape, &[(Shape::Circle, "Circle"), (Shape::Squircle, "Squircle"), (Shape::Rounded, "Rounded"), (Shape::Square, "Square")]);
    if l.cfg.shape == Shape::Rounded {
        let shown = format!("{}%", (l.cfg.icon_radius * 100.0) as i32);
        slider_row(ui, c, "Roundness", &mut l.cfg.icon_radius, 0.0..=1.0, shown);
    }
    section(ui, c, "Size");
    let shown = format!("{} dp", l.cfg.icon_size as i32);
    slider_row(ui, c, "Icon size", &mut l.cfg.icon_size, 36.0..=76.0, shown);
    section(ui, c, "Style");
    switch_row(ui, c, "Single-color icons", "Uses themed icons where the app has them", &mut l.cfg.themed_icons);
    switch_row(ui, c, "Show app names", "", &mut l.cfg.labels);
}

fn dock_page(ui: &mut Ui, l: &mut Lumen, c: &mut Ctl) {
    section(ui, c, "Style");
    chips(ui, c, &mut l.cfg.dock, &[(DockStyle::Bare, "None"), (DockStyle::Glass, "Glass"), (DockStyle::Solid, "Solid")]);
    if l.cfg.dock != DockStyle::Bare {
        let shown = format!("{}", l.cfg.dock_radius as i32);
        slider_row(ui, c, "Corner radius", &mut l.cfg.dock_radius, 0.0..=48.0, shown);
    }
    usize_row(ui, c, "Slots", &mut l.cfg.dock_slots, 3..=6);
}

fn drawer_page(ui: &mut Ui, l: &mut Lumen, c: &mut Ctl) {
    section(ui, c, "Layout");
    chips(ui, c, &mut l.cfg.drawer, &[(DrawerStyle::Grid, "Grid"), (DrawerStyle::List, "List")]);
    if l.cfg.drawer == DrawerStyle::Grid {
        usize_row(ui, c, "Columns", &mut l.cfg.drawer_cols, 3..=6);
    }
    switch_row(ui, c, "Suggested apps", "Your most used apps at the top", &mut l.cfg.suggestions);
    section(ui, c, "Order");
    chips(ui, c, &mut l.cfg.drawer_sort, &[(Sort::Alphabetical, "A to Z"), (Sort::MostUsed, "Most used first")]);
    switch_row(ui, c, "Letter scroller on the side", "", &mut l.cfg.drawer_scrubber);
    section(ui, c, "Search bar");
    chips(ui, c, &mut l.cfg.drawer_search, &[(SearchPos::Auto, "Match the style"), (SearchPos::Top, "Top"), (SearchPos::Bottom, "Bottom")]);
    section(ui, c, "Background");
    let shown = format!("{}%", (l.cfg.drawer_opacity * 100.0) as i32);
    slider_row(ui, c, "Opacity", &mut l.cfg.drawer_opacity, 0.5..=1.0, shown);
    section(ui, c, "Hidden apps");
    if l.cfg.hidden.is_empty() {
        note(ui, c, "No hidden apps. Long-press an app in the drawer and choose Hide.");
    } else {
        let hidden = l.cfg.hidden.clone();
        for pkg in hidden {
            let name = l.label_of(&pkg);
            ui.horizontal(|ui| {
                ui.label(RichText::new(if name.is_empty() { pkg.clone() } else { name }).font(c.body(15.0, false)).color(c.pal.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(RichText::new("Show").color(c.pal.accent)).fill(Color32::TRANSPARENT)).clicked() {
                        l.cfg.hidden.retain(|p| *p != pkg);
                        c.changed = true;
                    }
                });
            });
        }
    }
}

fn gestures_page(ui: &mut Ui, l: &mut Lumen, c: &mut Ctl) {
    section(ui, c, "Swipe up on the home screen");
    let up = [(Action::Drawer, "App drawer"), (Action::Search, "Search"), (Action::Notifications, "Notifications"), (Action::Settings, "Lumen settings"), (Action::None, "Nothing")];
    chips(ui, c, &mut l.cfg.swipe_up, &up);
    section(ui, c, "Swipe down on the home screen");
    chips(ui, c, &mut l.cfg.swipe_down, &Action::ALL.iter().filter(|(a, _)| *a != Action::Drawer).copied().collect::<Vec<_>>());
    section(ui, c, "Double tap on empty space");
    chips(ui, c, &mut l.cfg.double_tap, &Action::ALL.iter().filter(|(a, _)| *a != Action::Drawer).copied().collect::<Vec<_>>());
    section(ui, c, "Long press on empty space");
    chips(ui, c, &mut l.cfg.long_press_empty, &Action::ALL.iter().filter(|(a, _)| !matches!(a, Action::Drawer | Action::LockScreen)).copied().collect::<Vec<_>>());
    section(ui, c, "Pinch with two fingers");
    chips(ui, c, &mut l.cfg.pinch, &Action::ALL.iter().filter(|(a, _)| *a != Action::LockScreen).copied().collect::<Vec<_>>());
    if l.cfg.double_tap == Action::LockScreen || l.cfg.swipe_down == Action::LockScreen {
        ui.add_space(6.0);
        if platform::lock_enabled() {
            note(ui, c, "Locking is ready.");
        } else {
            note(ui, c, "Locking needs Lumen turned on in Accessibility. Lumen does not read your screen; it can only press the lock button.");
            if button(ui, c, ph::LOCK_SIMPLE, "Open Accessibility settings", false) {
                platform::ask_lock_access();
            }
        }
    }
}

fn badges_page(ui: &mut Ui, l: &mut Lumen, c: &mut Ctl) {
    section(ui, c, "Show on icons");
    chips(ui, c, &mut l.cfg.badges, &[(Badges::Off, "Off"), (Badges::Dot, "Dot"), (Badges::Count, "Count")]);
    section(ui, c, "Access");
    if platform::badge_access() {
        note(ui, c, "Lumen can count your notifications. It never reads what they say.");
    } else {
        note(ui, c, "Android needs your permission before Lumen can count notifications.");
        if button(ui, c, ph::BELL, "Allow notification access", true) {
            platform::ask_badge_access();
        }
    }
}

fn backup_page(ui: &mut Ui, l: &mut Lumen, c: &mut Ctl) {
    section(ui, c, "Backup");
    note(ui, c, "Copies your whole setup (look, layout, folders, names) as text. Paste it anywhere to keep it safe.");
    ui.add_space(8.0);
    if button(ui, c, ph::COPY_SIMPLE, "Copy backup", false) {
        if let Ok(s) = serde_json::to_string(&*l.cfg) {
            platform::clipboard_set(&s);
            c.toast = Some("Backup copied".into());
        }
    }
    if button(ui, c, ph::CLIPBOARD_TEXT, "Restore from clipboard", false) {
        match serde_json::from_str::<Config>(&platform::clipboard_get()) {
            Ok(cfg) => {
                l.cfg = crate::app::Cfg::new(cfg);
                l.cfg.seeded = true;
                c.changed = true;
                c.toast = Some("Setup restored".into());
            }
            Err(_) => c.toast = Some("The clipboard does not hold a Lumen backup".into()),
        }
    }
    section(ui, c, "Start over");
    if button(ui, c, ph::ARROW_COUNTER_CLOCKWISE, "Reset the home screen", false) {
        let preset = l.cfg.preset;
        l.cfg.names.clear();
        l.cfg.seeded = false;
        l.cfg.pages = vec![crate::config::Page::default()];
        l.cfg.dock_items.clear();
        if preset != Preset::Custom {
            l.cfg.apply_preset(preset);
        }
        l.reseed();
        c.changed = true;
        c.toast = Some("Home screen reset".into());
    }
}

fn updates_page(ui: &mut Ui, l: &mut Lumen, c: &mut Ctl) {
    let st = l.upd.state();
    ui.add_space(8.0);
    ui.label(RichText::new(format!("Lumen {}", l.version)).font(c.body(20.0, true)).color(c.pal.text));
    let status = match (&st.available, st.busy, st.checked) {
        (_, true, _) if st.progress.is_some() => format!("Downloading… {}%", st.progress.unwrap_or(0)),
        (_, true, _) => "Looking…".to_string(),
        (Some(r), _, _) => format!("Version {} is ready", r.version),
        (None, _, true) => "This is the newest version".to_string(),
        _ => String::new(),
    };
    if !status.is_empty() {
        ui.label(RichText::new(status).font(c.body(15.0, false)).color(c.pal.dim));
    }
    if let Some(r) = &st.available {
        ui.add_space(8.0);
        for line in update::plain(&r.notes).iter().take(16) {
            ui.label(RichText::new(line).font(c.body(14.0, false)).color(c.pal.dim));
        }
    }
    if !st.message.is_empty() {
        ui.add_space(6.0);
        ui.label(RichText::new(&st.message).font(c.body(14.0, false)).color(c.pal.accent));
    }
    ui.add_space(12.0);
    let ctx = c.ctx.clone();
    if st.available.is_some() {
        if button(ui, c, ph::DOWNLOAD_SIMPLE, "Update now", !st.busy) && !st.busy {
            l.upd.install(move || ctx.request_repaint());
        }
    } else if button(ui, c, ph::ARROW_CLOCKWISE, "Check for updates", false) && !st.busy {
        l.upd.check(move || ctx.request_repaint());
    }
    if button(ui, c, ph::SPARKLE, "What is new in this version", false) {
        l.show_current_notes();
        l.screen = Screen::Home;
    }
    switch_row(ui, c, "Look for updates on start", "", &mut l.cfg.auto_update);
    if st.busy {
        c.ctx.request_repaint_after(std::time::Duration::from_millis(200));
    }
}

fn about_page(ui: &mut Ui, l: &mut Lumen, c: &mut Ctl) {
    ui.add_space(10.0);
    ui.label(RichText::new(format!("Lumen {}", l.version)).font(c.body(22.0, true)).color(c.pal.text));
    ui.add_space(6.0);
    note(ui, c, "A fast, customizable launcher written in Rust. Fonts: Google Sans, Inter and DM Mono (SIL Open Font License). Icons: Phosphor (MIT).");
    ui.add_space(14.0);
    if button(ui, c, ph::ARROW_SQUARE_OUT, "github.com/veelvoer/lumen", false) {
        platform::open_url("https://github.com/veelvoer/lumen");
    }
}

fn motion_page(ui: &mut Ui, l: &mut Lumen, c: &mut Ctl) {
    section(ui, c, "Spring style");
    note(ui, c, "Auto moves like the preset: bouncy for iOS, smooth for Pixel, quick and mechanical for Nothing.");
    ui.add_space(6.0);
    chips(ui, c, &mut l.cfg.anim_style, &[(AnimStyle::Auto, "Match the style"), (AnimStyle::Smooth, "Smooth"), (AnimStyle::Bouncy, "Bouncy"), (AnimStyle::Snappy, "Snappy"), (AnimStyle::Off, "Off")]);
    let shown = format!("{:.1}x", l.cfg.anim_speed);
    slider_row(ui, c, "Speed", &mut l.cfg.anim_speed, 0.5..=2.0, shown);
    section(ui, c, "Pages");
    chips(ui, c, &mut l.cfg.page_effect, &[(PageEffect::Slide, "Slide"), (PageEffect::Parallax, "Parallax"), (PageEffect::Zoom, "Zoom"), (PageEffect::Fade, "Fade"), (PageEffect::Stack, "Stack")]);
    section(ui, c, "Opening an app");
    chips(ui, c, &mut l.cfg.launch_anim, &[(LaunchAnim::Zoom, "Zoom from the icon"), (LaunchAnim::Reveal, "Reveal"), (LaunchAnim::None, "None")]);
    section(ui, c, "Blur and touch");
    let has_blur = true;
    let shown = format!("{}%", (l.cfg.blur * 100.0) as i32);
    slider_row(ui, c, "Blur behind drawer and folders", &mut l.cfg.blur, 0.0..=1.5, shown);
    if has_blur {
        note(ui, c, "Needs Android 12 or newer. Turned off by the phone in battery saver.");
    }
    ui.add_space(6.0);
    switch_row(ui, c, "Vibrate on pick up and drop", "", &mut l.cfg.haptics);
}

fn layout_page(ui: &mut Ui, l: &mut Lumen, c: &mut Ctl) {
    section(ui, c, "Home screen margins");
    let shown = format!("{} dp", l.cfg.margin_side as i32);
    slider_row(ui, c, "Left and right", &mut l.cfg.margin_side, 0.0..=40.0, shown);
    let shown = format!("{} dp", l.cfg.margin_top as i32);
    slider_row(ui, c, "Top", &mut l.cfg.margin_top, 0.0..=80.0, shown);
    let shown = format!("{} dp", l.cfg.margin_bottom as i32);
    slider_row(ui, c, "Bottom", &mut l.cfg.margin_bottom, 0.0..=80.0, shown);
    section(ui, c, "Dock");
    let shown = format!("{} dp", l.cfg.dock_icon_size as i32);
    slider_row(ui, c, "Dock icon size", &mut l.cfg.dock_icon_size, 36.0..=76.0, shown);
    switch_row(ui, c, "Names under dock icons", "", &mut l.cfg.dock_labels);
    section(ui, c, "Names");
    let shown = format!("{} sp", l.cfg.label_size as i32);
    slider_row(ui, c, "Name size", &mut l.cfg.label_size, 9.0..=16.0, shown);
    switch_row(ui, c, "Shadow behind names", "Helps on bright wallpapers", &mut l.cfg.label_shadow);
    section(ui, c, "Page indicator");
    chips(ui, c, &mut l.cfg.dots_style, &[(DotsStyle::Dots, "Dots"), (DotsStyle::Bars, "Bars"), (DotsStyle::Numbers, "Numbers")]);
    section(ui, c, "Widgets and folders");
    let shown = format!("{} dp", l.cfg.widget_radius as i32);
    slider_row(ui, c, "Widget corner radius", &mut l.cfg.widget_radius, 0.0..=44.0, shown);
    usize_row(ui, c, "Folder columns", &mut l.cfg.folder_cols, 2..=5);
}

fn behavior_page(ui: &mut Ui, l: &mut Lumen, c: &mut Ctl) {
    section(ui, c, "Apps");
    switch_row(ui, c, "Put new apps on the home screen", "Otherwise they wait in the drawer", &mut l.cfg.auto_add);
    switch_row(ui, c, "Lock the layout", "Stops icons from being moved by accident", &mut l.cfg.lock_layout);
    section(ui, c, "Screen");
    switch_row(ui, c, "Hide the status bar", "Swipe from the top edge to see it", &mut l.cfg.hide_status_bar);
    switch_row(ui, c, "Hide the navigation bar", "Swipe from the bottom edge to see it", &mut l.cfg.hide_nav_bar);
    switch_row(ui, c, "Allow rotation", "", &mut l.cfg.rotate);
    section(ui, c, "Clock");
    chips(ui, c, &mut l.cfg.time_format, &[(TimeFormat::Phone, "Like the phone"), (TimeFormat::H24, "24 hour"), (TimeFormat::H12, "12 hour")]);
    section(ui, c, "Home app");
    note(ui, c, if l.default_home { "Lumen is your home app." } else { "Lumen is not your home app yet." });
    ui.add_space(6.0);
    if !l.default_home && button(ui, c, ph::HOUSE_SIMPLE, "Make Lumen the home app", true) {
        platform::pick_default_home();
    }
    if button(ui, c, ph::DEVICE_MOBILE, "Phone settings", false) {
        platform::open_system_settings();
    }
    section(ui, c, "Navigation gestures");
    note(ui, c, "Android only offers swipe navigation when the home app is the one that came with the phone: third-party launchers cannot provide the recent-apps swipe, so the system falls back to the 3-button bar and hides the gesture choice. To change it, switch the home app back for a moment, change the setting, then switch to Lumen again.");
    ui.add_space(8.0);
    if button(ui, c, ph::HAND_SWIPE_RIGHT, "Choose home app", false) {
        platform::pick_default_home();
    }
}

fn performance_page(ui: &mut Ui, l: &mut Lumen, c: &mut Ctl) {
    section(ui, c, "Mode");
    note(ui, c, "Light mode drops blur, shadows, page effects and extra animation so weaker phones stay smooth. Auto picks it for phones that need it and switches on by itself if the phone cannot keep up.");
    ui.add_space(8.0);
    chips(ui, c, &mut l.cfg.perf_mode, &[(PerfMode::Auto, "Auto"), (PerfMode::Full, "Full effects"), (PerfMode::Light, "Light")]);
    ui.add_space(4.0);
    ui.label(RichText::new(if l.light { "Light mode is on right now." } else { "Full effects are on right now." }).font(c.body(15.0, true)).color(c.pal.accent));
    if l.cfg.light_learned && l.cfg.perf_mode == PerfMode::Auto {
        ui.add_space(6.0);
        note(ui, c, "Auto switched to light mode after seeing slow frames.");
        if button(ui, c, ph::ARROW_COUNTER_CLOCKWISE, "Try full effects again", false) {
            l.cfg.light_learned = false;
            c.changed = true;
        }
    }
    section(ui, c, "Frame rate");
    note(ui, c, "Auto asks for the screen's fastest rate and steps down to 90 or 60 by itself if the phone misses frames, because a steady lower rate looks smoother than an unsteady higher one.");
    ui.add_space(8.0);
    let before = l.cfg.refresh_cap;
    chips(ui, c, &mut l.cfg.refresh_cap, &[(0.0, "Auto"), (120.0, "120"), (90.0, "90"), (60.0, "60")]);
    if l.cfg.refresh_cap != before {
        platform::set_refresh_cap(l.cfg.refresh_cap);
        l.bridge.high_refresh(!l.light);
    }
    section(ui, c, "About this phone");
    let report = l.diagnostics(&c.ctx);
    for line in report.lines() {
        ui.label(RichText::new(line).font(fonts::mono(12.5, false)).color(c.pal.dim));
    }
    ui.add_space(12.0);
    if button(ui, c, ph::COPY_SIMPLE, "Copy this report", true) {
        platform::clipboard_set(&report);
        c.toast = Some("Report copied".into());
    }
}
