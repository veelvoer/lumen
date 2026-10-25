//! Drawing helpers shared by the home screen, the drawer and the widgets.

use crate::config::{Badges, Config, Preset, Shape};
use crate::fonts;
use crate::icons::Icons;
use crate::platform::App;
use crate::theme::Palette;
use egui::epaint::Shadow;
use egui::{pos2, vec2, Align2, Color32, CornerRadius, Mesh, Painter, Pos2, Rect, Shape as PShape, Stroke, StrokeKind};

/// 5 columns x 7 rows per glyph. '#' is a lit dot.
const GLYPHS: [(char, [&str; 7]); 13] = [
    ('0', [".###.", "#...#", "#..##", "#.#.#", "##..#", "#...#", ".###."]),
    ('1', ["..#..", ".##..", "..#..", "..#..", "..#..", "..#..", ".###."]),
    ('2', [".###.", "#...#", "....#", "...#.", "..#..", ".#...", "#####"]),
    ('3', ["#####", "...#.", "..#..", "...#.", "....#", "#...#", ".###."]),
    ('4', ["...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#."]),
    ('5', ["#####", "#....", "####.", "....#", "....#", "#...#", ".###."]),
    ('6', ["..##.", ".#...", "#....", "####.", "#...#", "#...#", ".###."]),
    ('7', ["#####", "....#", "...#.", "..#..", ".#...", ".#...", ".#..."]),
    ('8', [".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###."]),
    ('9', [".###.", "#...#", "#...#", ".####", "....#", "...#.", ".###."]),
    (':', ["...", ".#.", ".#.", "...", ".#.", ".#.", "..."]),
    ('%', ["##..#", "##.#.", "...#.", "..#..", ".#...", ".#.##", "#..##"]),
    (' ', [".....", ".....", ".....", ".....", ".....", ".....", "....."]),
];

fn glyph(ch: char) -> &'static [&'static str; 7] {
    &GLYPHS.iter().find(|(c, _)| *c == ch).unwrap_or(&GLYPHS[12]).1
}

const DOT_GAP: f32 = 1.5;

pub fn dot_text_width(text: &str, dot: f32) -> f32 {
    let mut w = 0.0;
    for ch in text.chars() {
        w += glyph(ch)[0].len() as f32 + DOT_GAP;
    }
    (w - DOT_GAP) * dot
}

/// Draws `text` (digits, colon and percent) as round dots. `dot` is the dot pitch in points.
pub fn dot_text(p: &Painter, origin: Pos2, text: &str, dot: f32, color: Color32, colon: Color32, ghost: bool) {
    let mut x = origin.x;
    for ch in text.chars() {
        let g = glyph(ch);
        let cols = g[0].len();
        let col = if ch == ':' { colon } else { color };
        for (r, row) in g.iter().enumerate() {
            for (c, b) in row.bytes().enumerate() {
                let center = pos2(x + (c as f32 + 0.5) * dot, origin.y + (r as f32 + 0.5) * dot);
                if b == b'#' {
                    p.circle_filled(center, dot * 0.38, col);
                } else if ghost && ch != ':' {
                    p.circle_filled(center, dot * 0.09, color.gamma_multiply(0.16));
                }
            }
        }
        x += (cols as f32 + DOT_GAP) * dot;
    }
}

/// Corner radius of the plate / mask in points for an icon of this size.
pub fn plate_radius(cfg: &Config, size: f32) -> f32 {
    match cfg.shape {
        Shape::Circle => size / 2.0,
        Shape::Squircle => size * 0.2237,
        Shape::Rounded => cfg.icon_radius.clamp(0.0, 1.0) * size / 2.0,
        Shape::Square => size * 0.12,
    }
}

pub fn cr(r: f32) -> CornerRadius {
    CornerRadius::same(r.round().clamp(0.0, 255.0) as u8)
}

pub fn shadow(p: &Painter, rect: Rect, radius: f32, strength: u8) {
    p.add(Shadow { offset: [0, 6], blur: 20, spread: 0, color: Color32::from_black_alpha(strength) }.as_shape(rect, cr(radius)));
}

fn textured(p: &Painter, tex: egui::TextureId, rect: Rect, tint: Color32, rot: f32) {
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    if rot == 0.0 {
        p.image(tex, rect, uv, tint);
    } else {
        let mut m = Mesh::with_texture(tex);
        m.add_rect_with_uv(rect, uv, tint);
        m.rotate(egui::emath::Rot2::from_angle(rot), rect.center());
        p.add(PShape::mesh(m));
    }
}

/// The app icon inside `rect` (a square), in the style of the config.
#[allow(clippy::too_many_arguments)]
pub fn app_icon(p: &Painter, rect: Rect, app: &App, icons: &mut Icons, cfg: &Config, pal: &Palette, scale: f32, rot: f32, alpha: f32) {
    let rect = Rect::from_center_size(rect.center(), rect.size() * scale);
    let radius = plate_radius(cfg, rect.width());
    let fade = |c: Color32| c.gamma_multiply(alpha);
    match icons.get(app) {
        Some(ic) if ic.mono => {
            p.rect_filled(rect, cr(radius), fade(pal.plate));
            if cfg.preset == Preset::Nothing {
                p.rect_stroke(rect, cr(radius), Stroke::new(1.0, fade(pal.glass_line)), StrokeKind::Inside);
            }
            textured(p, ic.tex.id(), rect.shrink(rect.width() * 0.13), fade(pal.glyph), rot);
        }
        Some(ic) => textured(p, ic.tex.id(), rect, Color32::WHITE.gamma_multiply(alpha), rot),
        None => {
            p.rect_filled(rect, cr(radius), fade(pal.card));
        }
    }
}

pub fn text_width(p: &Painter, text: &str, font: &egui::FontId) -> f32 {
    p.layout_no_wrap(text.to_string(), font.clone(), Color32::WHITE).size().x
}

pub fn fit(p: &Painter, text: &str, font: &egui::FontId, max_w: f32) -> String {
    if text_width(p, text, font) <= max_w {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut n = chars.len();
    while n > 1 {
        n -= 1;
        let s = chars[..n].iter().collect::<String>().trim_end().to_string() + "…";
        if text_width(p, &s, font) <= max_w {
            return s;
        }
    }
    "…".into()
}

pub fn label(p: &Painter, center_top: Pos2, text: &str, cfg: &Config, color: Color32, max_w: f32, shadow: bool, alpha: f32) {
    let font = fonts::label(cfg, cfg.label_size * cfg.font_scale, false);
    let s = fit(p, text, &font, max_w);
    if shadow {
        p.text(center_top + vec2(0.0, 0.8), Align2::CENTER_TOP, &s, font.clone(), Color32::from_black_alpha((90.0 * alpha) as u8));
    }
    p.text(center_top, Align2::CENTER_TOP, s, font, color.gamma_multiply(alpha));
}

/// Notification badge at the top-right corner of an icon.
pub fn badge(p: &Painter, icon: Rect, count: u32, cfg: &Config, pal: &Palette) {
    match cfg.badges {
        Badges::Off => {}
        Badges::Dot => {
            let c = icon.right_top() + vec2(-icon.width() * 0.06, icon.height() * 0.08);
            let r = if cfg.preset == Preset::Nothing { 4.5 } else { 5.5 };
            p.circle_filled(c, r + 1.8, pal.sheet.gamma_multiply(0.0));
            p.circle_filled(c, r, pal.badge);
        }
        Badges::Count => {
            let text = if count > 99 { "99+".to_string() } else { count.to_string() };
            let font = fonts::body(cfg, 12.0, true);
            let w = (text_width(p, &text, &font) + 12.0).max(20.0);
            let r = Rect::from_center_size(icon.right_top() + vec2(-2.0, 4.0), vec2(w, 20.0));
            p.rect_filled(r, cr(10.0), pal.badge);
            p.text(r.center(), Align2::CENTER_CENTER, text, font, pal.on_badge);
        }
    }
}

/// Folder preview: the first apps as little icons on a plate.
#[allow(clippy::too_many_arguments)]
pub fn folder_icon(p: &Painter, rect: Rect, apps: &[&App], icons: &mut Icons, cfg: &Config, pal: &Palette, scale: f32, rot: f32) {
    let rect = Rect::from_center_size(rect.center(), rect.size() * scale);
    let radius = plate_radius(cfg, rect.width());
    let _ = rot;
    p.rect_filled(rect, cr(radius), pal.folder_bg);
    if cfg.preset == Preset::Nothing {
        p.rect_stroke(rect, cr(radius), Stroke::new(1.0, pal.glass_line), StrokeKind::Inside);
    }
    let n = if cfg.preset == Preset::Ios { 3 } else { 2 };
    let pad = rect.width() * if n == 3 { 0.1 } else { 0.14 };
    let gap = rect.width() * if n == 3 { 0.05 } else { 0.08 };
    let cell = (rect.width() - 2.0 * pad - gap * (n as f32 - 1.0)) / n as f32;
    for (i, a) in apps.iter().take(n * n).enumerate() {
        let (c, r) = (i % n, i / n);
        let tl = rect.min + vec2(pad + c as f32 * (cell + gap), pad + r as f32 * (cell + gap));
        let mini = Rect::from_min_size(tl, vec2(cell, cell));
        let mut mini_cfg = cfg.clone();
        if cfg.shape == Shape::Rounded {
            mini_cfg.icon_radius = cfg.icon_radius;
        }
        app_icon(p, mini, a, icons, &mini_cfg, pal, 1.0, 0.0, 1.0);
    }
}

/// A vector icon from the Phosphor font.
pub fn glyph_at(p: &Painter, pos: Pos2, align: Align2, g: &str, size: f32, color: Color32) {
    p.text(pos, align, g, fonts::icon(size), color);
}

pub fn glyph_fill_at(p: &Painter, pos: Pos2, align: Align2, g: &str, size: f32, color: Color32) {
    p.text(pos, align, g, fonts::icon_fill(size), color);
}

/// Translucent panel standing in for frosted glass.
pub fn glass(p: &Painter, rect: Rect, radius: f32, pal: &Palette) {
    p.rect_filled(rect, cr(radius), pal.glass);
    p.rect_stroke(rect, cr(radius), Stroke::new(1.0, pal.glass_line), StrokeKind::Inside);
}

pub fn search_pill(p: &Painter, rect: Rect, text: &str, cfg: &Config, pal: &Palette, trailing: bool) {
    let r = if cfg.preset == Preset::Nothing { 14.0 } else { rect.height() / 2.0 };
    p.rect_filled(rect, cr(r), pal.search_bg);
    if cfg.preset == Preset::Nothing {
        p.rect_stroke(rect, cr(r), Stroke::new(1.0, pal.glass_line), StrokeKind::Inside);
    }
    let h = rect.height();
    glyph_at(p, pos2(rect.left() + 20.0, rect.center().y), Align2::LEFT_CENTER, egui_phosphor_glyph::MAGNIFYING_GLASS, (h * 0.42).max(17.0), pal.dim);
    p.text(pos2(rect.left() + 20.0 + h * 0.42 + 14.0, rect.center().y), Align2::LEFT_CENTER, text, fonts::body(cfg, (h * 0.3).clamp(15.0, 17.0) * cfg.font_scale, false), pal.dim);
    if trailing {
        glyph_at(p, pos2(rect.right() - 20.0, rect.center().y), Align2::RIGHT_CENTER, egui_phosphor_glyph::MICROPHONE, (h * 0.42).max(18.0), pal.dim);
        glyph_at(p, pos2(rect.right() - 20.0 - h * 0.42 - 18.0, rect.center().y), Align2::RIGHT_CENTER, egui_phosphor_glyph::CAMERA, (h * 0.42).max(18.0), pal.dim);
    }
}

use crate::phosphor as egui_phosphor_glyph;
