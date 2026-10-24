//! Small drawing helpers: dot-matrix digits, the icon + label cell, the search pill.

use crate::android::App;
use crate::config::Config;
use crate::icons::Icons;
use crate::theme::Palette;
use eframe::egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, Painter, Rect, Shape, Stroke, StrokeKind};

/// 5 columns x 7 rows per glyph. '#' is a lit dot.
const GLYPHS: [(char, [&str; 7]); 12] = [
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
    (':', [".....", "..#..", "..#..", ".....", "..#..", "..#..", "....."]),
    (' ', [".....", ".....", ".....", ".....", ".....", ".....", "....."]),
];

/// Draws `text` (digits and colon) as round dots. `dot` is the dot pitch in points. Returns the width used.
pub fn dot_text(p: &Painter, origin: eframe::egui::Pos2, text: &str, dot: f32, color: Color32, accent: Color32) -> f32 {
    let mut x = origin.x;
    for ch in text.chars() {
        let g = GLYPHS.iter().find(|(c, _)| *c == ch).map(|(_, g)| g).unwrap_or(&GLYPHS[11].1);
        let col = if ch == ':' { accent } else { color };
        let w = if ch == ':' { 3.0 } else { 5.0 };
        for (r, row) in g.iter().enumerate() {
            for (c, b) in row.bytes().enumerate() {
                let cx = if ch == ':' { c as f32 - 1.0 } else { c as f32 };
                if ch == ':' && !(1..4).contains(&c) {
                    continue;
                }
                let center = pos2(x + (cx + 0.5) * dot, origin.y + (r as f32 + 0.5) * dot);
                if b == b'#' {
                    p.circle_filled(center, dot * 0.36, col);
                } else if ch != ':' {
                    p.circle_filled(center, dot * 0.1, color.gamma_multiply(0.18));
                }
            }
        }
        x += (w + 1.6) * dot;
    }
    x - origin.x
}

pub fn dot_text_width(text: &str, dot: f32) -> f32 {
    text.chars().map(|c| ((if c == ':' { 3.0 } else { 5.0 }) + 1.6) * dot).sum::<f32>() - 1.6 * dot
}

/// The app icon inside `rect`, in the style of the config.
pub fn icon(p: &Painter, rect: Rect, a: &App, icons: &mut Icons, cfg: &Config, pal: &Palette, scale: f32) {
    let rect = Rect::from_center_size(rect.center(), rect.size() * scale);
    let radius = (cfg.icon_radius * rect.width() / 2.0).round().clamp(0.0, 255.0) as u8;
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    match icons.get(a) {
        Some(ic) if ic.mono => {
            p.rect_filled(rect, CornerRadius::same(radius), pal.plate);
            p.image(ic.tex.id(), rect.shrink(rect.width() * 0.11), uv, pal.glyph);
        }
        Some(ic) => {
            p.image(ic.tex.id(), rect, uv, Color32::WHITE);
        }
        None => {
            p.rect_filled(rect, CornerRadius::same(radius), pal.card);
        }
    }
}

pub fn label(p: &Painter, center_top: eframe::egui::Pos2, text: &str, cfg: &Config, pal: &Palette, max_w: f32, shadow: bool) {
    let font = FontId::proportional(12.0 * cfg.font_scale);
    let width = |t: &str| p.layout_no_wrap(t.to_string(), font.clone(), pal.text).size().x;
    let mut s = text.to_string();
    if width(&s) > max_w {
        let chars: Vec<char> = text.chars().collect();
        let mut n = chars.len();
        while n > 1 {
            n -= 1;
            s = chars[..n].iter().collect::<String>().trim_end().to_string() + "…";
            if width(&s) <= max_w {
                break;
            }
        }
    }
    if shadow {
        p.text(center_top + vec2(0.0, 1.0), Align2::CENTER_TOP, &s, font.clone(), Color32::from_black_alpha(90));
    }
    p.text(center_top, Align2::CENTER_TOP, s, font, pal.text);
}

pub fn search_pill(p: &Painter, rect: Rect, text: &str, pal: &Palette, cfg: &Config) {
    let r = if cfg.preset == crate::config::Preset::Nothing { 10 } else { (rect.height() / 2.0) as u8 };
    p.rect_filled(rect, CornerRadius::same(r), pal.glass);
    p.rect_stroke(rect, CornerRadius::same(r), Stroke::new(1.0, pal.glass_line), StrokeKind::Inside);
    magnifier(p, pos2(rect.left() + 26.0, rect.center().y), 7.0, pal.dim);
    p.text(pos2(rect.left() + 48.0, rect.center().y), Align2::LEFT_CENTER, text, FontId::proportional(15.0 * cfg.font_scale), pal.dim);
}

pub fn magnifier(p: &Painter, c: eframe::egui::Pos2, r: f32, color: Color32) {
    let st = Stroke::new(1.8, color);
    p.circle_stroke(c - vec2(r * 0.25, r * 0.25), r, st);
    p.add(Shape::line_segment([c + vec2(r * 0.75, r * 0.75), c + vec2(r * 1.5, r * 1.5)], st));
}
