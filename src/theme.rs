//! Colors derived from the config, the phone's Material You palette and light/dark mode.

use crate::config::{Config, Preset};
use crate::platform::{Dynamic, SHADE_100, SHADE_200, SHADE_700, SHADE_800, SHADE_900};
use egui::{Color32, Context, CornerRadius, Stroke, Visuals};

#[derive(Clone, Copy)]
pub struct Palette {
    pub light: bool,
    /// text on the wallpaper (home screen labels)
    pub home_text: Color32,
    pub text: Color32,
    pub dim: Color32,
    pub accent: Color32,
    pub on_accent: Color32,
    /// tinted container behind accent elements (chips, selected rows)
    pub accent_soft: Color32,
    /// background behind glyph-only (single color) icons
    pub plate: Color32,
    pub glyph: Color32,
    pub glass: Color32,
    pub glass_line: Color32,
    /// full-screen surfaces: drawer, settings
    pub sheet: Color32,
    pub card: Color32,
    pub search_bg: Color32,
    pub menu_bg: Color32,
    pub folder_bg: Color32,
    pub scrim: Color32,
    pub badge: Color32,
    pub on_badge: Color32,
}

pub fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// A tonal ramp (shades 0..1000 like Material You) from one color. Used when the phone has no dynamic colors.
pub fn ramp(base: [u8; 3], tint: f32) -> [[u8; 3]; 13] {
    let mix = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let l = [1.0, 0.99, 0.96, 0.92, 0.82, 0.72, 0.62, 0.52, 0.42, 0.32, 0.22, 0.12, 0.0f32];
    let mut o = [[0u8; 3]; 13];
    for (i, t) in l.iter().enumerate() {
        for c in 0..3 {
            let b = base[c] as f32;
            let gray = 128.0;
            let col = mix(gray, b, tint);
            let v = if *t >= 0.5 { mix(col, 255.0, (t - 0.5) * 2.0) } else { col * t * 2.0 };
            o[i][c] = v.clamp(0.0, 255.0) as u8;
        }
    }
    o
}

pub fn dynamic_for(cfg: &Config, phone: Option<Dynamic>) -> Dynamic {
    if cfg.dynamic {
        if let Some(d) = phone {
            return d;
        }
    }
    Dynamic { accent1: ramp(cfg.accent, 1.0), neutral1: ramp(cfg.accent, 0.08), neutral2: ramp(cfg.accent, 0.2) }
}

fn a(c: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}

pub fn palette(cfg: &Config, dark: bool, phone: Option<Dynamic>) -> Palette {
    let d = dynamic_for(cfg, phone);
    let dim_alpha = (cfg.wallpaper_dim.clamp(0.0, 0.9) * 255.0) as u8;
    let scrim = if dark { Color32::from_black_alpha(dim_alpha) } else { Color32::from_white_alpha(dim_alpha) };
    let black = Color32::BLACK;
    let white = Color32::WHITE;

    // accent: from the ramp when dynamic, else the picked color itself
    let accent = if cfg.dynamic && phone.is_some() { rgb(d.accent1[if dark { SHADE_200 } else { 8 }]) } else { rgb(cfg.accent) };
    let lum = (accent.r() as u32 * 299 + accent.g() as u32 * 587 + accent.b() as u32 * 114) / 1000;
    let on_accent = if lum > 150 { black } else { white };

    let mut p = if dark {
        Palette {
            light: false,
            home_text: white,
            text: rgb(d.neutral1[SHADE_100]),
            dim: a(rgb(d.neutral1[SHADE_200]), 190),
            accent,
            on_accent,
            accent_soft: rgb(d.accent1[SHADE_800]),
            plate: rgb(d.accent1[SHADE_800]),
            glyph: rgb(d.accent1[SHADE_100]),
            glass: Color32::from_white_alpha(38),
            glass_line: Color32::from_white_alpha(30),
            sheet: rgb(d.neutral1[SHADE_900]),
            card: rgb(d.neutral1[SHADE_800]),
            search_bg: rgb(d.neutral1[SHADE_800]),
            menu_bg: rgb(d.neutral1[SHADE_800]),
            folder_bg: a(rgb(d.neutral1[SHADE_800]), 235),
            scrim,
            badge: accent,
            on_badge: on_accent,
        }
    } else {
        Palette {
            light: true,
            home_text: rgb(d.neutral1[SHADE_900]),
            text: rgb(d.neutral1[SHADE_900]),
            dim: a(rgb(d.neutral1[SHADE_800]), 160),
            accent,
            on_accent,
            accent_soft: rgb(d.accent1[SHADE_100]),
            plate: rgb(d.accent1[SHADE_100]),
            glyph: rgb(d.accent1[SHADE_700]),
            glass: Color32::from_white_alpha(150),
            glass_line: Color32::from_black_alpha(18),
            sheet: rgb(d.neutral1[2]),
            card: rgb(d.neutral2[SHADE_100]),
            search_bg: white,
            menu_bg: white,
            folder_bg: a(rgb(d.neutral1[2]), 235),
            scrim,
            badge: accent,
            on_badge: on_accent,
        }
    };

    match cfg.preset {
        Preset::Nothing => {
            // black, white and one red. No tints.
            let red = rgb(cfg.accent);
            if dark {
                p.sheet = black;
                p.text = white;
                p.dim = Color32::from_gray(140);
                p.card = Color32::from_gray(22);
                p.search_bg = Color32::from_gray(20);
                p.menu_bg = Color32::from_gray(18);
                p.plate = Color32::from_gray(20);
                p.glyph = white;
                p.accent_soft = Color32::from_gray(40);
                p.folder_bg = a(Color32::from_gray(14), 240);
                p.glass = a(black, 120);
                p.glass_line = Color32::from_white_alpha(40);
            } else {
                p.sheet = Color32::from_gray(242);
                p.text = black;
                p.dim = Color32::from_gray(110);
                p.card = white;
                p.search_bg = white;
                p.menu_bg = white;
                p.plate = white;
                p.glyph = black;
                p.accent_soft = Color32::from_gray(228);
                p.home_text = black;
            }
            p.accent = red;
            p.on_accent = white;
            p.badge = red;
            p.on_badge = white;
        }
        Preset::Ios => {
            p.home_text = white; // iOS labels stay white on the wallpaper in both modes
            p.badge = Color32::from_rgb(255, 59, 48);
            p.on_badge = white;
            if dark {
                p.sheet = Color32::from_rgb(28, 28, 30);
                p.card = Color32::from_rgb(44, 44, 46);
                p.search_bg = Color32::from_rgba_unmultiplied(118, 118, 128, 90);
                p.menu_bg = Color32::from_rgba_unmultiplied(44, 44, 46, 245);
                p.folder_bg = Color32::from_rgba_unmultiplied(90, 90, 100, 150);
                p.glass = Color32::from_rgba_unmultiplied(235, 235, 245, 46);
            } else {
                p.sheet = Color32::from_rgb(242, 242, 247);
                p.card = white;
                p.search_bg = Color32::from_rgba_unmultiplied(118, 118, 128, 40);
                p.menu_bg = Color32::from_rgba_unmultiplied(250, 250, 252, 245);
                p.folder_bg = Color32::from_rgba_unmultiplied(255, 255, 255, 140);
                p.glass = Color32::from_rgba_unmultiplied(255, 255, 255, 110);
                p.text = black;
            }
            if !cfg.dynamic {
                p.accent = rgb(cfg.accent);
            }
        }
        _ => {}
    }
    p
}

pub fn apply(ctx: &Context, cfg: &Config, p: &Palette) {
    let mut v = if p.light { Visuals::light() } else { Visuals::dark() };
    v.panel_fill = Color32::TRANSPARENT;
    v.window_fill = p.sheet;
    v.override_text_color = Some(p.text);
    v.selection.bg_fill = p.accent;
    v.selection.stroke = Stroke::new(1.0, p.on_accent);
    v.hyperlink_color = p.accent;
    v.extreme_bg_color = p.card;
    let r = CornerRadius::same(if cfg.preset == Preset::Nothing { 8 } else { 14 });
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = r;
    }
    v.widgets.inactive.bg_fill = p.card;
    v.widgets.inactive.weak_bg_fill = p.card;
    v.widgets.hovered.bg_fill = p.card;
    v.widgets.hovered.weak_bg_fill = p.card;
    v.widgets.active.bg_fill = p.accent;
    v.widgets.active.weak_bg_fill = p.accent;
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.hovered.bg_stroke = Stroke::NONE;
    v.widgets.noninteractive.bg_stroke = Stroke::NONE;
    v.window_stroke = Stroke::new(1.0, p.glass_line);
    v.window_corner_radius = CornerRadius::same(24);
    ctx.set_visuals(v);
}
