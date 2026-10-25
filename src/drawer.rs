//! The app drawer and the search overlay.

use crate::app::*;
use crate::config::{DrawerStyle, Item, Preset};
use crate::fonts;
use crate::phosphor as ph;
use crate::platform::{self, App};
use crate::widgets::{self, cr};
use egui::{pos2, vec2, Align2, Area, Color32, Context, CornerRadius, Frame, Id, Key, Order, Rect, Sense, Stroke, StrokeKind, UiBuilder};

pub enum Row {
    App(String),
    Calc(String, String),
    Web(String),
}

/// Apps whose name starts with, then contains, the query; plus a calculator line and a web search.
pub fn results(l: &Lumen, q: &str) -> Vec<Row> {
    let q = q.trim().to_lowercase();
    let mut starts: Vec<&App> = vec![];
    let mut contains: Vec<&App> = vec![];
    for a in l.apps.iter() {
        if l.cfg.hidden.contains(&a.pkg) && q.is_empty() {
            continue;
        }
        let name = l.cfg.label(a).to_lowercase();
        if name.starts_with(&q) || name.split_whitespace().any(|w| w.starts_with(&q)) {
            starts.push(a);
        } else if name.contains(&q) {
            contains.push(a);
        }
    }
    starts.sort_by_key(|a| std::cmp::Reverse(l.cfg.launches.get(&a.pkg).copied().unwrap_or(0)));
    let mut rows: Vec<Row> = vec![];
    if let Some(v) = calc(&q) {
        rows.push(Row::Calc(q.clone(), fmt_num(v)));
    }
    rows.extend(starts.into_iter().chain(contains).map(|a| Row::App(a.pkg.clone())));
    if !q.is_empty() {
        rows.push(Row::Web(q));
    }
    rows
}

fn fmt_num(v: f64) -> String {
    if v.fract().abs() < 1e-9 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.8}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// A small calculator: + - * / ^ and parentheses. None unless the text really is a sum.
pub fn calc(s: &str) -> Option<f64> {
    if !s.chars().any(|c| c.is_ascii_digit()) || !s.chars().any(|c| "+-*/x×÷^".contains(c)) {
        return None;
    }
    let cleaned: Vec<char> = s.chars().filter(|c| !c.is_whitespace()).map(|c| match c { 'x' | '×' => '*', '÷' => '/', ',' => '.', o => o }).collect();
    if cleaned.iter().any(|c| !(c.is_ascii_digit() || "+-*/^().".contains(*c))) {
        return None;
    }
    struct P<'a> {
        s: &'a [char],
        i: usize,
    }
    impl P<'_> {
        fn peek(&self) -> Option<char> {
            self.s.get(self.i).copied()
        }
        fn expr(&mut self) -> Option<f64> {
            let mut v = self.term()?;
            while let Some(c) = self.peek() {
                if c == '+' || c == '-' {
                    self.i += 1;
                    let r = self.term()?;
                    v = if c == '+' { v + r } else { v - r };
                } else {
                    break;
                }
            }
            Some(v)
        }
        fn term(&mut self) -> Option<f64> {
            let mut v = self.pow()?;
            while let Some(c) = self.peek() {
                if c == '*' || c == '/' {
                    self.i += 1;
                    let r = self.pow()?;
                    v = if c == '*' { v * r } else { v / r };
                } else {
                    break;
                }
            }
            Some(v)
        }
        fn pow(&mut self) -> Option<f64> {
            let b = self.atom()?;
            if self.peek() == Some('^') {
                self.i += 1;
                let e = self.pow()?;
                return Some(b.powf(e));
            }
            Some(b)
        }
        fn atom(&mut self) -> Option<f64> {
            match self.peek()? {
                '(' => {
                    self.i += 1;
                    let v = self.expr()?;
                    if self.peek()? != ')' {
                        return None;
                    }
                    self.i += 1;
                    Some(v)
                }
                '-' => {
                    self.i += 1;
                    Some(-self.atom()?)
                }
                _ => {
                    let st = self.i;
                    while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '.') {
                        self.i += 1;
                    }
                    self.s[st..self.i].iter().collect::<String>().parse().ok()
                }
            }
        }
    }
    let mut p = P { s: &cleaned, i: 0 };
    let v = p.expr()?;
    (p.i == cleaned.len() && v.is_finite()).then_some(v)
}

impl Lumen {
    // ───────────────────────────── drawer ─────────────────────────────

    pub fn drawer_ui(&mut self, ctx: &Context, screen: Rect) {
        let t = self.drawer;
        let ease = 1.0 - (1.0 - t).powi(3);
        let y = screen.top() + (1.0 - ease) * screen.height();
        let rect = Rect::from_min_size(pos2(screen.left(), y), screen.size());
        let k = ctx.pixels_per_point();
        let (il, it, ir, ib) = (self.insets.left / k, self.insets.top / k, self.insets.right / k, self.insets.bottom / k);
        let pal = self.pal;
        let cfg = self.cfg.clone();
        let nothing = cfg.preset == Preset::Nothing;
        let pixel = cfg.preset == Preset::Pixel;
        let round = if nothing || pixel { 0 } else { 28 };
        let sheet = pal.sheet.gamma_multiply(cfg.drawer_opacity);

        let mut launch: Option<String> = None;
        let mut web: Option<String> = None;
        let mut grab: Option<(String, Rect, egui::Pos2)> = None;
        let mut close = false;
        let time = ctx.input(|i| i.time);
        let lifting = matches!(self.touch, Touch::Lifted { .. } | Touch::Drag(_));
        let idle = matches!(self.touch, Touch::Idle);
        let mut click_ok = time > self.suppress_click;
        let scroll_to = self.drawer_scroll_to.take();

        Area::new(Id::new("drawer")).order(Order::Foreground).fixed_pos(rect.min).constrain(false).show(ctx, |ui| {
            ui.set_clip_rect(screen);
            ui.painter().rect_filled(rect, CornerRadius { nw: round, ne: round, sw: 0, se: 0 }, sheet);
            ui.allocate_rect(rect, Sense::click_and_drag());

            let left = rect.left() + il.max(12.0);
            let right = rect.right() - ir.max(12.0);
            let width = right - left;
            let bottom = rect.bottom() - ib.max(12.0);

            // search pill at the bottom (Pixel, Nothing) or at the top (others)
            let bar_h = if pixel { 56.0 } else { 50.0 };
            let at_bottom = match cfg.drawer_search { crate::config::SearchPos::Top => false, crate::config::SearchPos::Bottom => true, crate::config::SearchPos::Auto => pixel || nothing };
            let bar = if at_bottom { Rect::from_min_max(pos2(left + 4.0, bottom - bar_h), pos2(right - 4.0, bottom)) } else { Rect::from_min_size(pos2(left + 4.0, rect.top() + it.max(10.0) + 8.0), vec2(width - 8.0, bar_h)) };
            widgets::search_pill(&ui.painter().clone(), bar, "", &cfg, &pal, false);
            let field = Rect::from_min_max(pos2(bar.left() + 56.0, bar.top()), pos2(bar.right() - 56.0, bar.bottom()));
            ui.scope_builder(UiBuilder::new().max_rect(field).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
                ui.set_clip_rect(field);
                let te = egui::TextEdit::singleline(&mut self.query)
                    .hint_text(egui::RichText::new(if pixel { "Search apps, web and more" } else { "Search" }).color(pal.dim))
                    .desired_width(f32::INFINITY)
                    .margin(vec2(0.0, 0.0))
                    .min_size(vec2(field.width(), field.height()))
                    .vertical_align(egui::Align::Center)
                    .font(fonts::body(&cfg, 16.0 * cfg.font_scale, false))
                    .background_color(Color32::TRANSPARENT)
                    .frame(Frame::NONE);
                let r = ui.add(te);
                if self.focus_search && t > 0.9 {
                    r.request_focus();
                    self.focus_search = false;
                }
                if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    // Enter opens the best match
                }
            });
            if !self.query.is_empty() {
                let x = pos2(bar.right() - 28.0, bar.center().y);
                widgets::glyph_at(ui.painter(), x, Align2::CENTER_CENTER, ph::X, 18.0, pal.dim);
                if ui.interact(Rect::from_center_size(x, vec2(44.0, 44.0)), Id::new("clear-q"), Sense::click()).clicked() {
                    self.query.clear();
                }
            }

            // suggested apps (Pixel)
            let mut list_top = if at_bottom { rect.top() + it.max(10.0) + 8.0 } else { bar.bottom() + 12.0 };
            let list_bottom = if at_bottom { bar.top() - 10.0 } else { bottom };
            let searching = !self.query.trim().is_empty();
            let scrub = !searching && cfg.drawer_scrubber;
            if cfg.suggestions && !searching {
                let sug: Vec<String> = self.suggested(cfg.drawer_cols.min(5)).into_iter().map(|a| a.pkg.clone()).collect();
                if !sug.is_empty() {
                    let h = cfg.eff() + 26.0;
                    let box_r = Rect::from_min_size(pos2(left + 4.0, list_top), vec2(width - 8.0, h));
                    ui.painter().rect_filled(box_r, cr(28.0), pal.card);
                    let n = cfg.drawer_cols.min(5);
                    let cw = box_r.width() / n as f32;
                    for (i, pkg) in sug.iter().enumerate() {
                        let cell = Rect::from_min_size(box_r.min + vec2(i as f32 * cw, 0.0), vec2(cw, h));
                        let ir = Rect::from_center_size(cell.center(), vec2(cfg.eff(), cfg.eff()));
                        if let Some(&ai) = self.by_pkg.get(pkg) {
                            let app = self.apps[ai].clone();
                            let resp = ui.interact(cell, Id::new(("sug", pkg)), Sense::click());
                            widgets::app_icon(ui.painter(), ir, &app, self.icons.as_mut().unwrap(), &cfg, &pal, if resp.is_pointer_button_down_on() { 0.92 } else { 1.0 }, 0.0, 1.0);
                            if resp.clicked() && click_ok {
                                launch = Some(pkg.clone());
                            }
                        }
                    }
                    list_top += h + 12.0;
                }
            }

            let body = Rect::from_min_max(pos2(left, list_top), pos2(right, list_bottom));
            let rows_data = if searching { Some(results(self, &self.query.clone())) } else { None };
            let mut visible: Vec<usize> = if searching { vec![] } else { (0..self.apps.len()).filter(|&i| !cfg.hidden.contains(&self.apps[i].pkg)).collect() };
            if cfg.drawer_sort == crate::config::Sort::MostUsed {
                visible.sort_by_key(|&i| std::cmp::Reverse(cfg.launches.get(&self.apps[i].pkg).copied().unwrap_or(0)));
            }

            // content height first, so the scroller knows its limits
            let content_h = if let Some(rows) = &rows_data {
                rows.len() as f32 * 64.0 + 20.0
            } else {
                match cfg.drawer {
                    DrawerStyle::Grid => visible.len().div_ceil(cfg.drawer_cols.max(3)) as f32 * (cfg.eff() + 40.0 * cfg.font_scale + 10.0) + 24.0,
                    DrawerStyle::List => visible.len() as f32 * 62.0 + 24.0,
                }
            };
            let max_off = (content_h - body.height()).max(0.0);
            if let Some(o) = scroll_to {
                self.scroller.jump_to(o.min(max_off));
            }
            let dt = ui.ctx().input(|i| i.stable_dt).clamp(0.001, 0.05);
            self.scroller.update(ui.ctx(), body, max_off, !lifting && t > 0.98, dt);
            click_ok = click_ok && self.scroller.tap_ok();
            ui.scope_builder(UiBuilder::new().max_rect(body), |ui| {
                ui.set_clip_rect(body);
                {
                    let off = self.scroller.offset;
                    let viewport = Rect::from_min_size(pos2(0.0, off), body.size());
                    let origin = body.min - vec2(0.0, off);
                    let w = body.width() - if scrub { 24.0 } else { 0.0 };
                    if let Some(rows) = &rows_data {
                        // results as a list
                        let rh = 64.0;
                        ui.set_height(rows.len() as f32 * rh + 20.0);
                        for (i, row) in rows.iter().enumerate() {
                            let rr = Rect::from_min_size(origin + vec2(0.0, i as f32 * rh), vec2(w, rh));
                            if rr.bottom() < viewport.top() + origin.y - origin.y - 1.0 && false {
                                continue;
                            }
                            let id = Id::new(("res", i));
                            let resp = ui.interact(rr, id, Sense::click());
                            let p = ui.painter_at(rr);
                            if resp.is_pointer_button_down_on() {
                                p.rect_filled(rr, cr(16.0), pal.card);
                            }
                            match row {
                                Row::App(pkg) => {
                                    if let Some(&ai) = self.by_pkg.get(pkg) {
                                        let app = self.apps[ai].clone();
                                        let ic = Rect::from_center_size(pos2(rr.left() + 36.0, rr.center().y), vec2(44.0, 44.0));
                                        widgets::app_icon(&p, ic, &app, self.icons.as_mut().unwrap(), &cfg, &pal, 1.0, 0.0, 1.0);
                                        p.text(pos2(ic.right() + 16.0, rr.center().y), Align2::LEFT_CENTER, cfg.label(&app), fonts::body(&cfg, 17.0 * cfg.font_scale, false), pal.text);
                                        if resp.clicked() && click_ok {
                                            launch = Some(pkg.clone());
                                        }
                                    }
                                }
                                Row::Calc(expr, val) => {
                                    widgets::glyph_at(&p, pos2(rr.left() + 36.0, rr.center().y), Align2::CENTER_CENTER, ph::CALCULATOR, 28.0, pal.accent);
                                    p.text(pos2(rr.left() + 72.0, rr.center().y - 10.0), Align2::LEFT_CENTER, expr, fonts::body(&cfg, 13.0, false), pal.dim);
                                    p.text(pos2(rr.left() + 72.0, rr.center().y + 11.0), Align2::LEFT_CENTER, format!("= {val}"), fonts::body(&cfg, 20.0, true), pal.text);
                                    if resp.clicked() {
                                        platform::clipboard_set(val);
                                    }
                                }
                                Row::Web(q) => {
                                    widgets::glyph_at(&p, pos2(rr.left() + 36.0, rr.center().y), Align2::CENTER_CENTER, ph::GLOBE, 28.0, pal.accent);
                                    p.text(pos2(rr.left() + 72.0, rr.center().y), Align2::LEFT_CENTER, format!("Search the web for “{q}”"), fonts::body(&cfg, 16.0 * cfg.font_scale, false), pal.text);
                                    if resp.clicked() && click_ok {
                                        web = Some(q.clone());
                                    }
                                }
                            }
                        }
                        return;
                    }

                    match cfg.drawer {
                        DrawerStyle::Grid => {
                            let cols = cfg.drawer_cols.max(3);
                            let cw = w / cols as f32;
                            let ch = cfg.eff() + 40.0 * cfg.font_scale + 10.0;
                            let nrows = visible.len().div_ceil(cols);
                            ui.set_height(nrows as f32 * ch + 24.0);
                            let first = ((viewport.top() / ch).floor() as usize).saturating_sub(1);
                            let last = (((viewport.bottom() / ch).ceil() as usize) + 1).min(nrows);
                            let mut labels = widgets::Labels::default();
                            let p = ui.painter().clone();
                            for row in first..last {
                                for c in 0..cols {
                                    let Some(&ai) = visible.get(row * cols + c) else { break };
                                    let cell = Rect::from_min_size(origin + vec2(c as f32 * cw, row as f32 * ch), vec2(cw, ch));
                                    let app = self.apps[ai].clone();
                                    let resp = ui.interact(cell, Id::new(("d", &app.pkg)), Sense::click());
                                    let down = resp.is_pointer_button_down_on();
                                    let ir = Rect::from_center_size(pos2(cell.center().x, cell.top() + 8.0 + cfg.eff() / 2.0), vec2(cfg.eff(), cfg.eff()));
                                    widgets::app_icon(&p, ir, &app, self.icons.as_mut().unwrap(), &cfg, &pal, if down { 0.92 } else { 1.0 }, 0.0, 1.0);
                                    if let Some(n) = self.badges.get(&app.pkg) {
                                        widgets::badge(&p, ir, *n, &cfg, &pal);
                                    }
                                    if cfg.labels {
                                        labels.push(pos2(cell.center().x, ir.bottom() + 6.0), cfg.label(&app), pal.text, cw - 6.0, false, 1.0);
                                    }
                                    self.drawer_touch(ctx, &resp, &app.pkg, ir, &mut launch, &mut grab, click_ok, idle, time);
                                }
                            }
                            labels.flush(&p, &cfg);
                        }
                        DrawerStyle::List => {
                            let rh = 62.0;
                            ui.set_height(visible.len() as f32 * rh + 24.0);
                            let first = ((viewport.top() / rh).floor() as usize).saturating_sub(1);
                            let last = (((viewport.bottom() / rh).ceil() as usize) + 1).min(visible.len());
                            for idx in first..last {
                                let ai = visible[idx];
                                let app = self.apps[ai].clone();
                                let rr = Rect::from_min_size(origin + vec2(0.0, idx as f32 * rh), vec2(w - 22.0, rh));
                                let resp = ui.interact(rr, Id::new(("l", &app.pkg)), Sense::click());
                                let p = ui.painter_at(rr);
                                if resp.is_pointer_button_down_on() {
                                    p.rect_filled(rr, cr(14.0), pal.card);
                                }
                                let ic = Rect::from_center_size(pos2(rr.left() + 30.0, rr.center().y), vec2(44.0, 44.0));
                                widgets::app_icon(&p, ic, &app, self.icons.as_mut().unwrap(), &cfg, &pal, 1.0, 0.0, 1.0);
                                p.text(pos2(ic.right() + 16.0, rr.center().y), Align2::LEFT_CENTER, cfg.label(&app), fonts::body(&cfg, 17.0 * cfg.font_scale, false), pal.text);
                                if let Some(n) = self.badges.get(&app.pkg) {
                                    widgets::badge(&p, Rect::from_center_size(pos2(rr.right() - 30.0, rr.center().y), vec2(14.0, 14.0)), *n, &cfg, &pal);
                                }
                                self.drawer_touch(ctx, &resp, &app.pkg, ic, &mut launch, &mut grab, click_ok, idle, time);
                            }
                        }
                    }
                }
            });

            // letter scrubber on the right edge
            if scrub {
                let letters: Vec<char> = ('A'..='Z').chain(std::iter::once('#')).collect();
                let strip = Rect::from_min_max(pos2(right - 22.0, body.top() + 8.0), pos2(right, body.bottom() - 8.0));
                let step = strip.height() / letters.len() as f32;
                let resp = ui.interact(strip.expand2(vec2(10.0, 0.0)), Id::new("scrub"), Sense::click_and_drag());
                for (i, c) in letters.iter().enumerate() {
                    ui.painter().text(pos2(strip.center().x, strip.top() + (i as f32 + 0.5) * step), Align2::CENTER_CENTER, c.to_string(), fonts::mono(10.5, false), pal.dim);
                }
                if let (true, Some(pos)) = (resp.dragged() || resp.clicked(), resp.interact_pointer_pos()) {
                    let idx = (((pos.y - strip.top()) / step) as usize).min(letters.len() - 1);
                    let letter = letters[idx];
                    let at = visible.iter().position(|&i| {
                        let ch = self.apps[i].key.chars().next().unwrap_or('#');
                        if letter == '#' { !ch.is_alphabetic() } else { ch.to_ascii_uppercase() == letter }
                    });
                    if let Some(at) = at {
                        let off = match cfg.drawer {
                            DrawerStyle::List => at as f32 * 62.0,
                            DrawerStyle::Grid => (at / cfg.drawer_cols.max(3)) as f32 * (cfg.eff() + 40.0 * cfg.font_scale + 10.0),
                        };
                        self.drawer_scroll_to = Some(off);
                        ui.ctx().request_repaint();
                    }
                }
            }

            // pull down on the top edge closes it (Pixel/Nothing have no handle)
            let grip = Rect::from_min_size(rect.min, vec2(rect.width(), it.max(10.0) + 6.0));
            let g = ui.interact(grip, Id::new("drawer-grip"), Sense::drag());
            if g.dragged() && g.drag_delta().y > 0.0 {
                close = true;
            }
            if !at_bottom {
                ui.painter().rect_filled(Rect::from_center_size(pos2(rect.center().x, rect.top() + it.max(10.0) / 2.0 + 2.0), vec2(36.0, 4.0)), cr(2.0), pal.dim.gamma_multiply(0.5));
            }
            let _ = (Stroke::NONE, StrokeKind::Inside);
        });

        if close {
            self.close_drawer(ctx);
        }
        if let Some(pkg) = launch {
            self.launch(ctx, &pkg);
        }
        if let Some(q) = web {
            let q2 = q.clone();
            crate::bridge::bg(move || platform::search_web(&q2));
            self.close_drawer(ctx);
        }
        if let Some((pkg, ir, start)) = grab {
            self.haptic(platform::Haptic::Lift);
            self.suppress_click = time + 0.5;
            self.menu = Some(Menu { target: Target::App { pkg: pkg.clone(), on_home: false }, anchor: ir });
            self.touch = Touch::Lifted { start, item: Item::App(pkg), from: Source::Drawer, rect: ir };
        }
    }

    /// Tap launches; holding still lifts the icon (menu, and a drag onto the home screen if the finger moves).
    #[allow(clippy::too_many_arguments)]
    fn drawer_touch(&mut self, ctx: &Context, resp: &egui::Response, pkg: &str, ir: Rect, launch: &mut Option<String>, grab: &mut Option<(String, Rect, egui::Pos2)>, click_ok: bool, idle: bool, time: f64) {
        if resp.clicked() && click_ok {
            *launch = Some(pkg.to_string());
        }
        if resp.is_pointer_button_down_on() && idle {
            let (t0, origin, latest) = ctx.input(|i| (i.pointer.press_start_time(), i.pointer.press_origin(), i.pointer.latest_pos()));
            if let (Some(t0), Some(o), Some(l)) = (t0, origin, latest) {
                if time - t0 > 0.4 && (l - o).length() < 10.0 && !self.cfg.lock_layout {
                    *grab = Some((pkg.to_string(), ir, o));
                } else if time - t0 <= 0.4 {
                    ctx.request_repaint_after(std::time::Duration::from_millis(60));
                }
            }
        }
    }

    // ───────────────────────────── search overlay ─────────────────────────────

    pub fn search_ui(&mut self, ctx: &Context, screen: Rect) {
        let pal = self.pal;
        let cfg = self.cfg.clone();
        let k = ctx.pixels_per_point();
        let it = self.insets.top / k;
        let ib = self.insets.bottom / k;
        let mut launch: Option<String> = None;
        let mut web: Option<String> = None;
        let mut cancel = false;
        let time = ctx.input(|i| i.time);
        let click_ok = time > self.suppress_click;
        let mut search_sc = std::mem::take(&mut self.search_scroller);
        Area::new(Id::new("search")).order(Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
            let ios = cfg.preset == Preset::Ios;
            ui.painter().rect_filled(screen, 0, if ios { Color32::from_black_alpha(190) } else { pal.sheet });
            ui.allocate_rect(screen, Sense::click_and_drag());
            let top = screen.top() + it.max(10.0) + 10.0;
            let bar = Rect::from_min_size(pos2(screen.left() + 16.0, top), vec2(screen.width() - 32.0 - 72.0, 46.0));
            ui.painter().rect_filled(bar, cr(23.0), if ios { Color32::from_white_alpha(46) } else { pal.search_bg });
            widgets::glyph_at(ui.painter(), bar.left_center() + vec2(16.0, 0.0), Align2::LEFT_CENTER, ph::MAGNIFYING_GLASS, 18.0, pal.dim);
            let field = Rect::from_min_max(pos2(bar.left() + 44.0, bar.top()), pos2(bar.right() - 14.0, bar.bottom()));
            ui.scope_builder(UiBuilder::new().max_rect(field), |ui| {
                let te = egui::TextEdit::singleline(&mut self.query)
                    .hint_text(egui::RichText::new("Search").color(pal.dim))
                    .desired_width(f32::INFINITY)
                    .margin(vec2(0.0, 12.0))
                    .font(fonts::body(&cfg, 16.0, false))
                    .background_color(Color32::TRANSPARENT)
                    .frame(Frame::NONE);
                let r = ui.add(te);
                if self.focus_search {
                    r.request_focus();
                    self.focus_search = false;
                }
            });
            let cancel_r = Rect::from_min_size(pos2(bar.right() + 6.0, top), vec2(66.0, 46.0));
            ui.painter().text(cancel_r.center(), Align2::CENTER_CENTER, "Cancel", fonts::body(&cfg, 16.0, false), if ios { Color32::from_rgb(10, 132, 255) } else { pal.accent });
            if ui.interact(cancel_r, Id::new("cancel-search"), Sense::click()).clicked() {
                cancel = true;
            }

            let rows = results(self, &self.query.clone());
            let area = Rect::from_min_max(pos2(screen.left() + 8.0, bar.bottom() + 14.0), pos2(screen.right() - 8.0, screen.bottom() - ib));
            ui.scope_builder(UiBuilder::new().max_rect(area), |ui| {
                crate::scroll::show(ui, &mut search_sc, "spotlight", area, true, |ui| {
                    let origin = ui.min_rect().min;
                    let rh = 62.0;
                    ui.set_height(rows.len() as f32 * rh);
                    for (i, row) in rows.iter().enumerate() {
                        let rr = Rect::from_min_size(origin + vec2(0.0, i as f32 * rh), vec2(area.width(), rh));
                        let resp = ui.interact(rr, Id::new(("srow", i)), Sense::click());
                        let p = ui.painter_at(rr);
                        if resp.is_pointer_button_down_on() {
                            p.rect_filled(rr, cr(14.0), Color32::from_white_alpha(24));
                        }
                        match row {
                            Row::App(pkg) => {
                                if let Some(&ai) = self.by_pkg.get(pkg) {
                                    let app = self.apps[ai].clone();
                                    let ic = Rect::from_center_size(pos2(rr.left() + 34.0, rr.center().y), vec2(42.0, 42.0));
                                    widgets::app_icon(&p, ic, &app, self.icons.as_mut().unwrap(), &cfg, &pal, 1.0, 0.0, 1.0);
                                    p.text(pos2(ic.right() + 14.0, rr.center().y), Align2::LEFT_CENTER, cfg.label(&app), fonts::body(&cfg, 17.0, false), if ios { Color32::WHITE } else { pal.text });
                                    if resp.clicked() && click_ok {
                                        launch = Some(pkg.clone());
                                    }
                                }
                            }
                            Row::Calc(expr, val) => {
                                widgets::glyph_at(&p, pos2(rr.left() + 34.0, rr.center().y), Align2::CENTER_CENTER, ph::CALCULATOR, 28.0, pal.accent);
                                p.text(pos2(rr.left() + 70.0, rr.center().y - 10.0), Align2::LEFT_CENTER, expr, fonts::body(&cfg, 13.0, false), pal.dim);
                                p.text(pos2(rr.left() + 70.0, rr.center().y + 11.0), Align2::LEFT_CENTER, format!("= {val}"), fonts::body(&cfg, 20.0, true), if ios { Color32::WHITE } else { pal.text });
                            }
                            Row::Web(q) => {
                                widgets::glyph_at(&p, pos2(rr.left() + 34.0, rr.center().y), Align2::CENTER_CENTER, ph::GLOBE, 28.0, pal.accent);
                                p.text(pos2(rr.left() + 70.0, rr.center().y), Align2::LEFT_CENTER, format!("Search the web for “{q}”"), fonts::body(&cfg, 16.0, false), if ios { Color32::WHITE } else { pal.text });
                                if resp.clicked() && click_ok {
                                    web = Some(q.clone());
                                }
                            }
                        }
                    }
                });
            });
        });
        self.search_scroller = search_sc;
        if cancel {
            self.search_open = false;
            self.query.clear();
            ctx.memory_mut(|m| m.stop_text_input());
        }
        if let Some(pkg) = launch {
            self.launch(ctx, &pkg);
        }
        if let Some(q) = web {
            let q2 = q.clone();
            crate::bridge::bg(move || platform::search_web(&q2));
            self.search_open = false;
            self.query.clear();
        }
    }
}
