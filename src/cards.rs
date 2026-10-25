//! The widgets Lumen draws itself: clocks, date, battery, search.

use crate::clock::ClockInfo;
use crate::config::{Config, Preset, WidgetKind};
use crate::fonts;
use crate::phosphor as ph;
use crate::platform::Battery;
use crate::theme::Palette;
use crate::widgets::{self, cr, dot_text, dot_text_width};
use egui::{pos2, vec2, Align2, Color32, Painter, Rect, Stroke, StrokeKind};

pub struct Live<'a> {
    pub clock: &'a ClockInfo,
    pub battery: Battery,
}

pub fn paint(p: &Painter, rect: Rect, kind: WidgetKind, cfg: &Config, pal: &Palette, live: &Live) {
    let r = rect.shrink2(vec2(6.0, 4.0));
    let fs = cfg.font_scale;
    let c = live.clock;
    match kind {
        WidgetKind::Glance => {
            // Pixel "At a glance": the date, big, left aligned, with one quiet line under it
            let x = r.left() + 14.0;
            p.text(pos2(x, r.top() + 8.0), Align2::LEFT_TOP, c.date(), fonts::body(cfg, 38.0 * fs, false), pal.home_text);
            let line = pos2(x + 1.0, r.top() + 58.0);
            let b = live.battery;
            let icon = if b.charging { ph::BATTERY_CHARGING } else if b.level < 20 { ph::BATTERY_LOW } else { ph::BATTERY_FULL };
            widgets::glyph_at(p, line, Align2::LEFT_CENTER, icon, 18.0, pal.home_text);
            p.text(line + vec2(24.0, 0.0), Align2::LEFT_CENTER, format!("{}%{}", b.level, if b.charging { " · Charging" } else { "" }), fonts::body(cfg, 15.0 * fs, false), pal.home_text);
        }
        WidgetKind::ClockStack => {
            let big = fonts::body(cfg, 92.0 * fs, true);
            p.text(pos2(r.left() + 10.0, r.top() - 6.0), Align2::LEFT_TOP, format!("{:02}", c.hour_shown()), big.clone(), pal.home_text);
            p.text(pos2(r.left() + 10.0, r.top() + 78.0 * fs), Align2::LEFT_TOP, format!("{:02}", c.min), big, pal.accent.gamma_multiply(1.0));
        }
        WidgetKind::ClockDigital => {
            p.text(pos2(r.left() + 12.0, r.top() + 4.0), Align2::LEFT_TOP, c.time(), fonts::body(cfg, 76.0 * fs, false), pal.home_text);
            p.text(pos2(r.left() + 16.0, r.top() + 94.0 * fs), Align2::LEFT_TOP, c.date(), fonts::body(cfg, 17.0 * fs, false), pal.home_text.gamma_multiply(0.8));
        }
        WidgetKind::ClockAnalog => analog(p, r, cfg, pal, c),
        WidgetKind::DotClock => {
            // Nothing: a black card with the time in dots, the date in mono caps
            let card = rect.shrink2(vec2(4.0, 2.0));
            p.rect_filled(card, cr(cfg.widget_radius), Color32::from_rgba_unmultiplied(0, 0, 0, if pal.light { 235 } else { 215 }));
            p.rect_stroke(card, cr(cfg.widget_radius), Stroke::new(1.0, Color32::from_white_alpha(22)), StrokeKind::Inside);
            let time = c.time();
            let dot = ((card.width() - 56.0) / 30.0).min((card.height() - 90.0) / 7.0).min(13.0);
            let w = dot_text_width(&time, dot);
            let block = 7.0 * dot + 30.0;
            let top = card.center().y - block / 2.0;
            dot_text(p, pos2(card.center().x - w / 2.0, top), &time, dot, Color32::WHITE, pal.accent, true);
            p.text(pos2(card.center().x, top + 7.0 * dot + 22.0), Align2::CENTER_CENTER, c.short(), fonts::mono(13.0 * fs, true), Color32::from_white_alpha(170));
            p.circle_filled(pos2(card.right() - 26.0, card.top() + 26.0), 4.0, pal.accent);
        }
        WidgetKind::DotDate => {
            let card = rect.shrink2(vec2(4.0, 2.0));
            p.rect_filled(card, cr(cfg.widget_radius), Color32::from_rgba_unmultiplied(0, 0, 0, 215));
            p.rect_stroke(card, cr(cfg.widget_radius), Stroke::new(1.0, Color32::from_white_alpha(22)), StrokeKind::Inside);
            let day = format!("{:02}", c.day);
            let dot = ((card.width() - 40.0) / 11.5).min(10.0);
            let w = dot_text_width(&day, dot);
            dot_text(p, pos2(card.center().x - w / 2.0, card.top() + 28.0), &day, dot, Color32::WHITE, pal.accent, true);
            p.text(pos2(card.center().x, card.bottom() - 26.0), Align2::CENTER_CENTER, c.short().split(' ').next().unwrap_or("").to_string(), fonts::mono(13.0 * fs, true), pal.accent);
        }
        WidgetKind::DotBattery => {
            let card = rect.shrink2(vec2(4.0, 2.0));
            p.rect_filled(card, cr(cfg.widget_radius), Color32::from_rgba_unmultiplied(0, 0, 0, 215));
            p.rect_stroke(card, cr(cfg.widget_radius), Stroke::new(1.0, Color32::from_white_alpha(22)), StrokeKind::Inside);
            let t = format!("{}%", live.battery.level);
            let dot = ((card.width() - 36.0) / (dot_text_width(&t, 1.0))).min(8.0);
            let w = dot_text_width(&t, dot);
            dot_text(p, pos2(card.center().x - w / 2.0, card.top() + 24.0), &t, dot, Color32::WHITE, pal.accent, false);
            let lit = (live.battery.level as f32 / 10.0).ceil() as usize;
            for i in 0..10 {
                let x = card.left() + 24.0 + i as f32 * ((card.width() - 48.0) / 9.0);
                let col = if i < lit { if live.battery.level < 20 { pal.accent } else { Color32::WHITE } } else { Color32::from_white_alpha(40) };
                p.circle_filled(pos2(x, card.bottom() - 28.0), 3.2, col);
            }
        }
        WidgetKind::Calendar => {
            let card = r;
            let ios = cfg.preset == Preset::Ios;
            let bg = if pal.light { Color32::from_rgba_unmultiplied(255, 255, 255, 235) } else { Color32::from_rgba_unmultiplied(30, 30, 32, 225) };
            p.rect_filled(card, cr(cfg.widget_radius), bg);
            let red = Color32::from_rgb(255, 69, 58);
            p.text(pos2(card.left() + 16.0, card.top() + 14.0), Align2::LEFT_TOP, c.weekday_full().to_uppercase(), fonts::body(cfg, 13.0 * fs, true), if ios { red } else { pal.accent });
            p.text(pos2(card.left() + 14.0, card.top() + 26.0), Align2::LEFT_TOP, c.day.to_string(), fonts::body(cfg, 62.0 * fs, false), if pal.light { Color32::BLACK } else { Color32::WHITE });
            p.text(pos2(card.left() + 16.0, card.bottom() - 22.0), Align2::LEFT_CENTER, "No events today", fonts::body(cfg, 12.5 * fs, false), if pal.light { Color32::from_gray(110) } else { Color32::from_gray(160) });
        }
        WidgetKind::Battery => {
            let card = r;
            let bg = if pal.light { Color32::from_rgba_unmultiplied(255, 255, 255, 235) } else { Color32::from_rgba_unmultiplied(30, 30, 32, 225) };
            p.rect_filled(card, cr(cfg.widget_radius), bg);
            let center = card.center() - vec2(0.0, 6.0);
            let rad = card.height().min(card.width()) * 0.3;
            p.circle_stroke(center, rad, Stroke::new(8.0, Color32::from_gray(if pal.light { 225 } else { 62 })));
            let n = 48;
            let lit = (live.battery.level as usize * n) / 100;
            let col = if live.battery.level < 20 { Color32::from_rgb(255, 69, 58) } else { Color32::from_rgb(48, 209, 88) };
            let pts: Vec<_> = (0..=lit).map(|i| {
                let a = -std::f32::consts::FRAC_PI_2 + i as f32 / n as f32 * std::f32::consts::TAU;
                center + vec2(a.cos(), a.sin()) * rad
            }).collect();
            if pts.len() > 1 {
                p.add(egui::Shape::line(pts, Stroke::new(8.0, col)));
            }
            p.text(center, Align2::CENTER_CENTER, format!("{}%", live.battery.level), fonts::body(cfg, 17.0 * fs, true), if pal.light { Color32::BLACK } else { Color32::WHITE });
            p.text(pos2(card.center().x, card.bottom() - 16.0), Align2::CENTER_CENTER, if live.battery.charging { "Charging" } else { "Battery" }, fonts::body(cfg, 12.0 * fs, false), pal.dim);
        }
        WidgetKind::SearchBar => widgets::search_pill(p, rect.shrink2(vec2(6.0, 4.0)), "Search", cfg, pal, cfg.preset == Preset::Pixel),
    }
}

fn analog(p: &Painter, r: Rect, cfg: &Config, pal: &Palette, c: &ClockInfo) {
    let ios = cfg.preset == Preset::Ios;
    let bg = if ios { Color32::from_rgba_unmultiplied(20, 20, 22, 235) } else { pal.card };
    p.rect_filled(r, cr(cfg.widget_radius), bg);
    let ctr = r.center();
    let rad = r.width().min(r.height()) * 0.42;
    let fg = if ios { Color32::WHITE } else { pal.text };
    for i in 0..12 {
        let a = i as f32 / 12.0 * std::f32::consts::TAU;
        let d = vec2(a.sin(), -a.cos());
        let (w, len) = if i % 3 == 0 { (2.4, 8.0) } else { (1.2, 5.0) };
        p.line_segment([ctr + d * (rad - len), ctr + d * rad], Stroke::new(w, fg.gamma_multiply(if i % 3 == 0 { 0.95 } else { 0.5 })));
    }
    let hand = |angle: f32, len: f32, w: f32, col: Color32| {
        let d = vec2(angle.sin(), -angle.cos());
        p.line_segment([ctr - d * len * 0.12, ctr + d * len], Stroke::new(w, col));
    };
    let h = (c.hour % 12) as f32 + c.min as f32 / 60.0;
    hand(h / 12.0 * std::f32::consts::TAU, rad * 0.5, 3.6, fg);
    hand((c.min as f32 + c.sec as f32 / 60.0) / 60.0 * std::f32::consts::TAU, rad * 0.78, 2.6, fg);
    let orange = Color32::from_rgb(255, 149, 0);
    hand(c.sec as f32 / 60.0 * std::f32::consts::TAU, rad * 0.85, 1.2, orange);
    p.circle_filled(ctr, 3.4, orange);
}
