//! Colors and sizes derived from the config. One place decides how every preset looks.

use crate::config::{Config, Mode, Preset};
use eframe::egui::{Color32, Context, CornerRadius, Stroke, Visuals};

#[derive(Clone, Copy)]
pub struct Palette {
    pub text: Color32,
    pub dim: Color32,
    pub accent: Color32,
    pub on_accent: Color32,
    /// background behind glyph-only (monochrome) icons
    pub plate: Color32,
    pub glyph: Color32,
    pub glass: Color32,
    pub glass_line: Color32,
    pub sheet: Color32,
    pub card: Color32,
    pub scrim: Color32,
    pub light: bool,
}

pub fn palette(c: &Config) -> Palette {
    let accent = Color32::from_rgb(c.accent[0], c.accent[1], c.accent[2]);
    let lum = (c.accent[0] as u32 * 299 + c.accent[1] as u32 * 587 + c.accent[2] as u32 * 114) / 1000;
    let on_accent = if lum > 150 { Color32::BLACK } else { Color32::WHITE };
    let dim_alpha = (c.wallpaper_dim.clamp(0.0, 0.9) * 255.0) as u8;
    match c.mode {
        Mode::Dark => {
            let (sheet, plate) = match c.preset {
                Preset::Nothing => (Color32::BLACK, Color32::from_rgb(28, 28, 28)),
                Preset::Ios => (Color32::from_rgba_unmultiplied(28, 28, 32, 235), Color32::from_rgb(44, 44, 48)),
                _ => (Color32::from_rgb(24, 26, 30), Color32::from_rgb(44, 48, 56)),
            };
            Palette {
                text: Color32::WHITE,
                dim: Color32::from_white_alpha(150),
                accent,
                on_accent,
                plate,
                glyph: Color32::WHITE,
                glass: Color32::from_white_alpha(if c.preset == Preset::Ios { 40 } else { 34 }),
                glass_line: Color32::from_white_alpha(28),
                sheet,
                card: Color32::from_white_alpha(18),
                scrim: Color32::from_black_alpha(dim_alpha),
                light: false,
            }
        }
        Mode::Light => Palette {
            text: Color32::from_rgb(20, 20, 22),
            dim: Color32::from_black_alpha(140),
            accent,
            on_accent,
            plate: Color32::from_rgb(232, 232, 236),
            glyph: Color32::from_rgb(20, 20, 22),
            glass: Color32::from_white_alpha(150),
            glass_line: Color32::from_black_alpha(18),
            sheet: if c.preset == Preset::Nothing { Color32::from_rgb(240, 240, 240) } else { Color32::from_rgb(246, 247, 250) },
            card: Color32::from_black_alpha(10),
            scrim: Color32::from_white_alpha(dim_alpha),
            light: true,
        },
    }
}

pub fn apply(ctx: &Context, c: &Config, p: &Palette) {
    let mut v = if p.light { Visuals::light() } else { Visuals::dark() };
    v.panel_fill = Color32::TRANSPARENT;
    v.window_fill = p.sheet;
    v.override_text_color = Some(p.text);
    v.selection.bg_fill = p.accent;
    v.selection.stroke = Stroke::new(1.0, p.on_accent);
    v.hyperlink_color = p.accent;
    v.extreme_bg_color = p.card;
    let r = CornerRadius::same(if c.preset == Preset::Nothing { 6 } else { 14 });
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
    v.window_stroke = Stroke::new(1.0, p.glass_line);
    v.window_corner_radius = CornerRadius::same(22);
    ctx.set_visuals(v);
}
